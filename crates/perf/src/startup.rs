//! Getting the library ready, and everything that has to be said about a startup before a measurement begins.
//!
//! Two decisions are made here and nowhere else. The first is that this binary runs against the **shared**
//! configuration directory under the identity [`PERF`] — the same one the GUI and the CLI use — so a sweep reuses an
//! installed runtime and installed weights instead of pulling several gigabytes into a directory of its own, and so
//! that `--skip-verify` means something: the directory an operator drops a re-exported `.onnx` into is the one this
//! reads. The cost is that a sweep is refused while the GUI is running, which is the single-instance claim working
//! and is reported as such rather than as a generic failure.
//!
//! The second is that install progress goes to **stderr**, for the reason the crate root gives under *Where the
//! output goes*.

use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use opai::{APP_NAME, InitError, InitOptions, LogError, ModelTrust, OnProgress, Opai, PERF, Phase, Progress, logging};

use crate::cli::Options;
use crate::view;

/// A library that is ready to be measured, and what has to be said about how it got there.
#[derive(Debug)]
pub struct Started {
    /// The application handle every run goes through.
    pub opai: Opai,

    /// Where the library's own records went, or `None` where the sink could not be installed.
    ///
    /// Reported in the header because it replaces the reference's `-v`-prints-the-debug-log entirely: the per-tile
    /// and per-session records are in that file, and `RUST_LOG` is what turns them up.
    pub log: Option<PathBuf>,
}

/// Installs the log sink and initializes the library.
///
/// The sink goes in **first**, so a session's records begin where the session does and the initialization path's own
/// records are in the file. A sink that cannot be installed is reported and the run continues: a benchmark that
/// cannot write a log is still a benchmark, and the header says the log is missing rather than the run failing over
/// it.
///
/// # Errors
///
/// [`InitError`], unchanged — every one of them ends the run, and [`report`] is what turns one into something worth
/// reading.
pub async fn start(options: &Options) -> Result<Started, InitError> {
    let log = match logging::init(APP_NAME, PERF) {
        Ok(path) => Some(path),
        Err(error) => {
            warn_sink(&error);
            None
        }
    };

    // `--skip-verify` is declared **here** and reaches the library nowhere else: there is no setter and no per-run
    // field, so a sweep that did not ask for it when it started cannot arrive at it later.
    let models = if options.skip_verify { ModelTrust::LocalFiles } else { ModelTrust::default() };

    if options.skip_verify {
        eprintln!(
            "perftest: --skip-verify is on, so the model files on disk are used without checking them against the \
             published hashes. A surprising number is worth suspecting this first."
        );
    }

    // `..Default::default()` rather than every field named, which is the idiom `InitOptions` documents and the
    // property it exists for: the plan report this sweep has no use for — it prints each install as it happens rather
    // than drawing a component list — costs this call site nothing.
    let init = InitOptions {
        app: Some(PERF.to_string()),
        models,
        on_progress: Some(install_progress()),
        ..Default::default()
    };

    let opai = Opai::initialize(APP_NAME, Some(init)).await?;

    Ok(Started { opai, log })
}

/// What to print when a startup failed.
///
/// Everything but the single-instance refusal is the library's own sentence, which already names what it was working
/// on. The refusal gets one line more, because "close the GUI" is the action and because the two things that still
/// work while it is held are worth saying to whoever just wanted a list.
pub fn report(error: &InitError) -> String {
    match error {
        // The library's own message already renders the holder and its pid — *"Open Photo AI is already running
        // (gui, pid 4821); close it and try again"* — so it is printed unchanged rather than recomposed here, which
        // is the version that could disagree with it.
        InitError::AlreadyRunning { .. } => {
            format!("perftest: {error}\n  `perftest list` and `perftest --help` work while it is running.")
        }
        other => format!("perftest: {other}"),
    }
}

