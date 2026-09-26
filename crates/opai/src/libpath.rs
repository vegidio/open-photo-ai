//! Putting the installed library directories where the dynamic loader will look for them.
//!
//! Two platforms, two mechanisms. Windows' `LoadLibrary` consults `PATH` at call time, so extending it in-process is
//! enough. glibc reads `LD_LIBRARY_PATH` once at exec time, so on Linux nothing a running process writes into it can
//! change a later `dlopen` — the process has to be replaced with one started under the corrected value.
//!
//! Everything except the `execvp` itself is a pure function of its arguments, which is what makes the rules below
//! testable without an exec, an environment, or a GPU.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use crate::config;
use crate::deps::artifact;
use crate::error::InitError;

/// The CUDA driver stub WSL2 exposes for the Windows NVIDIA driver. Its presence is what says this is WSL2 with an
/// NVIDIA GPU.
#[cfg(target_os = "linux")]
const WSL_CUDA_STUB: &str = "/usr/lib/wsl/lib/libcuda.so.1";

/// The Windows driver store as WSL2 mounts it: one directory per driver package, old versions included.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const WSL_DRIVER_STORE: &str = "/usr/lib/wsl/drivers";

/// The variable whose presence says this process is already the replacement, so the re-exec happens at most once
/// however many times the entry point runs.
///
/// Application-specific rather than the Go original's `APP_REEXEC`: that name belonged to `go-sak`'s general-purpose
/// `os.ReExec`, nothing here owns it, and a generic one can collide with another program's re-exec guard in an
/// inherited environment.
// Only the Linux arm of `apply` restarts, so only it reads this; the unit tests below still pin the name everywhere.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const REEXEC_GUARD: &str = "OPAI_REEXEC";

/// Creates every library directory under `app_dir` and returns them in search-path order.
///
/// The directories and their order are read off [`artifact::ALL`] rather than listed again here. A second list would
/// be the one place a dependency could be installed and still never reach the loader, and a search-path entry that is
/// missing when the process starts is skipped for the rest of the process's life — so the symptom would be a provider
/// that will not attach, long after the edit that caused it.
///
/// They are created whether or not anything has been installed into them yet, because a loader that finds a
/// search-path entry missing at startup skips it for the rest of the process's life — which would leave a library
/// downloaded later in the same session unfindable.
///
/// # Errors
///
/// Returns [`InitError::InvalidName`] if `name` cannot be a directory and [`InitError::Io`] if a directory could not
/// be created. Unlike the Go original, which logged a directory it could not create and carried on with the rest,
/// this fails: a search path silently missing one entry is the failure that shows up much later as a provider that
/// will not attach for reasons that look nothing like a directory nobody could create at startup.
pub(crate) fn library_dirs(app_dir: &Path, name: &str) -> Result<Vec<PathBuf>, InitError> {
    artifact::ALL.iter().map(|release| config::sub_dir(app_dir, name, release.dir)).collect()
}

/// Composes the search path: `dirs` first, then whatever `inherited` already held.
///
/// Ours take precedence because they are version-matched to the runtime and a system-wide install of a different
/// version must not win. Inherited entries are preserved rather than discarded — a user who set the variable meant
/// it. Duplicates are dropped keeping the first occurrence, which is what makes "ours first" survive an inherited
/// value that names one of the same directories, and empty entries are dropped because an empty entry means the
/// current directory to the loader.
// Read by the Linux and Windows arms of `apply`. macOS has no search path to establish — no NVIDIA library is
// published for it — but the rules are still covered by the unit tests below on every platform.
#[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
pub(crate) fn search_path(dirs: &[PathBuf], inherited: Option<&OsStr>) -> OsString {
    let mut entries: Vec<PathBuf> = Vec::with_capacity(dirs.len());

    let inherited = inherited.map(std::env::split_paths).into_iter().flatten();
    for entry in dirs.iter().cloned().chain(inherited) {
        if entry.as_os_str().is_empty() || entries.contains(&entry) {
            continue;
        }
        entries.push(entry);
    }

    // `join_paths` only fails on an entry containing the platform's own separator, which cannot come out of
    // `split_paths` and cannot be in a path we just created. An entry that somehow did would be dropped rather than
    // allowed to turn one entry into two.
    std::env::join_paths(&entries).unwrap_or_else(|_| {
        let kept: Vec<&PathBuf> = entries.iter().filter(|e| std::env::join_paths([*e]).is_ok()).collect();
        std::env::join_paths(kept).unwrap_or_default()
    })
}

