//! What one real session leaves in a real file, and what a second one does to it.
//!
//! [`opai::logging::init`] can be called once per process and installs a subscriber for the life of it, so it is not
//! something a unit test can drive: the suite shares one process, and the second test to reach it would be refused
//! while the first had already redirected every record in the binary. Everything that can be covered without owning a
//! process is covered in `src/logging` — the format, the filter, the rotation, the retention, the panic hook.
//!
//! What is left, and what this file is for, is the composition: that `init` creates the directory, writes the divider
//! through the rotator, installs the subscriber and emits the header, in that order, and that a second session
//! appends to what the first wrote instead of replacing it. The second session is a **child process**, because one
//! process gets one sink.
//!
//! It runs against the real configuration directory under an application name nothing else uses, and removes it
//! afterwards. That is the same trade `tests/reexec_linux.rs` makes: the platform lookup is part of what is being
//! checked, so redirecting it would be checking something else.

use std::process::Command;

use opai::{CLI, GUI};

/// An application name nothing else uses, so the file this writes is unambiguously its own.
const APP: &str = "opai-logging-session-test";

/// Set by the parent in the child's environment; its presence is what turns a run of this test into the second
/// session.
const CHILD: &str = "OPAI_LOGGING_TEST_CHILD";

/// The divider a session writes before its first record.
const DIVIDER: &str = "---";

#[test]
fn two_sessions_leave_two_dividers_and_two_headers_in_one_file() {
    if std::env::var_os(CHILD).is_some() {
        return second_session();
    }

    // A previous run of this test that failed before its cleanup would otherwise be read as one of this run's
    // sessions.
    let path = opai::logging::log_path(APP).unwrap();
    let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());

    // The first session, in this process.
    let opened = opai::logging::init(APP, CLI).expect("the sink could not be installed");
    assert_eq!(opened, path, "`init` opened a file other than the one `log_path` names");
    assert!(opened.is_file(), "`init` did not create the log file");
    assert!(opened.parent().unwrap().is_dir(), "`init` did not create the log directory");

    tracing::info!(marker = "first-session", "a record from the first session");

    // The order `init` promises, read off the file rather than asserted about the code: the divider, then the header,
    // then whatever the session went on to say.
    let after_first = std::fs::read_to_string(&opened).unwrap();
    let lines: Vec<&str> = after_first.lines().collect();

    assert_eq!(lines[0], DIVIDER, "the first line is not the session divider:\n{after_first}");
    assert_header(lines[1], CLI);
    assert!(lines[2].contains("marker=first-session"), "{after_first}");

    // A second `init` in this process is refused, and the sink installed first goes on working.
    let refused = opai::logging::init(APP, CLI);
    assert!(refused.is_err(), "a second init in one process was accepted");
    tracing::info!(marker = "after-refusal", "still writing through the first sink");
    assert!(
        std::fs::read_to_string(&opened).unwrap().contains("marker=after-refusal"),
        "the refused second init stopped the first sink from writing"
    );

    // The second session, in a process of its own.
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "two_sessions_leave_two_dividers_and_two_headers_in_one_file", "--nocapture"])
        .env(CHILD, "1")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "the second session failed\n{stderr}");

    let after_second = std::fs::read_to_string(&opened).unwrap();
    let lines: Vec<&str> = after_second.lines().collect();

    // Nothing the first session wrote was truncated: a run does not destroy the account of the run before it.
    assert!(after_second.starts_with(&after_first), "the second session did not append:\n{after_second}");
    assert!(after_second.contains("marker=first-session"), "the first session's records are gone");
    assert!(
        after_second.contains("marker=second-session"),
        "the second session wrote nothing:\n{after_second}"
    );

    // Two dividers, each followed by that session's header.
    let dividers: Vec<usize> = lines.iter().enumerate().filter(|(_, line)| **line == DIVIDER).map(|(i, _)| i).collect();
    assert_eq!(dividers.len(), 2, "expected one divider per session, got {}:\n{after_second}", dividers.len());

    assert_header(lines[dividers[0] + 1], CLI);
    assert_header(lines[dividers[1] + 1], GUI);

    // And the two sessions are told apart by the field that exists for exactly that, independently of the divider.
    let first = lines[dividers[0] + 1];
    let second = lines[dividers[1] + 1];
    assert!(first.contains(" app=cli "), "{first}");
    assert!(second.contains(" app=gui "), "{second}");

    let _ = std::fs::remove_dir_all(opened.parent().unwrap().parent().unwrap());
}

/// The child half: one more session under the same name, from a different application.
///
/// A different identity on purpose — it is what makes the `app=` assertions above about the field rather than about
/// the order the two sessions happen to be in.
fn second_session() {
    let path = opai::logging::init(APP, GUI).expect("the second session could not install a sink");
    assert!(path.is_file());

    tracing::info!(marker = "second-session", "a record from the second session");
}

/// Asserts that `line` is a session header for `app`.
///
/// The header's job is to say what a reader has to know before reading anything under it: which application, which
/// version, and what it was running on.
fn assert_header(line: &str, app: &str) {
    assert!(line.contains("level=INFO"), "the header is not an informational record: {line}");
    assert!(line.contains(&format!(" app={app} ")), "the header does not name the application: {line}");
    assert!(line.contains(&format!("version={}", opai::version())), "the header carries no version: {line}");
    assert!(
        line.contains(&format!("os={}", std::env::consts::OS)),
        "the header carries no operating system: {line}"
    );
    assert!(
        line.contains(&format!("arch={}", std::env::consts::ARCH)),
        "the header carries no architecture: {line}"
    );

    // And it is a timestamped record like every other line, rather than a banner of its own shape.
    assert!(line.starts_with("time="), "the header is not in the record format: {line}");
}
