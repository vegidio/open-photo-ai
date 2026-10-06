//! Where ONNX Runtime's own records go: the `tracing` bridge `ort` would install, with three things it cannot do.
//!
//! The records keep exactly the shape `ort-2.0.0-rc.13/src/logging.rs` gives them — an event under the target
//! `ort::logging` whose explicit parent is a TRACE span named `ort` carrying `id` and `location` — because the log's
//! formatter and the telemetry export both read that shape (see `logging::format::is_ort_record_span`). What this adds:
//!
//! - **A severity correction.** TensorRT reports a kernel tactic it skipped for want of scratch memory as an `ERROR`
//!   (`virtualMemoryBuffer.cpp::…resizePhysical … OutOfMemory`) and then builds the engine without it. The session
//!   comes up fine, so they are written at `WARN`.
//! - **A repeat throttle.** A runtime short of device memory says so once per allocation it tries: one TensorRT build
//!   on a 6 GB card wrote over a thousand `resizePhysical` records, and a WebGPU device that ran out wrote the same
//!   line a dozen times in one microsecond. A `WARN` or `ERROR` record like one written in the last
//!   [`REPEAT_WINDOW`] is held back and counted instead, whatever its level, and the count is written as one record
//!   when the window closes or a session build ends. See [`Throttle`].
//! - **An out-of-memory mark.** A failed build's own error says only that TensorRT *"failed to create engine"*; that
//!   the device ran out of memory is in a record logged on the way there. Each one is counted on the thread that logged
//!   it, which for TensorRT is the build's own thread, so [`out_of_memory_since`] can tell a build what happened in
//!   it. Held-back records are counted too. See `sessions::build`.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};

use ort::logging::{LogLevel, LoggerFunction};
use tracing::Level;

thread_local! {
    /// How many out-of-memory records the runtime has logged on this thread.
    ///
    /// `const`-initialized and without a destructor, so reading it from the callback is sound even while the thread
    /// is being torn down — a panic there would cross an `extern "system"` boundary and abort the process.
    static OUT_OF_MEMORY: Cell<u64> = const { Cell::new(0) };
}

/// How long a record is held back for after one like it was written.
///
/// Long enough to swallow a whole burst, short enough that a condition that persists is still written about every
/// minute rather than once per process.
const REPEAT_WINDOW: Duration = Duration::from_secs(60);

/// The throttle every runtime record goes through.
static THROTTLE: LazyLock<Mutex<Throttle>> = LazyLock::new(Mutex::default);

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

/// Writes how many records were held back since each was last written, and starts every window afresh.
///
/// Called when a session build ends, which is where a burst ends: without it the tally of the last burst would wait
/// for the next record that happens to be logged.
pub(crate) fn flush() {
    flush_through(&THROTTLE);
}

/// Forwards one runtime record to `tracing`, as `ort`'s own bridge does, unless it repeats one written moments ago.
fn record(severity: LogLevel, id: &str, location: &str, message: &str) {
    record_through(&THROTTLE, severity, id, location, message);
}

