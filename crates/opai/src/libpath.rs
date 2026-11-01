//! Putting the installed library folders where the dynamic loader will look for them.
//!
//! The mechanism is `rust-sak`'s `ai::libpath::prepare`: `LD_LIBRARY_PATH` and one restart on Linux (with the active
//! Windows driver's folder under WSL2), `PATH` on Windows, nothing on macOS. What is here is which folders: the four
//! version folders of the shared dependency root, computed without touching the network, in install order.

use std::path::PathBuf;

use crate::config;
use crate::error::InitError;

/// The variable whose presence says this process is already the replacement, so the restart happens at most once
/// however many times the entry point runs. Unchanged from earlier releases, so a process upgraded in place behaves
/// the same.
const REEXEC_GUARD: &str = "OPAI_REEXEC";

/// Establishes the dynamic loader's search path for the runtime's and the NVIDIA libraries' folders, and on Linux
/// restarts the process under it.
///
/// Call this as the **first statement of `main`**, before the logging setup, before any runtime is started, and
/// before any other thread exists. What it does per platform:
///
/// - **Linux**: composes `LD_LIBRARY_PATH` and replaces the process image with one started under it, at most once per
///   process tree. On success this **does not return**. A replacement that fails is reported and startup continues
///   under the inherited path, which costs the GPU providers rather than the launch.
/// - **Windows**: prepends the folders to `PATH`, which `LoadLibrary` consults at call time.
/// - **macOS**: nothing. No NVIDIA library is published for it, and the CoreML provider ships inside the runtime.
///
/// On Linux and Windows the folders are created, since a loader permanently skips a search-path entry that is missing
/// when the process starts. All four are created on every machine, NVIDIA or not: nothing can probe the GPU this early
/// (a CUDA probe starts threads), so a machine without one keeps three empty folders in the shared root. `name` is the application's, validated as [`Opai::initialize`](crate::Opai::initialize)
/// validates it; the folders are the dependency root's and the same for every application.
///
/// # Safety
///
/// Must be called before any other thread exists — the first statement of `main`. It mutates the process environment,
/// which races with any other thread reading it, and on Linux it replaces the process image.
///
/// # Errors
///
/// Returns [`InitError::InvalidName`] if `name` cannot be a directory, [`InitError::Fs`] if the dependency root could
/// not be resolved or created, and [`InitError::Io`] if a library folder could not be created. A failed restart is
/// **not** an error: it is reported and the folders are returned as though there had been nothing to restart for.
pub unsafe fn prepare_library_path(name: &str) -> Result<Vec<PathBuf>, InitError> {
    config::validate_name(name)?;
    let root = crate::setup::dependency_root()?;
    let dirs = crate::setup::dependency_dirs(&root);

    // SAFETY: forwarded from this function's own contract — the first statement of `main`, no other thread.
    unsafe { rust_sak::ai::libpath::prepare(&dirs, REEXEC_GUARD) }.map_err(InitError::io(&root))?;

    Ok(dirs)
}
