//! `opai` — the terminal front end for Open Photo AI.
//!
//! Photographs and directories of them in, each enhanced by the same chain, written to one file, under a directory,
//! or without `-o` beside each source:
//!
//! ```text
//! opai <input>... <enhancement flags> [-r] [--format F] [--prefix P] [--suffix S] [--overwrite] [-q hd|sd]
//!      [-p PROCESSOR] [-o <output>]
//! ```
//!
//! Everything the run does is the library's, naming each result included (`opai::export`). What lives here is the
//! terminal's own vocabulary — the flags, the mapping from them to `opai` constructors, expanding the inputs, reading
//! `-o` as a file or a directory and mirroring directory inputs under it (or, without it, the `_opai` default suffix),
//! the lines, the rows and the bar the terminal shows, and the exit status.
//!
//! # Where the output goes
//!
//! Everything this command shows goes to stderr, and stdout is left empty. On a terminal, one live view covers the
//! whole batch, and the summary follows it:
//!
//! ```text
//! 💡 New update available: download at https://github.com/vegidio/open-photo-ai  <- only when a newer release is
//!                                                                                   published, then a blank line
//! 💾 Downloading dependency ONNX Runtime... ✓ 100% Complete  <- only when a dependency is downloaded; a spinner
//!                                                               turns until it reaches 100%
//! 🧠 Running inference using the processor Auto; quality HD  <- the processor asked for, in every mode
//!
//! Enhancing [2/3]  ██████████████████████░░░░░░░░░   79.0%   ETA 12s   <- the batch's bar, and the time it has left
//!
//! 🖼️ [100%] Enhancements applied to       photos/a.jpg   <- the list: the five most recent images
//! 🧨 [ 42%] Failed to enhance             photos/b.jpg   <- red
//! 🖼️ [ 37%] Applying Upscale to           photos/c.jpg
//!
//! ✨ 2 images enhanced successfully; files saved under out  <- or, for one image: … file saved at out_1.png
//! 🧨 could not enhance photos/b.jpg: <why>                 <- each failure, for a batch of more than one
//! 🧨 1 of 3 images failed                                  <- the closing line; a single image's is its own failure
//! ```
//!
//! The dependency line and the view are redrawn in place, and the lines are coloured unless `NO_COLOR` is set. Where
//! stderr is not a terminal, the processor line and a blank line come first, then each image prints one line without
//! colour when it ends (`🖼️ [100%] Enhancements applied to …`, `🧨 [ 42%] Failed to enhance …: <why>`), then the
//! success line and the closing line.
//!
//! The notes, the face report, what the run executed on and every failure in full go to the log file, `opai.log`.
//! clap's own `--help`, `--version` and argument errors are printed as clap prints them.
//!
//! # Telemetry
//!
//! As the GUI does, a run sends what the log file receives to the project's collector, beside the usage events in
//! `events` and a span for the run and for each image, unless `--no-analytics` is given or `OPAI_ANALYTICS` is `0`,
//! `off`, `false` or `no` (see `analytics`). Nothing is sent before the arguments have parsed.

mod analytics;
mod args;
mod autopilot;
mod chain;
mod error;
mod events;
mod inputs;
mod look;
mod output;
mod progress;
mod run;
#[cfg(test)]
mod test_support;
mod update;

use std::process::ExitCode;
use std::time::Instant;

use tracing::Instrument;

// A plain `main` building its runtime by hand rather than `#[tokio::main]`, because the attribute starts the worker
// threads before the body's first statement — and `prepare_library_path` is only sound while no other thread exists.
// On Linux it may also replace the process image, which has to happen before any runtime is running.
fn main() -> ExitCode {
    // How long the session lasted is what its closing record says.
    let began = Instant::now();

    // Set the CUDA and TensorRT provider library search path.
    //
    // SAFETY: the first statement of `main`, so no thread but this one exists and nothing has yet read or written the
    // process environment.
    let prepared = unsafe { opai::prepare_library_path(opai::APP_NAME) }.map(|_| ());

    // Before anything this binary prints, so that a session's records begin where the session does. A failure is
    // reported and the run continues: a terminal application that cannot write a log is still a terminal application.
    if let Err(error) = opai::logging::init(opai::APP_NAME, opai::CLI) {
        eprintln!("opai: could not install the log sink; continuing without a log: {error}");
    }

    // A usage error, `--help` and `--version` all end here, with clap's own message and exit status, and before
    // anything is initialized.
    let args = args::parse();

    // Only once the arguments have parsed, so `--help`, `--version` and a usage error never open a connection.
    let enabled = analytics::enabled(args.no_analytics, std::env::var_os(analytics::ENV).as_deref());
    analytics::begin(enabled, opai::telemetry::start);

    // Not fatal, as in the GUI: only the GPU providers are affected, and a run on the CPU still produces the image.
    if let Err(error) = prepared {
        tracing::warn!("could not establish the library search path; GPU providers will be unavailable: {error}");
    }

    // Decided once, before anything is drawn: whether the lines are live, and whether they are coloured.
    let look = look::Look::for_stderr();

    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!("could not start the async runtime: {error}");
            eprintln!("{}", look::failure(format_args!("could not start the async runtime: {error}"), look.color));
            analytics::closing(began.elapsed(), opai::telemetry::stop);
            return ExitCode::FAILURE;
        }
    };

    // The run's unit of work, as each GUI command's span is: everything the library does for it is beneath it.
    let span = tracing::info_span!(
        target: analytics::TARGET,
        "run",
        outcome = tracing::field::Empty,
        error = tracing::field::Empty
    );

    // The `Opai` handle lives and dies inside `run`, so the single-instance claim is released and the run cache's
    // pending writes are settled before the process exits. Every live session is finished by then too, and the summary
    // printed, so the failure line needs nothing before it.
    let result = runtime.block_on(run::run(args, look).instrument(span.clone()));
    analytics::mark(&span, &result);
    drop(span);

    let status = match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{}", full(&error));
            eprintln!("{}", look::failure(&error, look.color));
            ExitCode::from(error.exit_code())
        }
    };

    // Last, so the failure above is in the flush the stop makes.
    analytics::closing(began.elapsed(), opai::telemetry::stop);

    status
}

/// `error` and every error under it, joined by `: ` — the whole of what went wrong, for the log.
fn full(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();

    while let Some(cause) = source {
        let message = cause.to_string();
        // A wrapper whose message already ends with its cause's says nothing new by repeating it.
        if !text.ends_with(&message) {
            text = format!("{text}: {message}");
        }
        source = cause.source();
    }

    text
}