/// [`record`], against `throttle`.
fn record_through(throttle: &Mutex<Throttle>, severity: LogLevel, id: &str, location: &str, message: &str) {
    if is_out_of_memory(message) {
        OUT_OF_MEMORY.with(|count| count.set(count.get().wrapping_add(1)));
    }

    let level = match severity {
        LogLevel::Verbose => Level::TRACE,
        LogLevel::Info => Level::INFO,
        LogLevel::Warning => Level::WARN,
        LogLevel::Error if is_skipped_tactic(message) => Level::WARN,
        LogLevel::Error | LogLevel::Fatal => Level::ERROR,
    };

    // Only what is written under the default `ort=warn` floor: below it nothing reaches the log to flood it, and a
    // verbose runtime would otherwise pay a lock per record for nothing.
    if matches!(level, Level::WARN | Level::ERROR) {
        let admitted = lock(throttle).admit(pattern_of(message), level, Instant::now());
        let Some(summaries) = admitted else {
            return;
        };
        summaries.iter().for_each(Summary::write);
    }

    let span = tracing::span!(target: "ort::logging", Level::TRACE, "ort", id = id, location = location);

    match (level, severity) {
        (Level::TRACE, _) => tracing::event!(target: "ort::logging", parent: &span, Level::TRACE, "{message}"),
        (Level::INFO, _) => tracing::event!(target: "ort::logging", parent: &span, Level::INFO, "{message}"),
        (Level::WARN, _) => tracing::event!(target: "ort::logging", parent: &span, Level::WARN, "{message}"),
        (_, LogLevel::Fatal) => {
            tracing::event!(target: "ort::logging", parent: &span, Level::ERROR, "(FATAL): {message}");
        }
        _ => tracing::event!(target: "ort::logging", parent: &span, Level::ERROR, "{message}"),
    }
}

/// [`flush`], against `throttle`.
fn flush_through(throttle: &Mutex<Throttle>) {
    let summaries = lock(throttle).drain(|_| true);
    summaries.iter().for_each(Summary::write);
}

/// The throttle, whether or not a thread panicked holding it: what it guards is a tally, which a panic cannot leave
/// in a state worse than slightly off, and the callback it is taken from must not panic itself.
fn lock(throttle: &Mutex<Throttle>) -> std::sync::MutexGuard<'_, Throttle> {
    throttle.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What makes two records "the same" for the throttle: the message with every run of digits replaced by `#`.
///
/// So `Requested size was 3893362688 bytes.` and `Requested size was 4246732800 bytes.` are one pattern, as are the
/// timestamps TensorRT opens each of its records with — and a different failure, or the same one somewhere else in
/// the runtime, is another.
fn pattern_of(message: &str) -> String {
    let mut pattern = String::with_capacity(message.len());
    let mut in_digits = false;

    for character in message.trim_end().chars() {
        if character.is_ascii_digit() {
            if !in_digits {
                pattern.push('#');
            }
            in_digits = true;
        } else {
            pattern.push(character);
            in_digits = false;
        }
    }

    pattern
}

/// Which record patterns were written recently, and how many like each were held back since.
#[derive(Debug, Default)]
struct Throttle {
    held: HashMap<String, Window>,
}

/// One pattern's current window.
#[derive(Debug)]
struct Window {
    /// When the record that opened it was written.
    opened: Instant,
    /// The level it was written at, which its tally is written at too.
    level: Level,
    /// How many like it were held back since.
    suppressed: u64,
}

/// How many records like `pattern` were held back.
#[derive(Debug, PartialEq, Eq)]
struct Summary {
    pattern: String,
    level: Level,
    suppressed: u64,
}

impl Summary {
    /// Writes this tally as one record, at the level of the records it stands for.
    ///
    /// Under this module's own target rather than `ort::logging`: it is this crate's statement about the runtime's
    /// records, not one of them.
    fn write(&self) {
        let Self { pattern, level, suppressed } = self;

        if *level == Level::ERROR {
            tracing::error!(suppressed, %pattern, "repeated runtime records were suppressed");
        } else {
            tracing::warn!(suppressed, %pattern, "repeated runtime records were suppressed");
        }
    }
}

impl Throttle {
    /// Whether a record with `pattern` is written at `now`: `None` where it is held back, or the tallies of every
    /// window that has closed by now, to write before it.
    fn admit(&mut self, pattern: String, level: Level, now: Instant) -> Option<Vec<Summary>> {
        if let Some(window) = self.held.get_mut(&pattern)
            && now.saturating_duration_since(window.opened) < REPEAT_WINDOW
        {
            window.suppressed += 1;
            return None;
        }

        // Closing every expired window here, not only this pattern's, is also what keeps the map from growing with
        // every pattern the process ever saw.
        let summaries = self.drain(|window| now.saturating_duration_since(window.opened) >= REPEAT_WINDOW);
        self.held.insert(pattern, Window { opened: now, level, suppressed: 0 });

        Some(summaries)
    }

