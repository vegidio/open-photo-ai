// Raised from the default 128 because `sweep` awaits `Opai::process`, whose future nests one level deeper for the
// session-acquisition helper both inference drivers call. At 128 the layout query for this crate's own `sweep` future
// overflows, which is a compile error here rather than anything about this crate's code. It costs nothing at runtime;
// the limit bounds a compile-time recursion, not a call depth.
#![recursion_limit = "256"]

//! `perftest` — the inference benchmark for Open Photo AI.
//!
//! Measures one model at a time against one image — an untimed warm-up, a timed cold start, a number of timed runs,
//! and the distribution over them — and reports the conditions that produced the numbers beside the numbers
//! themselves.
//!
//! # Where the output goes
//!
//! The report is the **whole** of stdout, rendered or JSON. Install progress, the per-model lines, warnings about
//! the configuration and every diagnostic go to stderr, so that `perftest --json > results.json` produces a
//! parseable file on a machine that downloaded a runtime on the way.

mod cli;
mod input;
mod json;
mod report;
mod select;
mod startup;
mod stats;
mod sweep;
#[cfg(test)]
mod test_support;
mod view;
mod viewport;

use std::io::IsTerminal;
use std::process::ExitCode;
use std::time::Duration;

use clap::Parser;
use opai::{CancellationToken, Opai};

use crate::cli::{Cli, Command, Options};
use crate::input::Input;
use crate::report::Conditions;
use crate::select::Selected;
use crate::sweep::{Lines, Listener, Silent, SweepResult};
use crate::viewport::{Viewport, Watcher};

// A plain `main` building its runtime by hand rather than `#[tokio::main]`, because the attribute starts the worker
// threads before the body's first statement — and `prepare_library_path` is only sound while no other thread exists.
fn main() -> ExitCode {
    // Set the CUDA and TensorRT provider library search path.
    //
    // SAFETY: the first statement of `main`, so no thread but this one exists and nothing has yet read or written the
    // process environment.
    let prepared = unsafe { opai::prepare_library_path(opai::APP_NAME) }.map(|_| ());

    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => return fail(&format!("perftest: could not start the async runtime: {error}")),
    };

    runtime.block_on(start(prepared))
}

async fn start(prepared: Result<(), opai::InitError>) -> ExitCode {
    let arguments = Cli::parse();

    match arguments.command {
        // Before anything is initialized, so it works while the GUI holds the single-instance claim.
        Some(Command::List) => {
            print!("{}", select::listing());
            ExitCode::SUCCESS
        }
        None => {
            // Not fatal, as in the GUI: only the GPU providers are affected, and a sweep on them reports every
            // fallback to the CPU in its warnings. Said here too so that those warnings have a cause beside them.
            if let Err(error) = prepared {
                eprintln!(
                    "perftest: could not establish the library search path; GPU providers will be unavailable: {error}"
                );
            }

            run(&arguments.options).await
        }
    }
}

/// One sweep, from the selection to the exit status.
async fn run(options: &Options) -> ExitCode {
    // First, and before the runtime is touched: a typo in a model name must fail in milliseconds rather than after a
    // multi-gigabyte transfer, and being refused because the GUI is running when all that was wrong is a typo would
    // report the wrong problem.
    let selection = match select::resolve(options) {
        Ok(selection) => selection,
        Err(error) => return fail(&format!("perftest: {error}")),
    };

    let started = match startup::start(options).await {
        Ok(started) => started,
        Err(error) => return fail(&startup::report(&error)),
    };

    let input = match input::load(options.image.as_deref()).await {
        Ok(input) => input,
        Err(error) => return fail(&format!("perftest: {error}")),
    };

    let cancel = CancellationToken::new();
    watch_for_interrupt(&cancel);

    // The auxiliary run, and the only inference this binary makes outside a measurement: a face-recovery model
    // restores the faces it is given and finds none of its own, so a sweep that selected one obtains them **once**,
    // here, and hands them to every row that needs them. Its cost is charged to nothing — it is before the first
    // model starts — and a sweep that selected no such row does not make it at all.
    //
    // Before the header as well as before the sweep, because what it found is one of the conditions the numbers were
    // produced under.
    let detected = if select::needs_faces(&selection) {
        Some(sweep::detect(&started.opai, &input, options.provider, &cancel).await)
    } else {
        None
    };

    let selection = match &detected {
        Some(detected) => select::with_faces(selection, detected),
        None => selection,
    };

    let conditions = Conditions {
        options,
        input: &input,
        supported: started.opai.providers().available(),
        cache_mode: started.opai.cache_mode(),
        log: started.log.as_deref(),
        selected: selection.len(),
        detected: detected.as_ref(),
    };

    // The header before the sweep, so an operator watching a run that takes minutes can see what it is measuring —
    // and only in the rendered form, because under `--json` stdout is the document and nothing else.
    if !options.json {
        print!("{}", report::header(&conditions));
    }

    let named = !options.models.is_empty();

    let sweep = Sweep { opai: &started.opai, input: &input, selection: &selection, options, named, cancel: &cancel };

    let result = match renderer(options, std::io::stderr().is_terminal(), std::io::stdout().is_terminal()) {
        Renderer::Silent => paced(&sweep, &mut Silent, || {}).await,
        Renderer::Lines => paced(&sweep, &mut Lines, || {}).await,
        Renderer::Viewport => watched(&sweep).await,
    };

    if options.json {
        match json::render(&json::document(&conditions, &result)) {
            Ok(document) => println!("{document}"),
            Err(error) => return fail(&format!("perftest: the report could not be serialized: {error}")),
        }
    } else {
        print!("{}", report::body(&conditions, &result));
    }

    // Said on stderr rather than folded into the report, so that neither rendering has to carry it: what the status
    // means is for whoever ran the command, not for whoever reads the table later.
    if result.interrupted() {
        eprintln!("perftest: interrupted; the report above covers the models that finished.");
    } else if result.failures() > 0 {
        eprintln!("perftest: {} model(s) failed.", result.failures());
    }

    ExitCode::from(result.exit_code() as u8)
}

