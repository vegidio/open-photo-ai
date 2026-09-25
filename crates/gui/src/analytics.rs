//! The user's Analytics choice, as Rust has to know it before any window exists.
//!
//! The choice is the window's: it lives in the settings store beside every other preference, and what Settings shows
//! is what the user chose. Rust decides whether to start telemetry in `setup`, before the window has loaded, so the
//! window mirrors its choice into `analytics.json` in `opai`'s configuration directory, and [`read`] is what `setup`
//! consults. The file follows the store, never the other way round.
//!
//! [`set_analytics`] is how the window mirrors it: once at boot, and on every Save that changes it. Turning it off stops
//! sending at once. Turning it on takes effect at the next launch, because `o11y` starts once per process.
//!
//! **Turning it off is itself never sent.** [`set_analytics`] opens no command span, and stops telemetry before it
//! writes or records anything.

// Not Tauri's `app_config_dir()`: that resolves to the bundle identifier's directory, and this file belongs beside the
// logs, models and cache in `opai`'s.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use opai::telemetry::{Sending, TelemetryError};
use serde::{Deserialize, Serialize};

/// The file's name inside `opai`'s configuration directory.
const FILE: &str = "analytics.json";

/// What the file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Stored {
    enabled: bool,
}

/// Whether telemetry is not running because the user opted out: at launch, or since.
///
/// The one place that answer is kept. `setup` sets it when it does not start telemetry for the choice, and
/// [`set_analytics`] when it stops it.
static OPTED_OUT: AtomicBool = AtomicBool::new(false);

/// Why [`set_analytics`] could not record the choice. Telemetry was stopped whatever this says: an opt-out holds for the
/// session even when it cannot be written down.
///
/// Tagged like [`crate::logs::LogsError`], so the frontend reads every command's rejection the same way.
#[derive(Debug, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum AnalyticsError {
    /// The choice could not be written to the file the next launch reads.
    #[error("{message}")]
    SetAnalytics {
        /// What went wrong, in full.
        message: String,
    },
}

// ── The file ──────────────────────────────────────────────────────────────────────────────────────────────────────

/// Where the choice is mirrored. Creates nothing.
fn path() -> Result<PathBuf, rust_sak::fs::FsError> {
    rust_sak::fs::user_config_dir(opai::APP_NAME, FILE)
}

/// The choice the next launch should honour.
///
/// On for a file that is absent, which is a first launch and the recorded default. Off, with a warning, for one that is
/// present and cannot be read or parsed: a file exists only because a choice was mirrored, and a torn opt-out must not
/// read as an opt-in. The window's mirror at boot corrects it either way.
pub(crate) fn read() -> bool {
    match path() {
        Ok(path) => read_at(&path),
        // No configuration directory, so no file can have been written: what an absent file means.
        Err(error) => {
            tracing::warn!(%error, "the analytics choice's location could not be determined; reading it as the default");
            true
        }
    }
}

/// [`read`], from `path`.
pub(crate) fn read_at(path: &Path) -> bool {
    let raw = match std::fs::read(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return true,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "the analytics choice could not be read; reading it as off");
            return false;
        }
    };

    match serde_json::from_slice::<Stored>(&raw) {
        Ok(stored) => stored.enabled,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "the analytics choice is malformed; reading it as off");
            false
        }
    }
}

/// Mirrors `enabled` into the file at `path`, answering whether the file changed.
///
/// Written through a temporary file beside it and renamed over it, so a crash mid-write leaves the old choice or the
/// new one and never a torn file. A write that would change nothing is skipped.
pub(crate) fn write_at(path: &Path, enabled: bool) -> Result<bool, std::io::Error> {
    let stored = Stored { enabled };

    // Compared by value rather than by bytes, so a file written by hand with other spacing is not rewritten.
    if std::fs::read(path).ok().and_then(|raw| serde_json::from_slice::<Stored>(&raw).ok()) == Some(stored) {
        return Ok(false);
    }

    let directory = path.parent().ok_or_else(|| std::io::Error::other("the choice's path has no directory"))?;
    std::fs::create_dir_all(directory)?;

    let mut temporary = rust_sak::fs::mk_temp_file_in(directory, ".analytics-").map_err(std::io::Error::other)?;
    serde_json::to_writer(&mut temporary, &stored)?;
    temporary.flush()?;
    temporary.sync_all()?;
    temporary.into_inner().persist(path).map_err(|error| error.error)?;

    Ok(true)
}

