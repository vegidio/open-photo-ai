//! `prepare_library_path` against the real configuration directory.
//!
//! An integration test, and the only one in its binary, because it mutates the process environment — the Windows arm
//! extends `PATH` — and `prepare_library_path`'s safety contract is that no other thread exists while it runs. In the
//! unit-test binary the harness runs tests concurrently and several of them read the environment to resolve a
//! configuration directory, which is exactly the race the contract rules out.

use std::path::PathBuf;

/// An application name nothing else uses, so the directories this creates are unambiguously its own.
const APP: &str = "opai-prepare-library-path-test";

#[test]
fn the_directories_are_created_and_the_restart_guard_short_circuits() {
    // Set first: on Linux `prepare_library_path` would otherwise replace this test binary's process image, and the
    // run would restart from the top rather than reaching a single assertion below. That the call returns at all is
    // the assertion — the guard is what makes the restart happen at most once however many times the entry point is
    // reached.
    //
    // SAFETY: this is the only test in this binary, so no other thread exists to observe the write.
    unsafe { std::env::set_var("OPAI_REEXEC", "1") };

    // SAFETY: as above — no other thread exists in this process.
    let dirs = unsafe { opai::prepare_library_path(APP) }.unwrap();

    // The four version folders of the shared dependency root, in install order, whatever the application's name.
    let root = dirs[0].ancestors().nth(2).unwrap().to_path_buf();
    assert!(root.ends_with("io.vinicius.ai"), "{}", root.display());
    let expected: Vec<PathBuf> = ["onnx/1.30.0", "cuda/13.3.0", "cudnn/9.23.1", "tensorrt/10.14.1"]
        .iter()
        .map(|sub| sub.split('/').fold(root.clone(), |path, part| path.join(part)))
        .collect();

    assert_eq!(dirs, expected, "the library folders were not the four in search-path order");

    // Created where a loader reads them, whether or not anything has been installed into them yet: a loader that
    // finds a search-path entry missing at startup skips it for the rest of the process's life.
    if cfg!(any(target_os = "linux", target_os = "windows")) {
        for dir in &dirs {
            assert!(dir.is_dir(), "{} was not created", dir.display());
        }
    }

    #[cfg(target_os = "windows")]
    {
        // `LoadLibrary` consults `PATH` when it is called rather than when the process starts, so the extension is
        // made in place and must be visible immediately.
        let path = std::env::var_os("PATH").unwrap();
        let entries: Vec<PathBuf> = std::env::split_paths(&path).collect();
        assert_eq!(&entries[..dirs.len()], &dirs[..], "the library directories are not ahead of the inherited PATH");
    }
}
