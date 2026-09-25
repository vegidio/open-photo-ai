//! Reading back what image IO recorded, for the tests beside each operation.

// Apart from `test_support`, which an integration test also compiles by path: these reach the crate's own recording,
// which only exists inside it.

use std::path::Path;

/// Every record `work` produced at `info`, including those an async twin writes on the blocking thread it hops to,
/// beside what it returned.
pub(crate) fn recorded_async<T>(work: impl Future<Output = T>) -> (String, T) {
    // `records_of` binds its subscriber to the calling thread, and an async twin writes its record on the blocking
    // thread it hops to. A runtime of its own, whose every thread — the blocking pool included — is bound to the same
    // recording as it starts, is what lets that record be seen. The guard is forgotten because the binding is meant to
    // last for the thread's life, which ends with the runtime at the end of this closure.
    crate::logging::records_of_blocking("info", || {
        let recording = tracing::dispatcher::get_default(tracing::Dispatch::clone);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .on_thread_start(move || std::mem::forget(tracing::dispatcher::set_default(&recording)))
            .build()
            .unwrap();

        runtime.block_on(work)
    })
}

/// The one record in `log` whose message is `message`, asserting it is the only record at all and names `op`.
pub(crate) fn the_one_failure<'a>(log: &'a str, message: &str, op: &str) -> &'a str {
    let lines: Vec<&str> = log.lines().filter(|line| line.contains("msg=")).collect();
    let [line] = lines[..] else {
        panic!("one failure was recorded {} times:\n{log}", lines.len());
    };

    assert_eq!(crate::logging::records(log, message), [line], "{log}");
    assert!(line.contains("level=WARN"), "{line}");
    assert_eq!(crate::logging::field(line, "op"), Some(op), "{line}");
    assert!(crate::logging::field(line, "error").is_some(), "the record does not say why: {line}");

    line
}

/// `path` as the file writes it, quoted where it needs to be.
pub(crate) fn as_written(path: &Path) -> String {
    crate::logging::format::as_field_value(&path.display().to_string())
}
