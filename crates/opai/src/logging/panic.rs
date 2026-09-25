//! Getting a panic into the log before the process ends.
//!
//! This is the one thing the reference implementation's stderr capture gave that ONNX Runtime's callback does not.
//! That capture owned file descriptor 2, so the Go runtime's own crash output landed in `native.log` and was folded
//! back into the log on the next launch. Here nothing owns descriptor 2, and a Rust panic's message would otherwise
//! reach a terminal that a double-clicked `.app` bundle does not have.
//!
//! # It adds a record and changes nothing else
//!
//! The hook that was there is called afterwards, so the default message on stderr, the backtrace and the unwind or
//! abort all happen exactly as they would have. Chaining rather than replacing also means a front end that installed
//! its own hook — Tauri does not, but a consumer of this library might — keeps it.
//!
//! # What it does not cover
//!
//! A hard native abort, and output from a native library that writes to stderr and is not ONNX Runtime — a CUDA or
//! cuDNN loader message, a driver warning. The reference implementation caught those by owning the descriptor, and
//! this deliberately does not: the failure that motivated the capture is *better* covered here, because ORT's own
//! account of a native crash is written synchronously as it is produced rather than replayed from a second file on
//! the next run. Recorded as a known parity gap in `openspec` design D9.

use std::panic::PanicHookInfo;

/// Installs the record-then-default panic hook.
///
/// Called from [`init`](super::init), after the subscriber, since a hook installed before it would emit into a
/// process that has nowhere to put the record.
pub(super) fn install_hook() {
    let previous = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        record(info);
        previous(info);
    }));
}

/// Emits the error record for one panic.
///
/// Separated from the hook so the rendering can be tested without installing anything process-wide — a hook is global
/// state that every other test in the binary would then be running under.
fn record(info: &PanicHookInfo<'_>) {
    // `&str` for a literal panic message and `String` for a formatted one; anything else is a payload no formatting
    // can be assumed of, and is reported as the fact that there was one rather than guessed at.
    let payload = info.payload();
    let message = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("panicked with a payload that is not a string");

    // The location is where the `panic!` was written, which is the line a reader needs and is not otherwise anywhere
    // in the record. It is `None` only for a panic raised somewhere that has no caller location at all.
    let location = info.location().map(ToString::to_string).unwrap_or_else(|| "unknown".to_owned());

    // The payload goes in the **message** rather than into a field of its own. `message` is the one field the
    // formatter renames, to `msg`, so a `msg = ..` field here would put two `msg=` on one line — and `tracing` orders
    // the format message first, so the one a reader and a `grep msg=` found would be the literal rather than the
    // panic. One `msg=` per record is the format's contract with whoever reads the file; see `logging::format`.
    //
    // The `panicked:` prefix is what the separate literal was carrying. It keeps a panic findable by its text rather
    // than only by its level and its target.
    tracing::error!(location = %location, "panicked: {message}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::CLI;
    use crate::logging::format::subscriber;
    use crate::logging::writer::{Rotator, Sink};
    use std::sync::{Mutex, MutexGuard};
    use tracing::subscriber::with_default;
    use tracing_subscriber::EnvFilter;

    /// Serialises the tests that swap the process's panic hook.
    ///
    /// The hook is one global, so two of these running at once would each take the other's hook as "the previous
    /// one" and put it back in the wrong order — which shows up as a panic escaping a `catch_unwind` in whichever
    /// test lost the race, rather than as anything to do with what is being tested.
    static HOOK: Mutex<()> = Mutex::new(());

    /// Takes that lock, recovering it from a test that panicked while holding it — which, here, most of them do.
    fn serialised() -> MutexGuard<'static, ()> {
        HOOK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Catches an unwinding panic raised by `raise` and returns the log file the hook's record landed in.
    ///
    /// The hook is installed for the duration of the call and taken off again, so the rest of the suite is not run
    /// under it — and the subscriber is a scoped one for the same reason.
    fn log_of_panic(raise: impl FnOnce() + std::panic::UnwindSafe) -> String {
        let _serialised = serialised();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opai.log");
        let sink = Sink::new(Rotator::open(&path));

        with_default(subscriber(CLI, sink, EnvFilter::new("info")), || {
            let previous = std::panic::take_hook();

            // Silences the default hook's own stderr line for the duration, so a deliberate panic does not look like
            // a failing test in the suite's output. The record-then-delegate shape is what is under test, and the
            // chaining is asserted separately below.
            std::panic::set_hook(Box::new(record));
            let outcome = std::panic::catch_unwind(raise);
            std::panic::set_hook(previous);

            assert!(outcome.is_err(), "the test's own panic did not unwind");
        });

        std::fs::read_to_string(&path).unwrap()
    }

    #[test]
    fn a_panic_is_recorded_with_its_message_and_its_location() {
        let line = log_of_panic(|| panic!("the model registry lost a session"));

        assert!(line.contains("level=ERROR"), "a panic is not at error severity: {line}");
        assert!(line.contains(r#"msg="panicked: the model registry lost a session""#), "{line}");

        // Exactly one, which is what stops a reader — or a `grep msg=` over a file somebody attached — from finding a
        // marker word where the panic's own text should be. The payload is in the message for that reason.
        assert_eq!(line.matches("msg=").count(), 1, "a record carries more than one message: {line}");

        // The source location of the `panic!` above, by file rather than by line, so that editing this file does not
        // fail the test for the wrong reason.
        //
        // Built rather than spelled, because neither half of it is the same on every platform. The location arrives
        // from `std` as the compiler wrote the path, so the separator is the host's; and a Windows path contains `\`,
        // which is exactly what makes the formatter quote the value and escape it - so the record carries
        // `logging\\panic.rs` there and a bare `logging/panic.rs` everywhere else.
        let file = std::path::Path::new("logging").join("panic.rs").display().to_string();
        let file = file.replace('\\', r"\\");
        assert!(line.contains(&file), "the record carries no source location: {line}");
        assert!(line.contains("location="), "{line}");
    }

    #[test]
    fn a_formatted_panic_message_is_recorded_rather_than_its_type() {
        // `panic!("{}", ..)` produces a `String` payload rather than the `&str` a literal gives, and the two arrive
        // at the hook as different types — which is why both are downcast.
        let what = "cuda";
        let line = log_of_panic(move || panic!("{what} declined to attach"));

        assert!(line.contains(r#"msg="panicked: cuda declined to attach""#), "{line}");
    }

    #[test]
    fn a_panic_with_a_payload_that_is_not_a_string_still_produces_a_record() {
        let line = log_of_panic(|| std::panic::panic_any(42_u8));

        assert!(line.contains("level=ERROR"), "{line}");
        assert!(line.contains("payload"), "a non-string payload produced no record: {line}");
    }

    #[test]
    fn the_hook_that_was_there_still_runs() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        let _serialised = serialised();

        // The property the chaining exists for: the default message, the backtrace and the unwind are unchanged, and
        // only the record is added. Stood in for by a hook of the test's own, since the real default's effect is a
        // line on stderr that a test cannot read back.
        let ran = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&ran);

        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |_| flag.store(true, Ordering::SeqCst)));

        install_hook();
        let outcome = std::panic::catch_unwind(|| panic!("through both hooks"));
        std::panic::set_hook(previous);

        assert!(outcome.is_err());
        assert!(ran.load(Ordering::SeqCst), "the hook that was already installed did not run");
    }
}