// ── At launch ─────────────────────────────────────────────────────────────────────────────────────────────────────

/// Starts telemetry through `start` if the choice is on, and records why not where it is off.
///
/// `setup` calls it with [`read`]'s answer and `opai::telemetry::start`.
pub(crate) fn begin(enabled: bool, start: impl FnOnce() -> Result<Sending, TelemetryError>) {
    begin_with(&OPTED_OUT, enabled, start);
}

/// [`begin`], against an opt-out flag the test owns.
fn begin_with(opted_out: &AtomicBool, enabled: bool, start: impl FnOnce() -> Result<Sending, TelemetryError>) {
    if !enabled {
        opted_out.store(true, Ordering::SeqCst);
        tracing::debug!("telemetry is not sending: the user turned analytics off");

        return;
    }

    match start() {
        Ok(Sending::Yes) => {}
        Ok(Sending::NoCollector) => tracing::debug!("telemetry is not sending: this build names no collector"),
        Err(error) => tracing::warn!(%error, "telemetry could not start; running without it"),
    }
}

// ── The command ───────────────────────────────────────────────────────────────────────────────────────────────────

// The command's name is written once more, in `frontend/ipc/analytics.ts`.
/// Record the user's Analytics choice, as the window's settings store holds it.
///
/// Off stops sending at once, before anything is written or recorded. On takes effect at the next launch.
///
/// **Not traced.** Every other command runs inside a span named after it, and a span for this one would be one more
/// thing about an opt-out that could leave the process.
///
/// `async` only so the work is off the window's thread: stopping telemetry sends what it holds and can wait for one
/// export timeout.
///
/// # Errors
///
/// [`AnalyticsError::SetAnalytics`] where the file could not be written. An opt-out has still stopped sending.
#[tauri::command]
pub(crate) async fn set_analytics(enabled: bool) -> Result<(), AnalyticsError> {
    crate::task::spawn_blocking(move || {
        let path = path().map_err(std::io::Error::other);

        switch(&OPTED_OUT, enabled, opai::telemetry::stop, |enabled| write_at(&path?, enabled))
    })
    .await
    .unwrap_or_else(|error| Err(AnalyticsError::SetAnalytics { message: error.to_string() }))
}

/// [`set_analytics`]'s order, with the stop and the write passed in so a test can watch them.
///
/// - **Off:** stop, then write, then record. Because the stop is first, nothing after it reaches the collector.
/// - **On:** write, then record that it waits for the next launch, where telemetry is not running for an opt-out.
///
/// A record is written only when something changed: the file, or whether telemetry runs. The window mirrors the choice
/// at every boot, and that mirror of an unchanged choice is not news.
fn switch(
    opted_out: &AtomicBool,
    enabled: bool,
    stop: impl FnOnce(),
    write: impl FnOnce(bool) -> Result<bool, std::io::Error>,
) -> Result<(), AnalyticsError> {
    if enabled {
        let changed = write(enabled).map_err(unwritten)?;

        if changed && opted_out.load(Ordering::SeqCst) {
            tracing::info!("analytics was turned on; it takes effect at the next launch");
        }

        return Ok(());
    }

    stop();
    let was_sending = !opted_out.swap(true, Ordering::SeqCst);

    let changed = write(enabled).map_err(unwritten)?;
    if changed || was_sending {
        tracing::info!("analytics was turned off; nothing more is sent this session");
    }

    Ok(())
}

