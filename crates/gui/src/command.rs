//! The one wrapper every command the window can ask for runs inside: a span named after the command, how the request
//! ended recorded on it, and a record of a failure only this crate observed.
//!
//! A command's body is written as `traced(command_span!("enhance", traceparent, run = run), async move { ... }).await`,
//! or [`traced_sync`] for a synchronous one. The span is the request's unit of work: `opai`'s unit spans take the
//! current span as their parent, so everything the library does to serve the request is beneath it, and the file
//! writes its `run` onto every record emitted inside it. `set_analytics` and `log` are the two commands never wrapped:
//! see `crate::analytics` and `crate::frontend`.
//!
//! Every wrapped command takes a [`Traceparent`], the header the window's `ipc/invoke.ts` sends with the request when
//! it is tracing it, and hands it to `command_span!`, which has no form without one. The span then continues the
//! window's trace, as the child of the window's span for the request, so the two halves are one trace in Grafana. With
//! no header, or one that cannot be read, the span roots a trace of its own, as it always did. The header is never a
//! field of the span, so it never reaches the file.
//!
//! Who records a failure is decided once per error variant, in each error enum's [`CommandError`] implementation —
//! one exhaustive `match` with no wildcard, so a variant added later does not compile until someone has decided.

// Not Tauri's own `tracing` feature: its `ipc::request` spans are at `debug`, which the collector's `info` floor drops,
// and it never sees a command's typed result. Nor a wrapper around the `invoke_handler` closure, which returns before
// an async command's body has run. Nor `#[tracing::instrument]`, which cannot record how the command ended and would
// record every argument as a field the file then writes onto `opai`'s records.

use std::fmt::Display;
use std::future::Future;

use tauri::Runtime;
use tauri::ipc::{CommandArg, CommandItem, InvokeError};
use tracing::{Instrument, Span};

/// The target every command span, and the wrapper's own record, is emitted under.
pub(crate) const TARGET: &str = "gui::command";

/// The header a traced request names the window's trace in, as `ipc/invoke.ts` sends it.
const TRACEPARENT: &str = "traceparent";

/// The `traceparent` header of the request being served, if the window sent one. Never fails a request.
///
/// A command argument rather than [`tauri::ipc::Request`], which borrows and so forces an async command to answer a
/// `Result`, and rather than a read in the `invoke_handler` closure, which returns before an async command's body runs.
/// Owned, so an async command's body stays `'static`.
#[derive(Default)]
pub(crate) struct Traceparent(Option<String>);

impl Traceparent {
    /// The header's text, for `opai::telemetry::with_parent`, which decides whether it can be read.
    pub(crate) fn as_deref(&self) -> Option<&str> {
        self.0.as_deref()
    }
}

impl<'a, R: Runtime> CommandArg<'a, R> for Traceparent {
    fn from_command(command: CommandItem<'a, R>) -> Result<Self, InvokeError> {
        // A header that is not text is one the window did not send, and is read as none.
        let header = command.message.headers().get(TRACEPARENT).and_then(|value| value.to_str().ok());

        Ok(Self(header.map(str::to_owned)))
    }
}

/// Opens a command's span: named after the command, at `info`, under [`TARGET`], in the trace the request's
/// [`Traceparent`] names, with `outcome` and `error` left for the wrapper to record, and `run` where the command
/// carries one.
///
/// `run` is the only field that reaches the file, because it is the only one set while the body runs. `outcome` and
/// `error` are declared `Empty` and recorded after it, when no record inside can pick them up. The header is not a
/// field at all.
///
/// The header is the second argument of every form, so a command that does not take one does not compile. The name
/// stays first, where `every_registered_command_opens_its_span` reads it.
macro_rules! command_span {
    ($name:literal, $traceparent:expr) => {
        opai::telemetry::with_parent($traceparent.as_deref(), || {
            tracing::info_span!(
                target: $crate::command::TARGET,
                $name,
                outcome = tracing::field::Empty,
                error = tracing::field::Empty
            )
        })
    };
    ($name:literal, $traceparent:expr, run = $run:expr) => {
        opai::telemetry::with_parent($traceparent.as_deref(), || {
            tracing::info_span!(
                target: $crate::command::TARGET,
                $name,
                outcome = tracing::field::Empty,
                error = tracing::field::Empty,
                run = %$run
            )
        })
    };
}

pub(crate) use command_span;

/// How a command ended.
pub(crate) enum Ended<'a> {
    /// It did what was asked.
    Finished,
    /// It was withdrawn, by the window or by the application shutting down. Never marked as failed.
    Stopped,
    /// It failed.
    Failed {
        /// Why, as the window is told.
        error: &'a dyn Display,
        /// Whether `opai` already recorded this failure inside the command's span. Where it did not, the wrapper does.
        recorded: bool,
    },
}

#[cfg(test)]
impl Ended<'_> {
    /// Which column of design.md D4's classification table this ending is in.
    pub(crate) fn cell(&self) -> &'static str {
        match self {
            Self::Finished => "finished",
            Self::Stopped => "stopped",
            Self::Failed { recorded: true, .. } => "recorded by opai",
            Self::Failed { recorded: false, .. } => "recorded by the wrapper",
        }
    }
}

