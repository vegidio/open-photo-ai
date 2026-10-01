//! Where ONNX Runtime's own records go: the `tracing` bridge `ort` would install, with two things it cannot do.
//!
//! The records keep exactly the shape `ort-2.0.0-rc.13/src/logging.rs` gives them — an event under the target
//! `ort::logging` whose explicit parent is a TRACE span named `ort` carrying `id` and `location` — because the log's
//! formatter and the telemetry export both read that shape (see `logging::format::is_ort_record_span`). What this adds:
//!
//! - **A severity correction.** TensorRT reports a kernel tactic it skipped for want of scratch memory as an `ERROR`
//!   (`virtualMemoryBuffer.cpp::resizePhysical … OutOfMemory`) and then builds the engine without it. On an 8 GB card
//!   one engine build writes thousands of them, and the session comes up fine — so they are written at `WARN`.
//! - **An out-of-memory mark.** A failed build's own error says only that TensorRT *"failed to create engine"*; that
//!   the device ran out of memory is in a record logged on the way there. Each one is counted on the thread that logged
//!   it, which for TensorRT is the build's own thread, so [`out_of_memory_since`] can tell a build what happened in
//!   it. See `sessions::build`.

use std::cell::Cell;

use ort::logging::{LogLevel, LoggerFunction};

thread_local! {
    /// How many out-of-memory records the runtime has logged on this thread.
    ///
    /// `const`-initialized and without a destructor, so reading it from the callback is sound even while the thread
    /// is being torn down — a panic there would cross an `extern "system"` boundary and abort the process.
    static OUT_OF_MEMORY: Cell<u64> = const { Cell::new(0) };
}

/// The logger the environment is created with.
pub(super) fn logger() -> LoggerFunction {
    std::sync::Arc::new(|severity, _category, id, location, message| record(severity, id, location, message))
}

/// How many out-of-memory records this thread has seen so far, to hand back to [`out_of_memory_since`].
pub(crate) fn out_of_memory_mark() -> u64 {
    OUT_OF_MEMORY.with(Cell::get)
}

/// Whether the runtime logged an out-of-memory record on this thread since `mark` was taken.
pub(crate) fn out_of_memory_since(mark: u64) -> bool {
    out_of_memory_mark() != mark
}

/// Forwards one runtime record to `tracing`, as `ort`'s own bridge does.
fn record(severity: LogLevel, id: &str, location: &str, message: &str) {
    if is_out_of_memory(message) {
        OUT_OF_MEMORY.with(|count| count.set(count.get().wrapping_add(1)));
    }

    let span = tracing::span!(target: "ort::logging", tracing::Level::TRACE, "ort", id = id, location = location);

    match severity {
        LogLevel::Verbose => tracing::event!(target: "ort::logging", parent: &span, tracing::Level::TRACE, "{message}"),
        LogLevel::Info => tracing::event!(target: "ort::logging", parent: &span, tracing::Level::INFO, "{message}"),
        LogLevel::Warning => tracing::event!(target: "ort::logging", parent: &span, tracing::Level::WARN, "{message}"),
        LogLevel::Error if is_skipped_tactic(message) => {
            tracing::event!(target: "ort::logging", parent: &span, tracing::Level::WARN, "{message}");
        }
        LogLevel::Error => tracing::event!(target: "ort::logging", parent: &span, tracing::Level::ERROR, "{message}"),
        LogLevel::Fatal => {
            tracing::event!(target: "ort::logging", parent: &span, tracing::Level::ERROR, "(FATAL): {message}");
        }
    }
}

/// Whether `message` reports a device allocation that failed: TensorRT's `Error Code 2: OutOfMemory`, or the CUDA
/// provider's `out of memory`.
fn is_out_of_memory(message: &str) -> bool {
    message.contains("OutOfMemory") || message.contains("out of memory")
}

/// Whether `message` is TensorRT failing to grow the scratch memory of a tactic it then skips — which the builder
/// recovers from on its own, and which it logs as an error anyway.
fn is_skipped_tactic(message: &str) -> bool {
    message.contains("virtualMemoryBuffer.cpp::resizePhysical") && message.contains("OutOfMemory")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The record from the field: an RTX 3070 building Tokyo, whose session then came up on TensorRT.
    const SKIPPED_TACTIC: &str = "[2026-09-30 18:44:58   ERROR] [virtualMemoryBuffer.cpp::resizePhysical::153] Error \
                                  Code 2: OutOfMemory (Requested size was 2147483648 bytes.)";

    /// The record from the field that does fail the build: an RTX 5080 with Osaka resident.
    const FAILED_BUILD: &str = "[2026-09-27 06:51:55   ERROR] [globWriter.cpp::nvinfer1::builder::HybridGlobWriter::\
                                HybridGlobWriter::496] Error Code 2: OutOfMemory (Requested size was 1024 bytes.)";

    /// What `emit` writes to the log under the default directives, which are what keep `ort` at `warn`.
    fn captured(emit: impl FnOnce()) -> (String, ()) {
        crate::logging::records_of_blocking("info,ort=warn", emit)
    }

    #[test]
    fn a_skipped_tactic_is_written_as_a_warning() {
        let (line, ()) =
            captured(|| record(LogLevel::Error, "opai", "tensorrt_execution_provider.h:98", SKIPPED_TACTIC));

        assert!(line.contains("level=WARN"), "{line}");
        assert!(line.contains(" target=ort::logging "), "{line}");
        assert!(
            line.contains("location=tensorrt_execution_provider.h:98"),
            "the record lost its span fields: {line}"
        );
    }

    #[test]
    fn any_other_runtime_error_keeps_its_severity() {
        let (line, ()) = captured(|| record(LogLevel::Error, "opai", "trt.h:98", FAILED_BUILD));

        assert!(line.contains("level=ERROR"), "{line}");
    }

    #[test]
    fn an_out_of_memory_record_marks_the_thread_that_logged_it() {
        let mark = out_of_memory_mark();
        record(LogLevel::Warning, "opai", "here", "Some nodes were not assigned");
        assert!(!out_of_memory_since(mark), "a record about something else was counted");

        record(LogLevel::Error, "opai", "here", FAILED_BUILD);
        assert!(out_of_memory_since(mark));

        // Another thread's build is not this one's.
        let elsewhere = std::thread::spawn(|| {
            let mark = out_of_memory_mark();
            out_of_memory_since(mark)
        });
        assert!(!elsewhere.join().unwrap());
    }

    #[test]
    fn a_skipped_tactic_counts_as_running_out_of_memory() {
        // Only read when a build then fails, where memory being short is exactly what it says.
        let mark = out_of_memory_mark();
        record(LogLevel::Error, "opai", "here", SKIPPED_TACTIC);

        assert!(out_of_memory_since(mark));
    }
}