/// A failed write, recorded and turned into the error the window receives.
fn unwritten(error: std::io::Error) -> AnalyticsError {
    tracing::warn!(%error, "the analytics choice could not be written; the next launch may not honour it");

    AnalyticsError::SetAnalytics { message: format!("the analytics choice could not be saved: {error}") }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::time::Duration;

    use tracing::Level;

    use super::*;
    use crate::test_support::recorded;

    fn file() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("opai").join(FILE);

        (dir, path)
    }

    // ── The file ──

    #[test]
    fn an_absent_file_is_the_default_and_records_nothing() {
        let (_dir, path) = file();

        let (recorded, enabled) = recorded(|| read_at(&path));

        assert!(enabled, "a first launch did not read as on");
        assert!(recorded.events.is_empty(), "{:#?}", recorded.events);
    }

    #[test]
    fn a_written_choice_reads_back() {
        let (_dir, path) = file();

        for enabled in [false, true, false] {
            write_at(&path, enabled).expect("writable");
            assert_eq!(read_at(&path), enabled);
        }
        assert_eq!(std::fs::read_to_string(&path).expect("readable"), r#"{"enabled":false}"#);
    }

    #[test]
    fn a_file_that_cannot_be_parsed_reads_as_off_and_says_so() {
        let (_dir, path) = file();
        std::fs::create_dir_all(path.parent().expect("a directory")).expect("creatable");

        for garbage in [&b"{\"enabled\": tr"[..], b"", b"[]", b"{\"enabled\": \"yes\"}"] {
            std::fs::write(&path, garbage).expect("writable");

            let (recorded, enabled) = recorded(|| read_at(&path));

            assert!(!enabled, "a torn file read as an opt-in: {garbage:?}");
            assert_eq!(recorded.at(Level::WARN).len(), 1, "{:#?}", recorded.events);
        }
    }

    #[test]
    fn a_write_that_changes_nothing_leaves_the_file_alone() {
        let (_dir, path) = file();
        assert!(write_at(&path, false).expect("writable"), "a first write reported no change");
        let written = std::fs::metadata(&path).and_then(|metadata| metadata.modified()).expect("stat");

        // Longer than any filesystem's modification-time granularity, so a rewrite would show.
        std::thread::sleep(Duration::from_millis(20));
        assert!(!write_at(&path, false).expect("writable"), "a repeat write reported a change");

        let after = std::fs::metadata(&path).and_then(|metadata| metadata.modified()).expect("stat");
        assert_eq!(written, after, "a write that changed nothing touched the file");
    }

    #[test]
    fn a_write_over_an_existing_file_replaces_it_and_leaves_nothing_beside_it() {
        let (_dir, path) = file();
        write_at(&path, true).expect("writable");

        assert!(write_at(&path, false).expect("writable"));
        assert!(!read_at(&path));

        let names: Vec<_> = std::fs::read_dir(path.parent().expect("a directory"))
            .expect("listable")
            .map(|entry| entry.expect("readable").file_name())
            .collect();
        assert_eq!(names, [FILE], "the temporary file was left behind");
    }

    // ── At launch ──

    #[test]
    fn a_launch_with_the_choice_off_never_starts_telemetry() {
        let opted_out = AtomicBool::new(false);

        let (recorded, ()) = recorded(|| begin_with(&opted_out, false, || panic!("telemetry was started")));

        assert!(opted_out.load(Ordering::SeqCst));
        let [reason] = recorded.events.as_slice() else { panic!("{:#?}", recorded.events) };
        assert_eq!(reason.level, Level::DEBUG);
    }

    #[test]
    fn a_launch_with_the_choice_on_starts_telemetry() {
        let opted_out = AtomicBool::new(false);
        let mut started = false;

        begin_with(&opted_out, true, || {
            started = true;
            Ok(Sending::Yes)
        });

        assert!(started);
        assert!(!opted_out.load(Ordering::SeqCst));
    }

    // ── The command ──

    /// What happened, in order: the stop, the write, and each record.
    #[derive(Default)]
    struct Order(RefCell<Vec<String>>);

    impl Order {
        fn push(&self, step: &str) {
            self.0.borrow_mut().push(step.to_string());
        }
    }

    /// [`switch`] under a recorder, with a stop and a write that note when they ran and what had been recorded then.
    fn switched(opted_out: &AtomicBool, enabled: bool, changed: bool) -> (crate::test_support::Recorded, Vec<String>) {
        let order = Order::default();
        let (recorded, answer) = recorded(|| {
            switch(
                opted_out,
                enabled,
                || order.push("stop"),
                |_| {
                    order.push("write");
                    Ok(changed)
                },
            )
        });
        answer.expect("the write succeeded");

        (recorded, order.0.into_inner())
    }

    #[test]
    fn turning_it_off_stops_telemetry_before_anything_is_written_or_recorded() {
        let opted_out = AtomicBool::new(false);
        let order = Order::default();

        let (recorded, answer) = recorded(|| {
            switch(
                &opted_out,
                false,
                || order.push("stop"),
                |_| {
                    order.push("write");
                    Ok(true)
                },
            )
        });
        answer.expect("the write succeeded");

        assert_eq!(order.0.into_inner(), ["stop", "write"]);
        assert!(opted_out.load(Ordering::SeqCst));

        let [record] = recorded.events.as_slice() else { panic!("{:#?}", recorded.events) };
        assert_eq!(record.level, Level::INFO);
        assert!(recorded.spans.is_empty(), "the opt-out opened a span: {:#?}", recorded.spans);
    }

    #[test]
    fn nothing_is_recorded_before_the_stop() {
        let opted_out = AtomicBool::new(false);
        let recorder = crate::test_support::Recorder::default();
        let seen_at_stop = RefCell::new(None);

        {
            use tracing_subscriber::layer::SubscriberExt;

            let subscriber = tracing_subscriber::registry().with(recorder.clone());
            tracing::subscriber::with_default(subscriber, || {
                switch(
                    &opted_out,
                    false,
                    || *seen_at_stop.borrow_mut() = Some(recorder.recorded().events.len()),
                    |_| Ok(true),
                )
            })
            .expect("the write succeeded");
        }

        assert_eq!(seen_at_stop.into_inner(), Some(0), "something was recorded while telemetry still ran");
        assert_eq!(recorder.recorded().events.len(), 1);
    }

    #[test]
    fn turning_it_on_never_stops_telemetry_and_says_when_it_takes_effect() {
        let opted_out = AtomicBool::new(true);

        let (recorded, order) = switched(&opted_out, true, true);

        assert_eq!(order, ["write"]);
        let [record] = recorded.events.as_slice() else { panic!("{:#?}", recorded.events) };
        assert_eq!(record.level, Level::INFO);
        assert!(recorded.spans.is_empty());
    }

    #[test]
    fn mirroring_an_unchanged_choice_records_nothing() {
        // The boot mirror of a choice already in force: on while sending, and off after a launch that began off.
        let (recorded, order) = switched(&AtomicBool::new(false), true, false);
        assert_eq!(order, ["write"]);
        assert!(recorded.events.is_empty(), "{:#?}", recorded.events);

        let (recorded, order) = switched(&AtomicBool::new(true), false, false);
        assert_eq!(order, ["stop", "write"]);
        assert!(recorded.events.is_empty(), "{:#?}", recorded.events);
    }

    #[test]
    fn a_failed_write_after_an_opt_out_is_returned_and_telemetry_stays_stopped() {
        let opted_out = AtomicBool::new(false);
        let stopped = std::cell::Cell::new(false);

        let (recorded, answer) =
            recorded(|| switch(&opted_out, false, || stopped.set(true), |_| Err(std::io::Error::other("disk full"))));

        let AnalyticsError::SetAnalytics { message } = answer.expect_err("the write failed");
        assert!(message.contains("disk full"), "{message}");
        assert!(stopped.get() && opted_out.load(Ordering::SeqCst));
        assert_eq!(recorded.at(Level::WARN).len(), 1, "{:#?}", recorded.events);
    }

    #[test]
    fn the_rejection_crosses_under_its_kind() {
        let error = AnalyticsError::SetAnalytics { message: "a".to_string() };

        assert_eq!(
            serde_json::to_value(&error).expect("serializable"),
            serde_json::json!({ "kind": "setAnalytics", "message": "a" })
        );
    }
}