/// Everything one sweep is run from, so that choosing a renderer does not mean writing the call out three times.
struct Sweep<'a> {
    opai: &'a Opai,
    input: &'a Input,
    selection: &'a [Selected],
    options: &'a Options,
    named: bool,
    cancel: &'a CancellationToken,
}

impl Sweep<'_> {
    /// The sweep, reporting to `listener`.
    async fn run(&self, listener: &mut dyn Listener) -> SweepResult {
        sweep::sweep(self.opai, self.input, self.selection, self.options, self.named, self.cancel, listener).await
    }
}

// Below about ten frames a second the spinner reads as stuttering; above about twenty nothing is gained on any
// terminal and the cost is a line of escape codes per frame over what may be an SSH connection. This is the low end
// of the range that looks right, because the thing being watched takes minutes.
/// How often the sweep's own task is woken, and so how often the live view is redrawn.
const FRAME: Duration = Duration::from_millis(1_000 / 15);

/// The sweep, polled beside a ticker at [`FRAME`], with `tick` called on every beat.
///
/// **Every renderer is paced the same way, including the two that draw nothing**, so what a sweep measures does not
/// depend on whether anyone was watching it.
async fn paced(sweep: &Sweep<'_>, listener: &mut dyn Listener, mut tick: impl FnMut()) -> SweepResult {
    // A measurement decision rather than a structural convenience. Unpaced, a sweep under `--plain` measures about
    // five percent slower on the CPU provider than the same sweep watched by the live view, reproducibly, and in the
    // direction that says the display is not the thing being charged for. Removing the progress callback from every
    // timed run does not close the gap, so the callback is not the cause; what remains between the two is that one
    // task is woken fifteen times a second and the other sits at an `await` for the length of a run, which is the sort
    // of thing a scheduler reads as a process that does not need a fast core.
    //
    // Rather than explain that away in the report, the two are made the same. A timer that fires a closure doing
    // nothing costs nothing worth naming, and it buys the one property this capability is entirely about — that its
    // numbers can be compared.
    let mut running = std::pin::pin!(sweep.run(listener));
    let mut ticker = tokio::time::interval(FRAME);

    loop {
        tokio::select! {
            result = &mut running => break result,
            _ = ticker.tick() => tick(),
        }
    }
}

/// The sweep with the live view drawn beside it.
///
/// A view that cannot be opened is reported and the sweep runs with the plain renderer.
async fn watched(sweep: &Sweep<'_>) -> SweepResult {
    let shared = view::Shared::new();
    let mut watcher = Watcher::new(&shared);

    let mut live = match Viewport::open(&shared) {
        Ok(live) => live,
        Err(error) => {
            // Not a workaround but the other half of the renderer choice — a benchmark that cannot draw is still a
            // benchmark.
            eprintln!("perftest: the live view could not be opened; reporting as plain lines instead: {error}");
            return sweep.run(&mut Lines).await;
        }
    };

    // The sweep future is polled by the **same task that draws**, so nothing is spawned, nothing has to become
    // `'static`, and every borrow the sweep's signature takes stays a borrow. What makes that work is that the
    // inference runs on a blocking thread: the sweep future is parked at an `await` whenever there is work happening,
    // and the ticker — `paced`'s, the same one the renderers that draw nothing run under — gets its turn.
    //
    // A failed frame is ignored rather than fatal: a terminal that stops accepting frames partway through is not a
    // reason to throw away a sweep that has been running for an hour.
    let result = paced(sweep, &mut watcher, || drop(live.draw())).await;

    // Whatever the last model left in the scrollback goes out before the view does, and the view leaves nothing of
    // itself behind for the summary to be printed over.
    if let Err(error) = live.close() {
        eprintln!("perftest: the live view could not be closed cleanly: {error}");
    }

    result
}