/// Establishes the dynamic loader's search path for the application's library directories, and on Linux restarts the
/// process under it.
///
/// Call this as the **first statement of `main`**, before the logging setup, before any runtime is started, and
/// before any other thread exists. What it does per platform:
///
/// - **Linux**: composes `LD_LIBRARY_PATH` and replaces the process image with one started under it, at most once per
///   process tree. glibc reads that variable when the process starts, so appending to it later — after the libraries
///   have been downloaded — changes nothing about the `dlopen` calls the runtime makes. On success this **does not
///   return**. A replacement that fails is reported and startup continues under the inherited path, which costs the
///   GPU providers rather than the launch. Under WSL2 with an NVIDIA GPU, the active Windows driver's directory is
///   added after the application's own — see [`wsl_cuda_driver_dir`].
/// - **Windows**: prepends the directories to `PATH`, which `LoadLibrary` consults at call time, so no restart is
///   needed.
/// - **macOS**: creates the directories and returns. No NVIDIA library is published for it, and the CoreML provider
///   ships inside the runtime archive.
///
/// The directories are created on every platform, since a loader permanently skips a search-path entry that is
/// missing when the process starts.
///
/// # Safety
///
/// Must be called before any other thread exists — the first statement of `main`. It mutates the process environment,
/// which races with any other thread reading it, and on Linux it replaces the process image: file descriptors survive
/// `execvp`, so a restart after a logging setup has redirected stderr into a pipe would leave the new image writing
/// into a pipe whose reader died with the old process.
///
/// # Errors
///
/// Returns [`InitError::InvalidName`] if `name` cannot be a directory, and [`InitError::Fs`] or [`InitError::Io`] if
/// the configuration directory or a library directory could not be created. A failed restart is **not** an error: it
/// is reported and the library directories are returned as though there had been nothing to restart for.
pub unsafe fn prepare_library_path(name: &str) -> Result<Vec<PathBuf>, InitError> {
    let dirs = library_dirs(&config::app_dir(name)?, name)?;

    // SAFETY: the caller's contract is that no other thread exists yet, which is what makes reading and writing the
    // process environment sound. Each platform arm below relies on exactly that.
    unsafe { apply(&dirs) };

    Ok(dirs)
}

/// The per-platform half of [`prepare_library_path`].
///
/// # Safety
///
/// As [`prepare_library_path`]: no other thread may exist.
#[cfg(target_os = "linux")]
unsafe fn apply(dirs: &[PathBuf]) {
    use std::os::unix::process::CommandExt;

    // Before any work: the child would otherwise recompose the path and report a restart that never happens.
    if std::env::var_os(REEXEC_GUARD).is_some_and(|value| value == "1") {
        return;
    }

    // The two `eprintln!`s below stay `eprintln!` and are **not** `tracing` records, which is the one place in this
    // crate that is deliberately outside the log.
    //
    // They run before a sink can exist. `prepare_library_path` is the first statement of `main`, ahead of
    // `logging::init`, because it may replace the process image with `exec` — and a record written here would put a
    // session divider and a header into the file for a process that is about to stop existing, then do both again in
    // the replacement. The reference implementation writes the same thing down twice for the same reason.
    //
    // The cost is that on the one platform that restarts, a failure to restart reaches a stderr a bundled application
    // does not have. That is the failure this function already treats as survivable — the application starts on the
    // CPU — so what is lost is the explanation rather than the behaviour, and buying it would mean a log file written
    // twice on every Linux launch.

    let Ok(program) = std::env::current_exe() else {
        eprintln!("opai: could not find this executable to restart it; the NVIDIA libraries will not be found");
        return;
    };

    // `exec` replaces the process image and does not return on success, so everything after this line is the failure
    // path. Reported rather than fatal: a machine that cannot exec its own binary has worse problems than an
    // unfindable CUDA, and startup on the CPU is better than no startup at all.
    let mut ours = dirs.to_vec();
    ours.extend(wsl_cuda_driver_dir());

    let error = std::process::Command::new(program)
        .args(std::env::args_os().skip(1))
        .env("LD_LIBRARY_PATH", search_path(&ours, std::env::var_os("LD_LIBRARY_PATH").as_deref()))
        .env(REEXEC_GUARD, "1")
        .exec();

    eprintln!("opai: could not restart with LD_LIBRARY_PATH set; the NVIDIA libraries will not be found: {error}");
}

