//! The one thing in `prepare_library_path` that no unit test can reach: the real `execvp`.
//!
//! Everything around it — which directories go on the path, in what order, with what deduplication — is a pure
//! function covered in `libpath.rs`. What is left is a call that on success never returns, so it is exercised here by
//! having the test binary restart *itself*: the child runs this same test under a marker variable,
//! `prepare_library_path` replaces its process image, and the replacement reports the environment it came up
//! with.
//!
//! Linux only, because Linux is the only platform that restarts: glibc reads `LD_LIBRARY_PATH` once at exec time,
//! where Windows' `LoadLibrary` consults `PATH` at call time and macOS has no NVIDIA library to find.
#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Command;

/// An application name nothing else uses, so the directories this creates are unambiguously its own.
const APP: &str = "opai-reexec-test";

/// Set by the parent in the child's environment; its presence is what turns a run of this test into the child half.
const CHILD: &str = "OPAI_REEXEC_TEST_CHILD";

/// Where the child records each time it starts, so an exec loop fails with an assertion instead of hanging CI.
const TALLY: &str = "OPAI_REEXEC_TEST_TALLY";

/// An inherited entry the composed path has to preserve, and to put *after* the application's own directories.
const INHERITED: &str = "/opt/opai-inherited-sentinel";

/// How many times the child may start before it is declared to be looping: once as launched, once as re-execed.
const MAX_RUNS: usize = 2;

#[test]
fn the_process_restarts_once_under_the_composed_library_path() {
    if std::env::var_os(CHILD).is_some() {
        return child();
    }

    let root = tempfile::tempdir().unwrap();
    let tally = root.path().join("runs");
    std::fs::write(&tally, b"").unwrap();

    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "the_process_restarts_once_under_the_composed_library_path", "--nocapture"])
        .env(CHILD, "1")
        .env(TALLY, &tally)
        .env("LD_LIBRARY_PATH", INHERITED)
        // Whatever this process inherited, the child starts as though it had never been restarted.
        .env_remove("OPAI_REEXEC")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "the child failed\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}");

    // Two starts: the one the parent launched, and the one `execvp` produced. A third would mean the guard did not
    // take, which is the failure this bounds rather than lets run forever.
    let runs = stdout.lines().filter(|line| *line == "RUN").count();
    assert_eq!(runs, MAX_RUNS, "the process restarted {} time(s)\n{stdout}", runs.saturating_sub(1));

    let composed = line_after(&stdout, "LDPATH=").expect("the restarted process reported no LD_LIBRARY_PATH");
    let dirs: Vec<PathBuf> = line_after(&stdout, "DIRS=")
        .expect("the restarted process reported no library directories")
        .split(':')
        .map(PathBuf::from)
        .collect();
    let entries: Vec<PathBuf> = composed.split(':').map(PathBuf::from).collect();

    // The process is running under the composed path rather than the one it was started with: the inherited value was
    // exactly the sentinel, so anything more than that came from the restart.
    assert_ne!(composed, INHERITED, "the process is still running under the path it was started with");
    assert_eq!(dirs.len(), 4, "expected the runtime and the three library directories, got {dirs:?}");
    assert_eq!(&entries[..4], &dirs[..], "the library directories are not at the front of {composed}");
    assert_eq!(
        entries.get(4).map(PathBuf::as_path),
        Some(std::path::Path::new(INHERITED)),
        "the inherited entry was dropped or reordered in {composed}"
    );

    for dir in &dirs {
        assert!(dir.is_dir(), "{} was named on the search path but does not exist", dir.display());
    }

    let _ = std::fs::remove_dir_all(dirs[0].parent().unwrap());
}

/// The child half: report every start, restart once, and report what the replacement came up with.
fn child() {
    let tally = PathBuf::from(std::env::var_os(TALLY).expect("the child was launched without a tally file"));
    let runs = std::fs::read(&tally).unwrap().len() + 1;
    std::fs::write(&tally, vec![b'x'; runs]).unwrap();

    // `println!` writes through a `LineWriter`, so this reaches the pipe before the process image is replaced — which
    // is what makes the count above observable at all.
    println!("RUN");
    assert!(runs <= MAX_RUNS, "the restart guard did not take: the process started {runs} times");

    // SAFETY: the test harness has started no threads of its own for this single test, and nothing below reads the
    // environment concurrently. This is the one place the contract can be honoured inside a test at all, which is why
    // it runs in a process of its own.
    let dirs = unsafe { opai::prepare_library_path(APP) }.unwrap();

    // Only reached in the replacement: on the first run `prepare_library_path` does not return.
    let composed = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
    let dirs: Vec<String> = dirs.iter().map(|dir| dir.display().to_string()).collect();

    println!("LDPATH={composed}");
    println!("DIRS={}", dirs.join(":"));
}

/// The remainder of the first line starting with `prefix`.
fn line_after<'a>(stdout: &'a str, prefix: &str) -> Option<&'a str> {
    stdout.lines().find_map(|line| line.strip_prefix(prefix))
}
