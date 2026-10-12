//! `opai` — the terminal front end for Open Photo AI.
//!
//! One photograph in, upscaled and/or face-restored, one file out:
//!
//! ```text
//! opai <input> [--up [MODEL]] [--scale N] [--fr [MODEL]] [-q hd|sd] -o <output>
//! ```
//!
//! Everything the run does is the library's. What lives here is the terminal's own vocabulary — the flags, the mapping
//! from them to `opai` constructors, the progress bars and the exit status.
//!
//! # Where the output goes
//!
//! Everything this command says — progress, notes, warnings, the completion line and every error — goes to stderr.
//! stdout is left empty, so nothing a pipeline reads is interleaved with a progress bar.

mod args;
mod chain;
mod error;
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
        eprintln!("opai: could not establish the library search path; GPU providers will be unavailable: {error}");
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("opai: could not start the async runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    // The `Opai` handle lives and dies inside `run`, so the single-instance claim is released and the run cache's
    // pending writes are settled before the process exits.
    match runtime.block_on(run::run(args)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("opai: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}
