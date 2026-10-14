//! Whether this run sends telemetry, starting it where it does, and the record that closes the session before it stops.
//!
//! On unless the user turned it off for the run: with `--no-analytics`, or with `OPAI_ANALYTICS` set to `0`, `off`,
//! `false` or `no`. The GUI's own choice, kept beside its settings, is not read here: a terminal user opts out on the
//! command line or in the environment, where the rest of this command is configured.
//!
//! **An opt-out is never sent.** [`begin`] decides before telemetry is started, so a run that begins off sends nothing
//! at all, and says why in the log file only.

use std::ffi::OsStr;
use std::time::Duration;

use opai::telemetry::{Sending, TelemetryError};
use tracing::Span;

use crate::error::CliError;

/// The variable that turns analytics off for a run.
pub const ENV: &str = "OPAI_ANALYTICS";

/// The target the run's and each image's span are opened under, as the GUI opens its commands' under `gui::command`.
pub const TARGET: &str = "cli::command";

/// Whether this run may send telemetry: off where `flag` (`--no-analytics`) is given or `env` (`OPAI_ANALYTICS`) says
/// off, and on otherwise.
pub fn enabled(flag: bool, env: Option<&OsStr>) -> bool {
    if flag {
        return false;
    }

    let off = env
        .and_then(OsStr::to_str)
        .map(str::trim)
        .is_some_and(|value| ["0", "off", "false", "no"].iter().any(|word| value.eq_ignore_ascii_case(word)));

    !off
}

/// Starts telemetry through `start` if `enabled`, and records why not where it is off.
///
/// `main` calls it with [`enabled`]'s answer and `opai::telemetry::start`, once the arguments have parsed: `--help`,
/// `--version` and a usage error never open a connection.
pub fn begin(enabled: bool, start: impl FnOnce() -> Result<Sending, TelemetryError>) {
    if !enabled {
        tracing::debug!("telemetry is not sending: the user turned analytics off");
        return;
    }

    match start() {
        Ok(Sending::Yes) => {}
        Ok(Sending::NoCollector) => tracing::debug!("telemetry is not sending: this build names no collector"),
        Err(error) => tracing::warn!(%error, "telemetry could not start; running without it"),
    }
}

/// Records the session's end, then stops telemetry through `stop`.
///
/// In that order, so the record reaches the collector in the flush the stop makes, as the GUI's closing record does: a
/// run that ended is then told apart from one that stopped sending because it crashed.
pub fn closing(uptime: Duration, stop: impl FnOnce()) {
    tracing::info!(target: "cli", ?uptime, "Open Photo AI is closing");

    stop();
}

/// Records how the work `span` covers ended on it: `finished`, `stopped` for a Ctrl-C, or `failed` with the reason,
/// which the bridge turns into a failed status. The reason is the error as the log has it, which may name a path, as
/// the GUI's command spans' do.
pub fn mark<T>(span: &Span, result: &Result<T, CliError>) {
    match result {
        Ok(_) => {
            span.record("outcome", "finished");
        }
        Err(CliError::Cancelled) => {
            span.record("outcome", "stopped");
        }
        Err(error) => {
            span.record("outcome", "failed");
            span.record("error", tracing::field::display(crate::full(error)));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use tracing::Level;

    use super::*;
    use crate::test_support::recorded;

    #[test]
    fn analytics_is_on_unless_the_flag_or_the_variable_turns_it_off() {
        assert!(enabled(false, None));
        assert!(!enabled(true, None));
        assert!(!enabled(true, Some(OsStr::new("on"))), "the variable overrode the flag");

        for off in ["0", "off", "OFF", "False", "no", " no "] {
            assert!(!enabled(false, Some(OsStr::new(off))), "{off:?} did not turn it off");
        }
        for on in ["1", "on", "true", "yes", "", "anything"] {
            assert!(enabled(false, Some(OsStr::new(on))), "{on:?} turned it off");
        }
    }

    #[test]
    fn a_run_with_analytics_off_never_starts_telemetry_and_says_why_at_debug() {
        let (recorded, ()) = recorded(|| begin(false, || panic!("telemetry was started")));

        let [reason] = recorded.events.as_slice() else { panic!("{:#?}", recorded.events) };
        assert_eq!(reason.level, Level::DEBUG);
    }

    #[test]
    fn a_run_with_analytics_on_starts_telemetry() {
        let started = Cell::new(false);

        begin(true, || {
            started.set(true);
            Ok(Sending::Yes)
        });

        assert!(started.get());
    }

    #[test]
    fn the_sessions_end_is_recorded_before_telemetry_stops() {
        // Installs the suite's global subscriber, so no callsite here has cached `never` before the recorder below.
        let _ = recorded(|| ());
        let seen_at_stop = Cell::new(None);
        let recorder = crate::test_support::Recorder::default();

        {
            use tracing_subscriber::layer::SubscriberExt;

            let subscriber = tracing_subscriber::registry().with(recorder.clone());
            tracing::subscriber::with_default(subscriber, || {
                closing(Duration::from_secs(90), || seen_at_stop.set(Some(recorder.recorded().events.len())));
            });
        }

        assert_eq!(seen_at_stop.get(), Some(1), "the closing record was not emitted before the stop");
        let [record] = recorder.recorded().events.try_into().expect("one record");
        assert_eq!(record.level, Level::INFO);
        assert_eq!(record.target, "cli");
        assert_eq!(record.field("uptime"), Some("90s"));
    }

    #[test]
    fn a_span_is_marked_with_how_the_work_ended() {
        let cases: [(Result<(), CliError>, &str, Option<&str>); 3] = [
            (Ok(()), "finished", None),
            (Err(CliError::Cancelled), "stopped", None),
            (Err(CliError::Usage("nope".to_string())), "failed", Some("nope")),
        ];

        for (result, outcome, error) in cases {
            let (recorded, ()) = recorded(|| {
                let span = tracing::info_span!(
                    target: TARGET,
                    "run",
                    outcome = tracing::field::Empty,
                    error = tracing::field::Empty
                );
                mark(&span, &result);
            });

            let span = recorded.only("run");
            assert_eq!(span.target, TARGET);
            assert_eq!(span.field("outcome"), Some(outcome));
            assert_eq!(span.field("error"), error);
        }
    }
}