/// What a command answers, read for how it ended.
pub(crate) trait Outcome {
    fn ended(&self) -> Ended<'_>;
}

/// A command's successful answer. Finished, unless the answer is itself a stop.
pub(crate) trait Answer {
    fn ended(&self) -> Ended<'_> {
        Ended::Finished
    }
}

/// A command's error. No default: every error enum decides, variant by variant, whether it is a stop, a failure `opai`
/// recorded, or a failure only this crate saw.
pub(crate) trait CommandError: Display {
    fn ended(&self) -> Ended<'_>;
}

impl<T: Answer, E: CommandError> Outcome for Result<T, E> {
    fn ended(&self) -> Ended<'_> {
        match self {
            Ok(answer) => Answer::ended(answer),
            Err(error) => CommandError::ended(error),
        }
    }
}

impl Answer for () {}

/// The answers that cannot fail, each of which is a finished request.
macro_rules! finished {
    ($($answer:ty),+ $(,)?) => {
        $(
            impl Outcome for $answer {
                fn ended(&self) -> Ended<'_> {
                    Ended::Finished
                }
            }
        )+
    };
}

finished!(
    (),
    bool,
    f64,
    crate::export::ExportFormats,
    Option<String>,
    &'static str,
    &'static [opai::FamilyEntry],
    Vec<&'static str>
);

/// Runs `body` inside `span`, then records on the span how it ended.
pub(crate) async fn traced<T: Outcome>(span: Span, body: impl Future<Output = T>) -> T {
    let answer = body.instrument(span.clone()).await;
    close(&span, &answer);

    answer
}

/// [`traced`], for a synchronous command.
pub(crate) fn traced_sync<T: Outcome>(span: Span, body: impl FnOnce() -> T) -> T {
    let answer = span.in_scope(body);
    close(&span, &answer);

    answer
}