/// Reports an install to stderr, so a first run does not look hung for several minutes.
///
/// One line per dependency, rewritten in place where stderr is a terminal and printed at each phase boundary where
/// it is not — a redirected stderr should not collect a megabyte of carriage returns.
///
/// This is install progress only. **A run's progress goes to the renderer's own callback** instead
/// (`sweep::Listener::on_progress`), which stores a number and writes nothing: the models invoke it per tile, and
/// anything that writes to a terminal from inside the timed section is inside the measurement.
fn install_progress() -> OnProgress {
    // What is currently being reported and whether it has already been announced as finished, so a dependency's
    // last report is not printed twice and so its line is closed before the next one opens its own.
    let current: Mutex<Option<(String, Phase)>> = Mutex::new(None);
    let interactive = std::io::stderr().is_terminal();

    Arc::new(move |progress: &Progress| {
        let Ok(mut current) = current.lock() else {
            // A poisoned lock means another reporting thread panicked. The install itself is unaffected, and losing
            // a progress line is not worth failing a sweep over.
            return;
        };

        let name = progress.dependency.as_str().to_string();
        let step = (name.clone(), progress.phase);

        // A dependency that has already reported itself finished says nothing further: the transfer and the
        // expansion both end at a fraction of 1.0, so without this the last one is announced twice.
        //
        // It also swallows the whole of what a dependency with **nothing to do** reports — `Phase::AlreadyInstalled`,
        // which is terminal at 1.0 with no phase opened before it — and that is the wanted answer rather than a
        // coincidence worth removing: a line here means work this run paid for, and a steady-state sweep pays for
        // none. Without it, `view::code` has no name for the variant and would announce each one as
        // "installing 100%" — four lines about installs that did not happen, immediately above the measurement.
        if current.is_none() && progress.fraction >= 1.0 {
            return;
        }

        let changed = current.as_ref() != Some(&step);
        let finished = progress.fraction >= 1.0;

        let mut stderr = std::io::stderr().lock();

        if interactive {
            // One line per dependency, rewritten in place, closed when the dependency is done.
            let opening = current.as_ref().map(|(dependency, _)| dependency.as_str()) != Some(name.as_str());
            if opening && current.is_some() {
                let _ = writeln!(stderr);
            }

            let _ = write!(
                stderr,
                "\rperftest: installing {name} — {} {:3.0}%",
                view::phase_label(progress.phase),
                progress.fraction * 100.0
            );

            if finished {
                let _ = writeln!(stderr);
            }
        } else if changed || finished {
            // A redirected stderr should collect a line per phase boundary rather than a megabyte of carriage
            // returns.
            let _ = writeln!(
                stderr,
                "perftest: installing {name} — {} {:3.0}%",
                view::phase_label(progress.phase),
                progress.fraction * 100.0
            );
        }

        let _ = stderr.flush();

        *current = if finished { None } else { Some(step) };
    })
}

/// Says the log sink could not be installed, once, to stderr.
fn warn_sink(error: &LogError) {
    eprintln!("perftest: could not install the log sink; continuing without a log: {error}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use opai::{GUI, Holder};

    #[test]
    fn a_contended_start_names_the_holder_its_pid_and_what_still_works() {
        let error = InitError::AlreadyRunning { holder: Some(Holder { app: GUI.to_string(), pid: 4821 }) };

        let message = report(&error);

        // The holder and its pid, so nobody has to go looking for which process is in the way.
        assert!(message.contains("gui, pid 4821"), "{message}");
        assert!(message.contains("close it"), "{message}");
        // And the two things the initialization path's own comment promises still work while a claim is held.
        assert!(message.contains("perftest list"), "{message}");
        assert!(message.contains("--help"), "{message}");
    }

    #[test]
    fn any_other_startup_failure_is_the_librarys_own_sentence() {
        let error = InitError::UnknownProvider { name: "openvino".to_string() };

        assert_eq!(report(&error), format!("perftest: {error}"));
    }
}