/// Cancels the run in flight when the operator interrupts.
///
/// The token is [`ProcessOptions::cancel`](opai::ProcessOptions::cancel), which the library already honours: a run
/// fails as a whole with `InferenceError::Cancelled` and produces no partial image. The sweep breaks out, the model
/// in flight is marked interrupted rather than failed, and the summary for the models that finished is still
/// printed — which is why this cancels a token rather than ending the process.
fn watch_for_interrupt(cancel: &CancellationToken) {
    let cancel = cancel.clone();

    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            eprintln!("\nperftest: interrupted — stopping the run in flight and reporting what finished.");
            cancel.cancel();
        }
    });
}

/// Reports a failure that ended the run before a sweep could produce anything.
fn fail(message: &str) -> ExitCode {
    eprintln!("{message}");

    ExitCode::FAILURE
}

/// Which of the three progress renderers a sweep gets.
///
/// The choice is made once, from three facts, and the report never learns which way it went: both the rendered
/// summary and the JSON document are composed from the sweep's result after it has ended, so a renderer can only
/// affect what was shown *during* the sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Renderer {
    /// The live view, drawn on stderr.
    Viewport,

    /// One line per model, on stderr.
    Lines,

    /// Nothing at all, for `--json`.
    Silent,
}

/// The renderer for a sweep run with `options` against the given streams.
///
/// The view draws on **stderr**, which is where every renderer writes and which leaves stdout to be the report
/// entire — so `perftest kyoto 2> /dev/null` correctly gets no view.
///
/// **Stdout has to be a terminal too.** What a redirected sweep loses is the view, not its progress:
/// `perftest kyoto > results.txt` still reports a line per model on stderr, and the file still holds the report and
/// nothing else.
fn renderer(options: &Options, stderr_is_terminal: bool, stdout_is_terminal: bool) -> Renderer {
    if options.json {
        // A machine asked for a document, and progress it did not ask for is not written to either stream.
        return Renderer::Silent;
    }

    // Stdout is tested as a limitation rather than a preference. Placing a view below output that already exists
    // means asking the terminal where the cursor is, and the question is written to stdout by the layer that asks it —
    // not to the stream the view draws on. With stdout redirected the question lands in the report file instead of on
    // the screen, nothing answers it, and the view gives up after a timeout. Testing stdout here is what turns that
    // into an immediate, clean choice of the plain renderer: no stray escape in the captured report, no pause before
    // the sweep starts, and no terminal mode entered to ask a question nobody will hear.
    //
    // `--verbose` is deliberately not one of these facts: it only lengthens the report printed after the sweep, and the
    // library's own records go to the file `logging::init` owns rather than to stderr. Nothing writes to stderr during
    // a sweep except the renderer itself.
    if options.plain || !stderr_is_terminal || !stdout_is_terminal {
        return Renderer::Lines;
    }

    Renderer::Viewport
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats;
    use crate::sweep::{Measured, ModelResult, Outcome, Stage, Verdict};
    use crate::view::Shared;
    use crate::viewport::Viewport as LiveView;
    use opai::{CacheMode, ExecutionProvider, Family, FloatPrecision, Scale, Upscale};
    use ratatui::backend::TestBackend;
    use std::path::Path;

    /// The options of a command line, as the binary would see it.
    fn options(arguments: &[&str]) -> Options {
        Cli::try_parse_from(std::iter::once("perftest").chain(arguments.iter().copied()))
            .expect("the arguments parse")
            .options
    }

    fn selected() -> Selected {
        Selected {
            codename: "kyoto",
            family: Family::Upscale,
            operation: Upscale::kyoto(FloatPrecision::Fp32, Scale::new(4.0).expect("4x is in range")),
            blocked: None,
        }
    }

    /// One sweep's worth of results, as a measurement would leave them.
    fn measured() -> SweepResult {
        let runs = vec![Duration::from_millis(400), Duration::from_millis(420), Duration::from_millis(410)];

        SweepResult {
            results: vec![
                ModelResult {
                    codename: "kyoto",
                    family: Family::Upscale,
                    display_name: "Kyoto 4x (FP32)".to_string(),
                    outcome: Outcome::Completed(Box::new(Measured {
                        cold: Duration::from_millis(2_400),
                        stats: stats::compute(&runs),
                        runs,
                        output: (2560, 2560),
                        providers: Verdict::AsRequested(ExecutionProvider::CoreMl),
                    })),
                },
                ModelResult {
                    codename: "osaka",
                    family: Family::Upscale,
                    display_name: "Osaka 4x (FP16)".to_string(),
                    outcome: Outcome::Declined {
                        reason: "no pipeline for Osaka 4x (FP16): this variant does not declare its decoder graph"
                            .to_string(),
                    },
                },
            ],
            skipped: 3,
        }
    }

    /// Every call a renderer is given over one model, in loop order.
    fn watch(listener: &mut dyn Listener, result: &SweepResult) {
        for (index, model) in result.results.iter().enumerate() {
            listener.started(index, result.results.len(), &selected());
            listener.stage(Stage::Warmup { run: 1, of: 1 });
            listener.stage(Stage::Cold);
            listener.stage(Stage::Timed { run: 1, of: 1 });
            listener.stage(Stage::Run { elapsed: Duration::from_millis(410) });
            listener.finished(model);
        }
    }

    #[tokio::test]
    async fn the_report_is_the_same_document_whichever_renderer_was_watching() {
        let input = input::load(None).await.expect("the embedded sample decodes");
        let result = measured();

        let rendered = |options: &Options| {
            let conditions = Conditions {
                options,
                input: &input,
                supported: vec![ExecutionProvider::Cpu, ExecutionProvider::CoreMl],
                cache_mode: CacheMode::Disk,
                log: Some(Path::new("/config/logs/opai.log")),
                selected: 5,
                detected: None,
            };

            (
                report::header(&conditions),
                report::body(&conditions, &result),
                json::render(&json::document(&conditions, &result)).expect("the document serializes"),
            )
        };

        // The plain renderer, driven over the same results.
        let plain = options(&["--plain"]);
        watch(&mut Lines, &result);
        let after_lines = rendered(&plain);

        // And the live view, over a terminal, driven over the same results.
        let watched = options(&[]);
        let shared = Shared::new();
        let mut watcher = Watcher::new(&shared);
        let mut live = LiveView::over(TestBackend::new(100, 8), &shared).expect("the view opens");
        watch(&mut watcher, &result);
        live.draw().expect("a frame");
        live.close().expect("the view closes");
        let after_viewport = rendered(&watched);

        // Byte for byte, in both renderings — the property [`Renderer`] documents.
        assert_eq!(after_lines, after_viewport);
    }

    #[test]
    fn a_live_view_is_never_chosen_under_json_however_the_sweep_was_run() {
        // The other half of the same requirement: a machine asked for a document, and a document is what the file
        // holds — with nothing of a display interleaved into it and nothing on stdout but the report.
        for arguments in [&["--json"][..], &["--json", "--verbose"], &["--json", "--plain"]] {
            for streams in [(true, true), (true, false), (false, true), (false, false)] {
                assert_ne!(renderer(&options(arguments), streams.0, streams.1), Renderer::Viewport, "{arguments:?}");
            }
        }
    }

    #[test]
    fn the_view_is_chosen_only_where_both_streams_are_a_terminal_and_neither_flag_was_given() {
        assert_eq!(renderer(&options(&[]), true, true), Renderer::Viewport);
    }

    #[test]
    fn a_redirected_stderr_gets_the_lines_it_gets_today_rather_than_silence() {
        // The requirement is that a sweep with no terminal still reports its progress, as plain lines.
        assert_eq!(renderer(&options(&[]), false, true), Renderer::Lines);
        assert_eq!(renderer(&options(&["--verbose"]), false, true), Renderer::Lines);
    }

    #[test]
    fn a_captured_report_gets_the_plain_lines_rather_than_a_view_that_would_write_into_it() {
        // `perftest kyoto > results.txt`, which is the shape [`renderer`] tests stdout for.
        assert_eq!(renderer(&options(&[]), true, false), Renderer::Lines);
        assert_eq!(renderer(&options(&["--verbose"]), true, false), Renderer::Lines);
    }

    #[test]
    fn the_plain_form_can_be_asked_for_on_a_terminal() {
        assert_eq!(renderer(&options(&["--plain"]), true, true), Renderer::Lines);
    }

    #[test]
    fn json_is_silent_however_it_is_run() {
        // Whichever way the other facts fall: stdout is the document, and a view drawn beside it is progress a
        // machine did not ask for.
        for streams in [(true, true), (true, false), (false, true), (false, false)] {
            for arguments in [&["--json"][..], &["--json", "--plain"], &["--json", "--verbose"]] {
                assert_eq!(renderer(&options(arguments), streams.0, streams.1), Renderer::Silent, "{arguments:?}");
            }
        }
    }

    #[test]
    fn verbose_does_not_force_the_plain_form() {
        // Which in the reference it must, because its `-v` installs a log handler onto stderr mid-sweep. Here
        // `--verbose` only adds the per-run listing to the report printed after the sweep.
        assert_eq!(renderer(&options(&["--verbose"]), true, true), Renderer::Viewport);
    }
}