/// Under WSL2, the directory of the Windows NVIDIA driver that CUDA actually uses, so it can go on the search path.
///
/// WSL2's `libcuda.so.1` is a stub that finds the real driver, `libcuda.so.1.1`, in the Windows driver store and loads
/// it — and that driver then loads its PTX JIT compiler by soname alone, `libnvidia-ptxjitcompiler.so.1`. The
/// compiler it needs sits beside it in the driver store, which is not on any search path, so the loader falls through
/// to its cache. On a distribution with a native NVIDIA driver package installed — Ubuntu's `nvidia-cuda-toolkit`
/// pulls in `libnvidia-compute-*` — the cache hands it that package's compiler, from a different driver version, and
/// the first CUDA call segfaults inside it. Putting the driver's own directory on the path makes its compiler win.
///
/// The directory is asked for rather than guessed: the driver store keeps old driver versions beside the current one,
/// and the compiler has to match the driver exactly. `cuInit` is what makes the stub load the real driver, and
/// `/proc/self/maps` then says where it came from. That costs a CUDA initialisation, around 60 ms, on WSL2 with an
/// NVIDIA GPU and nothing anywhere else. The compiler itself is not loaded by `cuInit`, so this is safe to call on
/// exactly the machines it exists for.
///
/// The driver is never unloaded: a driver is not written to be, and the process is about to be replaced anyway.
#[cfg(target_os = "linux")]
fn wsl_cuda_driver_dir() -> Option<PathBuf> {
    use std::ffi::{c_int, c_uint};

    if !Path::new(WSL_CUDA_STUB).is_file() {
        return None;
    }

    // SAFETY: loading the stub runs its initialisers, which it ships precisely for a process to load it.
    let library = unsafe { libloading::Library::new(WSL_CUDA_STUB) }.ok()?;

    // The status is not read: a failed initialisation still says where the driver was loaded from if it got that far,
    // and one that never loaded it leaves nothing in the maps and nothing to add.
    //
    // SAFETY: `cuInit` has this signature in `cuda.h`, and the symbol does not outlive `library`, which is never
    // unloaded. `0` is the only flag value it accepts.
    unsafe {
        let init: libloading::Symbol<unsafe extern "C" fn(c_uint) -> c_int> = library.get(b"cuInit\0").ok()?;
        init(0);
    }
    std::mem::forget(library);

    driver_dir_in_maps(&std::fs::read_to_string("/proc/self/maps").ok()?)
}

/// The driver-store directory the real CUDA driver was mapped from, read from a `/proc/<pid>/maps` listing.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn driver_dir_in_maps(maps: &str) -> Option<PathBuf> {
    maps.lines()
        // The path is the last column and the only one that contains a `/`; it may itself contain spaces.
        .filter_map(|line| line.find('/').map(|start| Path::new(&line[start..])))
        .find(|path| path.starts_with(WSL_DRIVER_STORE) && path.file_name() == Some(OsStr::new("libcuda.so.1.1")))
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}

/// The Windows half: `LoadLibrary` consults `PATH` when it is called rather than when the process starts, so the
/// variable is simply extended in place.
///
/// # Safety
///
/// As [`prepare_library_path`]: no other thread may exist, which is what makes [`std::env::set_var`] sound.
#[cfg(target_os = "windows")]
unsafe fn apply(dirs: &[PathBuf]) {
    let path = search_path(dirs, std::env::var_os("PATH").as_deref());

    // SAFETY: the caller's contract is that no other thread exists yet, so nothing can be reading the environment
    // while this writes to it.
    unsafe { std::env::set_var("PATH", path) };
}