    /// Closes every window `closes` picks, returning a tally for each that held something back.
    fn drain(&mut self, closes: impl Fn(&Window) -> bool) -> Vec<Summary> {
        let closed: Vec<String> =
            self.held.iter().filter(|(_, window)| closes(window)).map(|(key, _)| key.clone()).collect();

        let mut summaries: Vec<Summary> = closed
            .into_iter()
            .filter_map(|pattern| {
                let window = self.held.remove(&pattern)?;
                (window.suppressed > 0).then_some(Summary {
                    pattern,
                    level: window.level,
                    suppressed: window.suppressed,
                })
            })
            .collect();

        // Stable for a reader and a test: the map's order is not.
        summaries.sort_by(|a, b| a.pattern.cmp(&b.pattern));
        summaries
    }
}

/// Whether `message` reports a device allocation that failed: TensorRT's `Error Code 2: OutOfMemory`, or the CUDA
/// provider's `out of memory`.
fn is_out_of_memory(message: &str) -> bool {
    message.contains("OutOfMemory") || message.contains("out of memory")
}

/// Whether `message` is TensorRT failing to grow the scratch memory of a tactic it then skips — which the builder
/// recovers from on its own, and which it logs as an error anyway.
///
/// The file and the function are matched apart because TensorRT spells the location two ways: bare on Linux,
/// `[virtualMemoryBuffer.cpp::resizePhysical::153]`, and qualified on Windows,
/// `[virtualMemoryBuffer.cpp::nvinfer1::StdVirtualMemoryBufferImpl::resizePhysical::153]`.
fn is_skipped_tactic(message: &str) -> bool {
    message.contains("virtualMemoryBuffer.cpp::")
        && message.contains("::resizePhysical::")
        && message.contains("OutOfMemory")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The record from the field: an RTX 3070 building Tokyo, whose session then came up on TensorRT.
    const SKIPPED_TACTIC: &str = "[2026-09-30 18:44:58   ERROR] [virtualMemoryBuffer.cpp::resizePhysical::153] Error \
                                  Code 2: OutOfMemory (Requested size was 2147483648 bytes.)";

    /// The same record as TensorRT writes it on Windows: an RTX 3050 building Tokyo, which also came up on TensorRT.
    const SKIPPED_TACTIC_WINDOWS: &str = "[2026-10-06 13:28:34   ERROR] [virtualMemoryBuffer.cpp::nvinfer1::\
                                          StdVirtualMemoryBufferImpl::resizePhysical::153] Error Code 2: OutOfMemory \
                                          (Requested size was 3893362688 bytes.)";

    /// The record from the field that does fail the build: an RTX 5080 with Osaka resident.
    const FAILED_BUILD: &str = "[2026-09-27 06:51:55   ERROR] [globWriter.cpp::nvinfer1::builder::HybridGlobWriter::\
                                HybridGlobWriter::496] Error Code 2: OutOfMemory (Requested size was 1024 bytes.)";

    /// What `emit` writes to the log under the default directives, which are what keep `ort` at `warn`.
    fn captured(emit: impl FnOnce()) -> (String, ()) {
        crate::logging::records_of_blocking("info,ort=warn", emit)
    }

    /// A throttle of a test's own, so no other test's records hold this one's back.
    fn fresh() -> Mutex<Throttle> {
        Mutex::default()
    }

    #[test]
    fn a_skipped_tactic_is_written_as_a_warning() {
        let throttle = fresh();
        let (line, ()) = captured(|| {
            record_through(&throttle, LogLevel::Error, "opai", "tensorrt_execution_provider.h:98", SKIPPED_TACTIC);
        });

        assert!(line.contains("level=WARN"), "{line}");
        assert!(line.contains(" target=ort::logging "), "{line}");
        assert!(
            line.contains("location=tensorrt_execution_provider.h:98"),
            "the record lost its span fields: {line}"
        );
    }

    #[test]
    fn a_skipped_tactic_in_the_windows_spelling_is_written_as_a_warning() {
        let throttle = fresh();
        let (line, ()) = captured(|| {
            record_through(&throttle, LogLevel::Error, "opai", "trt.h:98", SKIPPED_TACTIC_WINDOWS);
        });

        assert!(line.contains("level=WARN"), "{line}");
    }

    #[test]
    fn any_other_runtime_error_keeps_its_severity() {
        let throttle = fresh();
        let (line, ()) = captured(|| record_through(&throttle, LogLevel::Error, "opai", "trt.h:98", FAILED_BUILD));

        assert!(line.contains("level=ERROR"), "{line}");
    }

    #[test]
    fn a_burst_of_one_record_is_written_once_and_its_count_when_flushed() {
        let throttle = fresh();
        let (log, ()) = captured(|| {
            record_through(&throttle, LogLevel::Error, "opai", "trt.h:98", SKIPPED_TACTIC_WINDOWS);
            // Another size and another second: still the same record.
            for _ in 0..41 {
                record_through(&throttle, LogLevel::Error, "opai", "trt.h:98", SKIPPED_TACTIC);
                record_through(
                    &throttle,
                    LogLevel::Error,
                    "opai",
                    "trt.h:98",
                    &SKIPPED_TACTIC_WINDOWS.replace("3893362688", "4246732800"),
                );
            }
            flush_through(&throttle);
        });

        let written: Vec<&str> = log.lines().filter(|line| line.contains("target=ort::logging")).collect();
        assert_eq!(written.len(), 2, "one per spelling, the rest held back: {log}");

        let summaries = crate::logging::records(&log, "repeated runtime records were suppressed");
        assert_eq!(summaries.len(), 2, "{log}");
        let total: u64 = summaries
            .iter()
            .map(|line| crate::logging::field(line, "suppressed").unwrap().parse::<u64>().unwrap())
            .sum();
        assert_eq!(total, 81);
        assert!(summaries.iter().all(|line| line.contains("level=WARN")), "{log}");
    }

    #[test]
    fn a_summary_of_errors_is_an_error() {
        let throttle = fresh();
        let (log, ()) = captured(|| {
            record_through(&throttle, LogLevel::Error, "opai", "trt.h:98", FAILED_BUILD);
            record_through(&throttle, LogLevel::Error, "opai", "trt.h:98", FAILED_BUILD);
            flush_through(&throttle);
        });

        let summaries = crate::logging::records(&log, "repeated runtime records were suppressed");
        assert_eq!(summaries.len(), 1, "{log}");
        assert!(summaries[0].contains("level=ERROR"), "{log}");
        assert_eq!(crate::logging::field(summaries[0], "suppressed"), Some("1"));
    }

    #[test]
    fn records_that_are_not_alike_are_each_written() {
        let throttle = fresh();
        let (log, ()) = captured(|| {
            record_through(&throttle, LogLevel::Error, "opai", "trt.h:98", SKIPPED_TACTIC);
            record_through(&throttle, LogLevel::Error, "opai", "trt.h:98", FAILED_BUILD);
            record_through(&throttle, LogLevel::Warning, "opai", "here", "Some nodes were not assigned");
            flush_through(&throttle);
        });

        assert_eq!(log.lines().filter(|line| line.contains("target=ort::logging")).count(), 3, "{log}");
        assert!(crate::logging::records(&log, "repeated runtime records were suppressed").is_empty(), "{log}");
    }

    #[test]
    fn a_flush_with_nothing_held_back_writes_nothing() {
        let throttle = fresh();
        let (log, ()) = captured(|| {
            record_through(&throttle, LogLevel::Error, "opai", "trt.h:98", FAILED_BUILD);
            flush_through(&throttle);
        });

        assert!(crate::logging::records(&log, "repeated runtime records were suppressed").is_empty(), "{log}");
    }

    #[test]
    fn a_record_after_its_window_closed_is_written_again_after_the_tally() {
        let mut throttle = Throttle::default();
        let start = Instant::now();
        let pattern = pattern_of(FAILED_BUILD);

        assert_eq!(throttle.admit(pattern.clone(), Level::ERROR, start), Some(Vec::new()));
        assert_eq!(throttle.admit(pattern.clone(), Level::ERROR, start + Duration::from_secs(1)), None);
        assert_eq!(throttle.admit(pattern.clone(), Level::ERROR, start + Duration::from_secs(59)), None);

        let reopened = throttle.admit(pattern.clone(), Level::ERROR, start + REPEAT_WINDOW);
        assert_eq!(reopened, Some(vec![Summary { pattern, level: Level::ERROR, suppressed: 2 }]));
    }

    #[test]
    fn a_closed_window_that_held_nothing_back_is_forgotten() {
        let mut throttle = Throttle::default();
        let start = Instant::now();

        throttle.admit("one".to_string(), Level::WARN, start);
        throttle.admit("two".to_string(), Level::WARN, start + REPEAT_WINDOW);

        assert_eq!(throttle.held.len(), 1, "{throttle:?}");
    }

    #[test]
    fn records_below_warn_are_never_held_back() {
        let throttle = fresh();
        let (log, ()) = crate::logging::records_of_blocking("info,ort=info", || {
            for _ in 0..3 {
                record_through(&throttle, LogLevel::Info, "opai", "here", "Graph optimized");
            }
        });

        assert_eq!(log.lines().filter(|line| line.contains("target=ort::logging")).count(), 3, "{log}");
    }

    #[test]
    fn digits_make_no_difference_to_a_pattern_and_anything_else_does() {
        assert_eq!(pattern_of(SKIPPED_TACTIC), pattern_of(&SKIPPED_TACTIC.replace("2147483648", "1")));
        assert_eq!(pattern_of("conv.2 needs 48234496 bytes\n"), "conv.# needs # bytes");
        assert_ne!(pattern_of(SKIPPED_TACTIC), pattern_of(SKIPPED_TACTIC_WINDOWS));
    }

    #[test]
    fn an_out_of_memory_record_marks_the_thread_that_logged_it() {
        let throttle = fresh();
        let mark = out_of_memory_mark();
        record_through(&throttle, LogLevel::Warning, "opai", "here", "Some nodes were not assigned");
        assert!(!out_of_memory_since(mark), "a record about something else was counted");

        record_through(&throttle, LogLevel::Error, "opai", "here", FAILED_BUILD);
        assert!(out_of_memory_since(mark));

        // Another thread's build is not this one's.
        let elsewhere = std::thread::spawn(|| {
            let mark = out_of_memory_mark();
            out_of_memory_since(mark)
        });
        assert!(!elsewhere.join().unwrap());
    }

    #[test]
    fn a_held_back_out_of_memory_record_still_marks_the_thread() {
        let throttle = fresh();
        record_through(&throttle, LogLevel::Error, "opai", "here", FAILED_BUILD);

        let mark = out_of_memory_mark();
        record_through(&throttle, LogLevel::Error, "opai", "here", FAILED_BUILD);

        assert!(out_of_memory_since(mark), "a record the throttle held back was not counted");
    }

    #[test]
    fn a_skipped_tactic_counts_as_running_out_of_memory() {
        // Only read when a build then fails, where memory being short is exactly what it says.
        let throttle = fresh();
        let mark = out_of_memory_mark();
        record_through(&throttle, LogLevel::Error, "opai", "here", SKIPPED_TACTIC);

        assert!(out_of_memory_since(mark));
    }
}
