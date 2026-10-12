//! `opai` — the terminal front end for Open Photo AI.
//!
//! One photograph in, upscaled and/or face-restored, one file out:
//!
//! ```text
//! opai <input> [--up [MODEL]] [--scale N] [--fr [MODEL]] [-q hd|sd] -o <output>
//! ```
//!
//! Everything the run does is the library's. What lives here is the terminal's own vocabulary — the flags, the mapping
//! from them to `opai` constructors, the lines and the bar the terminal shows, and the exit status.
//!
//! # Where the output goes
//!
//! Everything this command shows goes to stderr, and stdout is left empty. On a terminal:
//!
//! ```text
//! 💾 Downloading dependency ONNX Runtime   34%            <- only when a dependency is downloaded
//! 🖼️ Applying enhancements to the image photo.jpg
//!
//! Face Recovery   [1/2]  ████████████░░░░░░░░░░░░░░░   38.4%
//!
//! ✨ Image enhanced successfully; file saved at out.avif  <- or: 🧨 <what went wrong>
//! ```
//!
//! The dependency line and the bar are redrawn in place, and the lines are coloured unless `NO_COLOR` is set. Where
//! stderr is not a terminal, only the header and the result line are printed, without colour.
//!
//! The notes, the face report, what the run executed on and every failure in full go to the log file, `opai.log`.
//! clap's own `--help`, `--version` and argument errors are printed as clap prints them.

mod args;
mod chain;
mod error;
mod look;
mod output;
mod progress;
mod run;

use std::process::ExitCode;

// A plain `main` building its runtime by hand rather than `#[tokio::main]`, because the attribute starts the worker
// threads before the body's first statement — and `prepare_library_path` is only sound while no other thread exists.
// On Linux it may also replace the process image, which has to happen before any runtime is running.
fn main() -> ExitCode {
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
            return ExitCode::FAILURE;
        }
    };

    // The `Opai` handle lives and dies inside `run`, so the single-instance claim is released and the run cache's
    // pending writes are settled before the process exits. Every live line is finished by then too, and the blank
    // line after a bar printed, so the failure line needs nothing before it.
    match runtime.block_on(run::run(args, look)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{}", full(&error));
            eprintln!("{}", look::failure(&error, look.color));
            ExitCode::from(error.exit_code())
        }
    }
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
