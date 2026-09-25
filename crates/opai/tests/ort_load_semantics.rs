//! What `ort`'s process-wide load global actually does, pinned.
//!
//! Two decisions rest on this and neither can be reached through a real load on a runner with no ~175 MB runtime:
//! that the first library a process loads is the one it keeps (design D5), and that a load which *fails* leaves the
//! process unable to try again (design D9).
//!
//! An integration test, and the only one in its binary, because the global is process-wide: Cargo runs a crate's unit
//! tests in one binary on concurrent threads, so a process gets exactly one first load attempt and which test spent
//! it would be whichever thread won.
//!
//! **If this test fails, `ort` has most likely fixed the defect below — this crate has not broken.** The guard in
//! `crates/opai/src/runtime.rs` would then be redundant rather than wrong, and design D9 is what to re-read before
//! removing it.
// This file exists to pin `ort`'s own load-global behaviour, so it calls the banned method deliberately: it measures
// the defect the ban exists because of, in a process of its own with nothing else loaded.
#![allow(clippy::disallowed_methods)]

use std::path::Path;

#[test]
fn a_failed_load_leaves_the_process_reporting_success_for_every_later_one() {
    let first = Path::new("/no/such/directory/first-onnxruntime.so");

    // What a missing or unreadable runtime does on a first attempt, and the only honest answer of the three.
    let error = ort::init_from(first).err().expect("loading a path that does not exist must fail");
    let message = error.to_string();
    assert!(message.contains("first-onnxruntime.so"), "the error must name the path it tried: {message}");

    // The defect. `ort` loads through a `OnceLock` built on `Once::call_once_force`, which marks the `Once` completed
    // whenever its closure returns without panicking — a returned `Err` included. Nothing is written into the slot,
    // but `get` consults `is_completed()` and hands back a reference to uninitialized memory. So a second call
    // reports success having loaded nothing, whatever path it is given.
    let second = ort::init_from(Path::new("/no/such/directory/second-onnxruntime.so"));
    assert!(second.is_ok(), "ort no longer treats a failed load as a completed one; re-read design D9");

    // Not even a file that exists and is definitely not a library, which is what makes it a property of the poisoned
    // global rather than of anything about the path. Written out rather than pointed at a source file, so that it is
    // an absolute path that really exists on every runner — `ort` resolves a relative one against the executable's
    // directory, which would make this the missing-file case again on a platform where the test's working directory
    // is not what it was assumed to be.
    let dir = tempfile::tempdir().unwrap();
    let existing_but_not_a_library = dir.path().join("onnxruntime-shaped-but-not-a-library");
    std::fs::write(&existing_but_not_a_library, b"this is not a shared library").unwrap();
    assert!(existing_but_not_a_library.is_file());

    assert!(
        ort::init_from(&existing_but_not_a_library).is_ok(),
        "ort no longer treats a failed load as a completed one; re-read design D9"
    );

    // Deliberately not used: calling into the API through what those `Ok`s hand back is undefined behaviour, and in
    // practice panics inside `ort` with `dlsym(0x0, OrtGetApiBase): invalid handle`. That it cannot safely be used is
    // the whole reason `runtime::start` records the first failure instead of asking `ort` a second time.
}