/// Every other platform, macOS included: the directories have been created and there is nothing to point at them.
///
/// # Safety
///
/// As [`prepare_library_path`], though this arm touches nothing.
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
unsafe fn apply(_dirs: &[PathBuf]) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Joins `entries` the way the platform spells a search path, so an expectation reads the same on Windows.
    fn joined(entries: &[&str]) -> OsString {
        std::env::join_paths(entries.iter().map(Path::new)).unwrap()
    }

    #[test]
    fn our_directories_come_first_and_inherited_entries_are_kept() {
        let ours = [PathBuf::from("/opt/app/runtime"), PathBuf::from("/opt/app/libs/cuda")];
        let inherited = joined(&["/usr/lib", "/usr/local/lib"]);

        let path = search_path(&ours, Some(inherited.as_os_str()));

        // Ours are version-matched to the runtime, so a system-wide install of a different version must not win; the
        // user's own entries are preserved rather than discarded.
        assert_eq!(path, joined(&["/opt/app/runtime", "/opt/app/libs/cuda", "/usr/lib", "/usr/local/lib"]));
    }

    #[test]
    fn a_directory_already_inherited_keeps_its_place_at_the_front() {
        // The duplicate rule is what makes "ours first" survive an inherited value that names the same directory: if
        // the later occurrence won, an inherited entry would demote the version-matched one.
        let ours = [PathBuf::from("/opt/app/runtime"), PathBuf::from("/opt/app/libs/cuda")];
        let inherited = joined(&["/usr/lib", "/opt/app/runtime", "/usr/lib"]);

        let path = search_path(&ours, Some(inherited.as_os_str()));

        assert_eq!(path, joined(&["/opt/app/runtime", "/opt/app/libs/cuda", "/usr/lib"]));
    }

    #[test]
    fn a_directory_named_twice_by_us_appears_once() {
        let ours = [PathBuf::from("/opt/app/runtime"), PathBuf::from("/opt/app/runtime")];

        assert_eq!(search_path(&ours, None), joined(&["/opt/app/runtime"]));
    }

    #[test]
    fn an_empty_inherited_entry_is_dropped() {
        // An empty entry means the current working directory to the loader, which is not something to search for a
        // shared library.
        let ours = [PathBuf::from("/opt/app/runtime")];
        let mut inherited = OsString::from("");
        inherited.push(joined(&["", "/usr/lib"]));

        let path = search_path(&ours, Some(inherited.as_os_str()));

        assert_eq!(path, joined(&["/opt/app/runtime", "/usr/lib"]));
    }

    #[test]
    fn nothing_inherited_yields_our_directories_alone() {
        let ours = [PathBuf::from("/opt/app/runtime"), PathBuf::from("/opt/app/libs/cuda")];

        assert_eq!(search_path(&ours, None), joined(&["/opt/app/runtime", "/opt/app/libs/cuda"]));
        assert_eq!(search_path(&ours, Some(OsStr::new(""))), joined(&["/opt/app/runtime", "/opt/app/libs/cuda"]));
    }

    #[test]
    fn no_directories_and_nothing_inherited_yields_an_empty_path() {
        assert_eq!(search_path(&[], None), OsString::new());
    }

    #[test]
    fn the_four_directories_are_created_in_search_path_order() {
        let root = tempfile::tempdir().unwrap();
        let app_dir = root.path().join("opai-test");

        let dirs = library_dirs(&app_dir, "opai-test").unwrap();

        assert_eq!(
            dirs,
            vec![
                app_dir.join("runtime"),
                app_dir.join("libs").join("cuda"),
                app_dir.join("libs").join("cudnn"),
                app_dir.join("libs").join("tensorrt"),
            ]
        );
        // Created even though nothing has been installed into them: a loader permanently skips a search-path entry
        // that is missing when the process starts, so a library downloaded later in the same session would never be
        // found.
        for dir in &dirs {
            assert!(dir.is_dir(), "{} was not created", dir.display());
        }
    }

    #[test]
    fn creating_the_directories_again_keeps_what_is_already_installed() {
        let root = tempfile::tempdir().unwrap();
        let app_dir = root.path().join("opai-test");

        let first = library_dirs(&app_dir, "opai-test").unwrap();
        std::fs::write(first[0].join("onnxruntime.so"), b"installed").unwrap();
        let second = library_dirs(&app_dir, "opai-test").unwrap();

        assert_eq!(first, second);
        assert!(second[0].join("onnxruntime.so").exists(), "a second call emptied an install");
    }

    /// A `/proc/self/maps` excerpt from a WSL2 process after `cuInit`, trimmed to the lines that matter.
    const WSL_MAPS: &str = "\
7b64646d4000-7b64646d5000 r--p 00000000 00:3f 2533274790988457           /usr/lib/wsl/lib/libcuda.so.1
7b612e6c0000-7b612e6c1000 r--p 00000000 00:40 1125899906950443           /usr/lib/wsl/drivers/nv_dispsi.inf_amd64_d95662815b9b13a8/libcuda.so.1.1
7b612e6c1000-7b612f854000 r-xp 00001000 00:40 1125899906950443           /usr/lib/wsl/drivers/nv_dispsi.inf_amd64_d95662815b9b13a8/libcuda.so.1.1
7b640f773000-7b640f774000 r--p 00000000 00:3f 1970324837567045           /usr/lib/wsl/lib/libdxcore.so
7ffd2c9f1000-7ffd2ca12000 rw-p 00000000 00:00 0                          [stack]
";

    #[test]
    fn the_active_driver_directory_is_where_the_real_cuda_driver_was_mapped_from() {
        assert_eq!(
            driver_dir_in_maps(WSL_MAPS),
            Some(PathBuf::from("/usr/lib/wsl/drivers/nv_dispsi.inf_amd64_d95662815b9b13a8"))
        );
    }

    #[test]
    fn no_driver_directory_before_the_real_driver_is_loaded() {
        // The stub alone, which is all a failed `cuInit` leaves: there is no driver to point the loader at.
        let stub_only = "7b64646d4000-7b64646d5000 r--p 00000000 00:3f 25332 /usr/lib/wsl/lib/libcuda.so.1\n";

        assert_eq!(driver_dir_in_maps(stub_only), None);
        assert_eq!(driver_dir_in_maps(""), None);
    }

    #[test]
    fn a_cuda_driver_outside_the_driver_store_is_not_a_wsl_driver() {
        // A native Linux driver has nothing to add: its compiler is already the one the loader finds.
        let native = "7b612e6c0000-7b612e6c1000 r--p 00000000 08:20 4242 /usr/lib/x86_64-linux-gnu/libcuda.so.1.1\n";

        assert_eq!(driver_dir_in_maps(native), None);
    }

    #[test]
    fn other_files_in_the_driver_store_are_not_the_driver() {
        // `libcuda_loader.so` is the stub's twin in the store, and old driver versions keep theirs too: only the real
        // driver that was mapped says which version is active.
        let loader = "7b612e6c0000-7b612e6c1000 r--p 00000000 00:40 42 /usr/lib/wsl/drivers/nv_dispi.inf_amd64_0ld/libcuda_loader.so\n";

        assert_eq!(driver_dir_in_maps(loader), None);
    }

    #[test]
    fn the_restart_guard_is_application_specific() {
        // Part of the contract rather than an implementation detail: the integration tests set it by name, and a
        // generic one — the Go original used `APP_REEXEC` — can collide with another program's re-exec guard in an
        // inherited environment.
        assert_eq!(REEXEC_GUARD, "OPAI_REEXEC");
    }

    #[test]
    fn a_name_that_cannot_be_a_directory_is_rejected_before_anything_is_created() {
        let root = tempfile::tempdir().unwrap();

        assert!(library_dirs(&root.path().join("x"), "..").is_err());
        assert!(
            std::fs::read_dir(root.path()).unwrap().next().is_none(),
            "a rejected name left something behind"
        );
    }
}