/// Records how `answer` ended on `span`, and the failure where nothing else recorded it.
fn close(span: &Span, answer: &impl Outcome) {
    match answer.ended() {
        Ended::Finished => {
            span.record("outcome", "finished");
        }
        Ended::Stopped => {
            span.record("outcome", "stopped");
        }
        Ended::Failed { error, recorded } => {
            // Before `error` and `outcome` are recorded, so the file writes only `run` from the span onto this record.
            if !recorded {
                let command = span.metadata().map_or("unknown", |metadata| metadata.name());

                span.in_scope(|| tracing::warn!(target: TARGET, command, %error, "a request from the window failed"));
            }

            // `error` is what the collector's bridge turns into a failed status.
            span.record("error", tracing::field::display(error));
            span.record("outcome", "failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use tracing::Level;

    use super::*;
    use crate::test_support::recorded;

    /// An error that decides its own recording, so each ending can be driven on its own.
    #[derive(Debug)]
    enum Probe {
        Stopped,
        Recorded,
        Unrecorded,
    }

    impl Display for Probe {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "the probe failed: {self:?}")
        }
    }

    impl CommandError for Probe {
        fn ended(&self) -> Ended<'_> {
            match self {
                Self::Stopped => Ended::Stopped,
                Self::Recorded => Ended::Failed { error: self, recorded: true },
                Self::Unrecorded => Ended::Failed { error: self, recorded: false },
            }
        }
    }

    /// A request whose window named its trace.
    const WINDOW: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

    /// No header, as from a window that is not tracing.
    fn untraced() -> Traceparent {
        Traceparent(None)
    }

    /// `answer`, traced under a `probe` span carrying `run-1`.
    fn probe(answer: Result<(), Probe>) -> crate::test_support::Recorded {
        recorded(|| {
            tauri::async_runtime::block_on(traced(command_span!("probe", untraced(), run = "run-1"), async { answer }))
        })
        .0
    }

    #[test]
    fn a_finished_request_records_finished() {
        let recorded = probe(Ok(()));
        let span = recorded.only("probe");

        assert_eq!(span.target, TARGET);
        assert_eq!(span.field("run"), Some("run-1"));
        assert_eq!(span.field("outcome"), Some("finished"));
        assert_eq!(span.field("error"), None);
        assert!(recorded.events.is_empty(), "a finished request wrote a record: {:#?}", recorded.events);
    }

    #[test]
    fn a_stopped_request_records_stopped_and_is_not_marked_failed() {
        let recorded = probe(Err(Probe::Stopped));
        let span = recorded.only("probe");

        assert_eq!(span.field("outcome"), Some("stopped"));
        assert_eq!(span.field("error"), None, "a stop was marked as a failure");
        assert!(recorded.events.is_empty(), "a stop wrote a record: {:#?}", recorded.events);
    }

    #[test]
    fn a_failure_the_library_recorded_is_marked_and_not_recorded_again() {
        let recorded = probe(Err(Probe::Recorded));
        let span = recorded.only("probe");

        assert_eq!(span.field("outcome"), Some("failed"));
        assert_eq!(span.field("error"), Some("the probe failed: Recorded"));
        assert!(
            recorded.events.is_empty(),
            "a failure `opai` recorded was recorded again: {:#?}",
            recorded.events
        );
    }

    #[test]
    fn a_failure_only_this_crate_saw_is_recorded_once_inside_the_span_before_it_is_marked() {
        let recorded = probe(Err(Probe::Unrecorded));
        let span = recorded.only("probe");

        let [warning] = recorded.events.as_slice() else {
            panic!("expected exactly one record: {:#?}", recorded.events);
        };
        assert_eq!(warning.level, Level::WARN);
        assert_eq!(warning.target, TARGET);
        assert_eq!(warning.span, Some(span.index), "the record was not emitted inside the command's span");
        assert_eq!(warning.field("command"), Some("probe"));
        assert_eq!(warning.field("error"), Some("the probe failed: Unrecorded"));

        // What the file writes onto the record from the span: the run, and nothing the wrapper records afterwards.
        let written: Vec<_> = warning.span_fields.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(written, ["run"], "the record picked up the span's ending: {:?}", warning.span_fields);

        assert_eq!(span.field("outcome"), Some("failed"));
        assert_eq!(span.field("error"), Some("the probe failed: Unrecorded"));
    }

    #[test]
    fn a_synchronous_command_and_one_without_a_run_are_traced_alike() {
        let (recorded, answer) =
            recorded(|| traced_sync(command_span!("probe", untraced()), || Err::<(), _>(Probe::Unrecorded)));
        assert!(answer.is_err());

        let span = recorded.only("probe");
        assert_eq!(span.field("run"), None);
        assert_eq!(span.field("outcome"), Some("failed"));
        assert_eq!(recorded.at(Level::WARN).len(), 1);
    }

    #[test]
    fn work_inside_the_body_is_beneath_the_commands_span() {
        let (recorded, ()) = recorded(|| {
            traced_sync(command_span!("probe", untraced()), || {
                let _unit = tracing::info_span!(target: "opai::unit", "image_load").entered();
                tracing::info!("inside");
            });
        });

        let command = recorded.only("probe");
        let unit = recorded.only("image_load");
        assert_eq!(unit.parent, Some(command.index));
        assert_eq!(recorded.events[0].span, Some(unit.index));
    }

    #[test]
    fn a_span_opened_under_a_header_records_only_its_ending_and_run() {
        let traceparent = Traceparent(Some(WINDOW.to_string()));
        let (recorded, answer) = recorded(|| {
            traced_sync(command_span!("probe", traceparent, run = "run-1"), || {
                tracing::info!("inside a window's trace");
                Err::<(), _>(Probe::Recorded)
            })
        });
        assert!(answer.is_err());

        let span = recorded.only("probe");
        let fields: Vec<_> = span.fields.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(fields, ["run", "error", "outcome"], "the span carries more than its ending and run: {span:?}");

        // What the file writes onto a record inside: the run, and nothing of the window's trace.
        let [inside] = recorded.events.as_slice() else {
            panic!("expected exactly one record: {:#?}", recorded.events);
        };
        assert_eq!(inside.span_fields, [("run".to_string(), "run-1".to_string())]);
        assert!(!format!("{:?}", recorded.events).contains("4bf92f3577b34da6a3ce929d0e0e4736"));
    }

    // ── Reading the header ────────────────────────────────────────────────────────────────────────────────────────

    /// What `echo_traceparent` answers when the window sends `headers`, through Tauri's own invoke path.
    fn read_through_an_invoke(headers: tauri::http::HeaderMap) -> Option<String> {
        #[tauri::command]
        fn echo_traceparent(traceparent: Traceparent) -> Option<String> {
            traceparent.0
        }

        let app = tauri::test::mock_builder()
            .invoke_handler(tauri::generate_handler![echo_traceparent])
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("the mock app should build");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("the mock window should build");

        let request = tauri::webview::InvokeRequest {
            cmd: "echo_traceparent".into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().expect("a URL"),
            body: tauri::ipc::InvokeBody::default(),
            headers,
            invoke_key: tauri::test::INVOKE_KEY.to_string(),
        };

        tauri::test::get_ipc_response(&webview, request)
            .expect("reading the header never fails a request")
            .deserialize()
            .expect("an optional string")
    }

    #[test]
    fn the_header_is_read_when_the_window_sent_it() {
        let mut headers = tauri::http::HeaderMap::new();
        headers.insert(TRACEPARENT, tauri::http::HeaderValue::from_static(WINDOW));

        assert_eq!(read_through_an_invoke(headers).as_deref(), Some(WINDOW));
    }

    #[test]
    fn no_header_reads_as_none() {
        assert_eq!(read_through_an_invoke(tauri::http::HeaderMap::new()), None);
    }

    #[test]
    fn a_header_that_is_not_text_reads_as_none_and_fails_nothing() {
        let mut headers = tauri::http::HeaderMap::new();
        let not_text = tauri::http::HeaderValue::from_bytes(b"00-\xff\xfe-01").expect("a header may carry bytes");
        headers.insert(TRACEPARENT, not_text);

        assert_eq!(read_through_an_invoke(headers), None);
    }
}
