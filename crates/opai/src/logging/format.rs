//! What one line in the log file looks like, and which records reach it.
//!
//! ```text
//! time=2026-09-14T10:04:12.481+02:00 level=INFO app=gui target=opai msg="Open Photo AI starting" version=26.9.0
//! time=2026-09-14T10:04:19.902+02:00 level=WARN app=gui target=ort::logging msg="Some nodes were not assigned..." id=io.vinicius.opai location="session_state.cc:1166 VerifyEachNodeIsAssignedToAnEp"
//! ```
//!
//! `time`, `level`, `app`, `target`, `msg`, then the event's own fields, then the fields of the spans enclosing it.
//! That is `slog.NewTextHandler`'s output — what the reference implementation writes — with two insertions, so a user
//! who has read the Go application's log can read this one and a support instruction written for either works for
//! both.
//!
//! # `app=` is why a custom format exists at all
//!
//! More than one application writes to one file — this project's own binaries, and anything embedding this library
//! under the same name — and the single-instance claim does not exclude every overlap: a process refused by it has
//! already installed its sink and written a header by the time it is refused. So every record has to say which
//! application produced it — including the records this project did not emit, which is the part that decides the
//! implementation.
//!
//! `tracing` has no concept of a field constant across a subscriber. Fields belong to events and spans, and a span's
//! fields reach an event only on the thread that entered the span — which the ORT callback's thread and every Tokio
//! worker are not. So the value is held by the formatter and written onto every line, which is both the simplest
//! implementation and the only one that covers a record from ONNX Runtime.
//!
//! The value is the identity the process declared, which is the same one a single-instance refusal names the holder
//! with: one identity, resolved once in [`Opai::initialize`](crate::Opai::initialize), rather than a lock-holder
//! string and a logging value that can disagree. This project's binaries declare [`GUI`](crate::GUI),
//! [`CLI`](crate::CLI) or [`PERF`](crate::PERF); an application embedding this library carries its own.
//!
//! # `target=` is an addition over the reference
//!
//! It is what makes a record self-identifying: `ort::logging` against `opai::…`, without the `source=onnxruntime`
//! attribute the Go parser had to attach by hand after recognising the format it had just parsed.
//!
//! # Two floors under the runtime's volume
//!
//! [`filter`] defaults to `info,ort=warn`. The second directive is there because the runtime's levels are not like
//! this project's: ORT at informational logs per-node assignment for *every node in the graph*, so a user who sets
//! `RUST_LOG=debug` to see this project's cache decisions would otherwise get tens of thousands of lines about a
//! model's internals. The C++ side is clamped to warning independently by
//! [`runtime::start`](crate::runtime), so this is the second of two floors rather than the only one.
//!
//! The collector has the same default as a floor of its own, which `RUST_LOG` does not move: the environment is for
//! one user chasing a problem in their own file, not for how much every installation sends.
//!
//! # A record the application keeps out of the collector
//!
//! An event that declares a field named [`FILE_ONLY`](super::FILE_ONLY) is written to the file and never sent. It is
//! for a record whose account reaches the collector by another path — a window that sends its own failures — so the
//! collector does not receive the same fact twice. The check is joined to the collector's filter outside its
//! directives, as the file's exclusion of unit spans is, and reads only the callsite's field names, so its answer is
//! cached per callsite rather than paid per record. The file writes the field like any other, which tells a reader
//! exactly why the collector does not have the line.
//!
//! # The runtime's record span gets past both
//!
//! `ort` wraps each diagnostic in a TRACE span carrying its `id` and `location`, and emits the record with that span as
//! explicit parent. `ort=warn` disables the span on its own, and a disabled parent is no parent: the runtime's records
//! used to reach the file with neither field. So each destination's filter is widened by exactly that span's shape
//! ([`is_ort_record_span`]). The file never writes a span on its own, only its fields into the records inside it, and
//! the collector folds it into its records rather than exporting it, so enabling it adds nothing but the two fields.

use std::fmt::{self, Write as _};

use rust_sak::o11y;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Metadata, Subscriber};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::field::RecordFields;
use tracing_subscriber::filter::{FilterExt, filter_fn};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::{ChronoLocal, FormatTime};
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, FormattedFields};
use tracing_subscriber::layer::{Filter, Layer, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;

use crate::telemetry::{EXPORT_TARGET, unit};

/// What is recorded when nothing in the environment says otherwise.
///
/// `info` for this project, and the runtime floored at `warn` separately. See this module's header for why the two
/// are separate directives.
const DEFAULT_DIRECTIVES: &str = "info,ort=warn";

/// The environment variable that overrides it for one run.
///
/// `RUST_LOG` rather than an `OPAI_LOG` of this project's own: it is the ecosystem's variable, it costs nothing, and a
/// user being walked through a bug report can be given a command any Rust developer would recognise.
const FILTER_ENV: &str = "RUST_LOG";

/// The filter the sink is installed with.
///
/// [`DEFAULT_DIRECTIVES`] unless `RUST_LOG` is set and parses. A `RUST_LOG` that does **not** parse falls back to the
/// default rather than failing the installation or silently recording nothing: a typo in an environment variable
/// should cost the extra detail that was asked for, not the log.
pub(super) fn filter() -> EnvFilter {
    EnvFilter::try_from_env(FILTER_ENV).unwrap_or_else(|_| EnvFilter::new(DEFAULT_DIRECTIVES))
}

/// What is sent to the collector, whatever the environment asks the file for.
///
/// The file's own default, plus `off` for the records telemetry writes about its own export failures: sending those
/// would queue a record about the queue failing onto the queue that is failing. Never read from `RUST_LOG`, which is
/// for one user chasing a problem in their own file and must not decide how much every installation sends.
fn grafana_directives() -> String {
    format!("{DEFAULT_DIRECTIVES},{EXPORT_TARGET}=off")
}

/// The subscriber the sink is installed as: a registry carrying two layers, each under its own filter.
///
/// - **The file**: [`Line`] over [`KeyValues`], writing into `writer`, filtered by `filter` — [`filter`] in a real
///   installation.
/// - **The collector**: `rust-sak`'s `o11y` bridge, filtered by [`grafana_directives`]. It is installed in every
///   binary, and costs one atomic load per record until [`telemetry::start`](crate::telemetry::start) opens `o11y`'s
///   gate, so whether anything is sent is decided there rather than by which subscriber was installed.
///
/// Both filters also let [`is_ort_record_span`] through, which is what carries the runtime's `id` and `location` onto
/// its records. The file's also shuts out every [unit span](crate::telemetry::unit), which is the collector's alone.
///
/// One constructor rather than a builder spelled out at each call site, so that [`init`](super::init) and the tests
/// that assert on the format cannot drift into rendering two different files.
pub(super) fn subscriber<W>(app: &str, writer: W, filter: EnvFilter) -> impl Subscriber + Send + Sync
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    tracing_subscriber::registry().with(file_layer(app, writer, filter)).with(grafana_layer())
}

/// [`subscriber`], with `observer` standing beside the collector's layer under the collector's filter.
///
/// The same two layers, so a test asserting on spans runs against the file and the filters a real installation has.
#[cfg(test)]
pub(super) fn observed_subscriber<W>(
    app: &str,
    writer: W,
    filter: EnvFilter,
    observer: super::observed::Observer,
) -> impl Subscriber + Send + Sync
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    tracing_subscriber::registry()
        .with(file_layer(app, writer, filter))
        .with(grafana_layer())
        .with(observer.with_filter(grafana_filter()))
}

/// The file: [`Line`] over [`KeyValues`], into `writer`, under [`file_filter`].
fn file_layer<S, W>(app: &str, writer: W, filter: EnvFilter) -> impl Layer<S> + Send + Sync
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    tracing_subscriber::fmt::layer()
        .with_writer(writer)
        // Both halves of the line, and they have to be set together: the event's own fields go through [`Line`], and
        // an enclosing span's go through this, at the moment the span is created. A default field formatter here
        // would quote a span's values with `Debug` while the event's were quoted the way `slog` quotes them, so one
        // record would carry two conventions.
        .fmt_fields(KeyValues)
        .event_format(Line::for_app(app))
        .with_filter(file_filter(filter))
}

/// The collector: `rust-sak`'s bridge, under [`grafana_filter`].
fn grafana_layer<S>() -> impl Layer<S> + Send + Sync
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    // Folded rather than exported: the runtime's span only carries context, and exported it would be one zero-length
    // trace per diagnostic. Its fields land on the record instead, which is where the file has them too.
    o11y::tracing::layer().fold_spans(is_ort_record_span).with_filter(grafana_filter())
}

/// The file layer's filter: `filter` plus the runtime's record span, and never a unit span.
///
/// The exclusion is joined outside `filter` rather than written into it as a directive, so no `RUST_LOG` can undo it.
/// A span this filter disables is invisible to the file layer, so [`Line`]'s walk of a record's scope passes over it
/// and none of its fields reaches the line.
fn file_filter<S>(filter: EnvFilter) -> impl Filter<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    with_ort_record_span(filter).and(filter_fn(|metadata| !is_unit_span(metadata)))
}

/// Whether `metadata` is a [unit span](crate::telemetry::unit), which the collector receives and the file does not.
fn is_unit_span(metadata: &Metadata<'_>) -> bool {
    metadata.is_span() && metadata.target() == unit::TARGET
}

/// The collector layer's filter: [`grafana_directives`], plus the runtime's record span, and never a record marked
/// [`FILE_ONLY`](super::FILE_ONLY).
fn grafana_filter<S>() -> impl Filter<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    with_ort_record_span(EnvFilter::new(grafana_directives())).and(filter_fn(|metadata| !is_file_only(metadata)))
}

/// Whether `metadata` is an event marked as the file's alone.
fn is_file_only(metadata: &Metadata<'_>) -> bool {
    metadata.is_event() && metadata.fields().field(super::FILE_ONLY).is_some()
}

/// `filter`, widened to also enable [`is_ort_record_span`].
///
/// `EnvFilter` cannot say this itself: `target[span]=level` filters the events *inside* a span, not the span. The
/// predicate reads metadata alone, so its interest is cached per callsite and the widening costs nothing on any other
/// record.
fn with_ort_record_span<S>(filter: EnvFilter) -> impl Filter<S>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    filter.or(filter_fn(is_ort_record_span))
}

/// Whether `metadata` is the span `ort` wraps each of the runtime's diagnostics in.
///
/// `ort-2.0.0-rc.13/src/logging.rs` builds it as `span!(Level::TRACE, "ort", id, location)` under its module target
/// and emits the record with it as explicit parent. At TRACE, the `ort=warn` floor disables it, and a disabled parent
/// is no parent: the record then arrives with neither field, which is how the file lost them. Matched by exact shape,
/// so an `ort` bump that changes it fails the tests built on that shape rather than silently losing the fields again.
fn is_ort_record_span(metadata: &Metadata<'_>) -> bool {
    metadata.is_span() && metadata.target() == "ort::logging" && metadata.name() == "ort"
}

/// Renders a span's fields, so that they read the same as an event's.
///
/// `tracing_subscriber::fmt` stores each span's formatted fields once, when the span is created, and [`Line`] reads
/// them back out of the span's extensions. This is what does that formatting — the same [`Fields`] visitor the
/// events go through, so `id=opai` is `id=opai` wherever it came from.
#[derive(Debug, Clone, Copy)]
pub(super) struct KeyValues;

impl<'writer> FormatFields<'writer> for KeyValues {
    fn format_fields<R: RecordFields>(&self, mut writer: Writer<'writer>, fields: R) -> fmt::Result {
        let mut visitor = Fields::new(&mut writer);
        fields.record(&mut visitor);
        visitor.finish()
    }
}

/// The formatter, holding the one value that is on every line and is not on any event.
#[derive(Debug, Clone)]
pub(super) struct Line {
    /// The identity this process declared, written onto every line.
    app: String,
    /// The clock `time=` is read off, built once rather than per record.
    clock: ChronoLocal,
}

impl Line {
    /// The formatter for records produced by `app`.
    ///
    /// Owned rather than borrowed: the subscriber outlives the call that installed it, and the identity is one short
    /// string copied once per process.
    pub(super) fn for_app(app: &str) -> Self {
        Self { app: app.to_string(), clock: clock() }
    }
}

impl<S, N> FormatEvent<S, N> for Line
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    /// Writes one record.
    ///
    /// **Nothing here may panic.** ONNX Runtime's callback runs this on threads this project did not start, under
    /// `extern "system"` frames that cannot accept an unwind — so every write goes into the `fmt::Writer` and every
    /// failure is the caller's to discard, exactly as `tracing_subscriber::fmt` already does.
    fn format_event(&self, ctx: &FmtContext<'_, S, N>, mut writer: Writer<'_>, event: &Event<'_>) -> fmt::Result {
        let metadata = event.metadata();

        writer.write_str("time=")?;
        self.clock.format_time(&mut writer)?;
        write!(writer, " level={} ", level_name(*metadata.level()))?;
        write!(writer, "app={} ", self.app)?;
        write!(writer, "target={}", metadata.target())?;

        // The message and the event's own fields in one pass, because `message` is an ordinary field to `tracing` and
        // is only distinguished by its name. The visitor writes `msg=` for it and `key=` for everything else, which
        // is what puts the message before the fields without the event being walked twice.
        let mut visitor = Fields::new(&mut writer);
        event.record(&mut visitor);
        visitor.finish()?;

        // Then the enclosing spans' fields, which is how the `id` and `location` that `ort`'s callback attaches to
        // its span reach the line. Outermost first, so a record reads from the broadest context inwards.
        if let Some(scope) = ctx.event_scope() {
            for span in scope.from_root() {
                let extensions = span.extensions();
                // No separator of its own: [`Fields`] writes a leading space before every field, so a span's
                // formatted block already begins with one.
                if let Some(fields) = extensions.get::<FormattedFields<N>>()
                    && !fields.is_empty()
                {
                    write!(writer, "{fields}")?;
                }
            }
        }

        writeln!(writer)
    }
}

/// `INFO` rather than `tracing`'s own `Display`, which pads to five characters.
///
/// The padding is there to align a terminal; this is a file whose lines are `key=value` throughout, and a trailing
/// space inside a value is the kind of thing that makes a file awkward to grep.
fn level_name(level: Level) -> &'static str {
    match level {
        Level::TRACE => "TRACE",
        Level::DEBUG => "DEBUG",
        Level::INFO => "INFO",
        Level::WARN => "WARN",
        Level::ERROR => "ERROR",
    }
}

/// Writes an event's fields as ` key=value`, with `message` rendered as `msg=`.
struct Fields<'a, 'b> {
    /// Where the fields go.
    writer: &'a mut Writer<'b>,
    /// The first failure, kept so that the rest of the record is still attempted and the caller still hears about it.
    ///
    /// [`Visit`] cannot fail — its methods return `()` — so an error has to be carried out of the walk rather than
    /// returned from it.
    result: fmt::Result,
}

impl<'a, 'b> Fields<'a, 'b> {
    fn new(writer: &'a mut Writer<'b>) -> Self {
        Self { writer, result: Ok(()) }
    }

    /// Whether every field was written.
    fn finish(self) -> fmt::Result {
        self.result
    }

    /// Writes one `key=value`, keeping the first failure.
    fn write(&mut self, key: &str, value: fmt::Arguments<'_>) {
        if self.result.is_err() {
            return;
        }

        // `message` is the one field with a name of its own in the output. Everything else keeps the name the caller
        // gave it.
        let key = if key == "message" { "msg" } else { key };
        self.result = write!(self.writer, " {key}={}", Quoted(value));
    }
}

impl Visit for Fields<'_, '_> {
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.write(field.name(), format_args!("{value}"));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.write(field.name(), format_args!("{value}"));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.write(field.name(), format_args!("{value}"));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.write(field.name(), format_args!("{value}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.write(field.name(), format_args!("{value}"));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.write(field.name(), format_args!("{value}"));
    }

    /// `Debug` is the fallback every unspecialised field arrives through.
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        // `{:?}` on a `&str` would quote and escape it a second time, so strings go through `record_str` below and
        // this is left for everything that has no better rendering.
        self.write(field.name(), format_args!("{value:?}"));
    }
}

/// A value quoted the way `slog`'s text handler quotes one.
///
/// Bare when it is a run of ordinary characters, and `"…"` with `"` and `\` escaped when it is empty or contains
/// anything that would end the value early — which is what keeps `msg="some nodes were not assigned"` one field
/// rather than four.
struct Quoted<'a>(fmt::Arguments<'a>);

/// `value` rendered as a field value is rendered in a record.
///
/// For the tests that look a path up in a log. A path is the one value whose rendering is not the same on every
/// platform - a Windows one carries `\\`, so it is quoted and escaped where the POSIX one is written bare - and a
/// test that builds its own needle would be asserting the rule a second time, differently, and only on the platform
/// whose spelling it happened to be written on.
#[cfg(test)]
pub(crate) fn as_field_value(value: &str) -> String {
    Quoted(format_args!("{value}")).to_string()
}

impl fmt::Display for Quoted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Rendered once into a `String` because the decision needs the whole value: whether to quote cannot be made
        // from a prefix. A record is a few short fields, so this is a small allocation on a path that then makes a
        // syscall.
        let value = self.0.to_string();

        let plain = !value.is_empty()
            && !value.chars().any(|c| c.is_whitespace() || c == '"' || c == '\\' || c == '=' || c.is_control());

        if plain {
            return f.write_str(&value);
        }

        f.write_str("\"")?;
        for c in value.chars() {
            match c {
                '"' => f.write_str("\\\"")?,
                '\\' => f.write_str("\\\\")?,
                '\n' => f.write_str("\\n")?,
                '\r' => f.write_str("\\r")?,
                '\t' => f.write_str("\\t")?,
                _ => f.write_char(c)?,
            }
        }
        f.write_str("\"")
    }
}

/// The local time a record was produced, to milliseconds, with the offset.
///
/// `2026-09-14T10:04:12.481+02:00` — RFC 3339, and local rather than UTC because the reader is the person who was
/// sitting at the machine, and the timestamps have to line up with when they remember the problem happening.
///
/// Through `tracing-subscriber`'s own [`ChronoLocal`] rather than by naming `chrono` here. The crate is in the graph
/// either way — `file-rotate` brings it, and the rotator decides the day boundary through it — so this keeps one
/// answer to what time it is and leaves this crate's manifest without a date library of its own.
const TIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.3f%:z";

/// The clock every record's `time=` is read off.
///
/// Built once and held by the formatter: [`ChronoLocal::new`] takes an owned format string, and rebuilding it per
/// record would allocate on a path that is otherwise one lock and one write.
fn clock() -> ChronoLocal {
    ChronoLocal::new(TIME_FORMAT.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CLI, GUI, PERF};
    use crate::logging::writer::{Rotator, Sink};
    use tracing::subscriber::with_default;

    /// Drives `emit` under a subscriber formatting through [`Line`] into a file, and returns what was written.
    ///
    /// A real [`Sink`] over a real file rather than an in-memory buffer, because the writer is part of what is being
    /// checked: a record has to survive the `MakeWriter` and the rotator to be in the file a user attaches.
    fn rendered(app: &str, directives: &str, emit: impl FnOnce()) -> String {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opai.log");
        let sink = Sink::new(Rotator::open(&path));

        with_default(subscriber(app, sink, EnvFilter::new(directives)), emit);

        std::fs::read_to_string(&path).unwrap()
    }

    /// The same, at the default directives.
    fn at_defaults(app: &str, emit: impl FnOnce()) -> String {
        rendered(app, DEFAULT_DIRECTIVES, emit)
    }

    #[test]
    fn a_record_carries_the_fields_in_the_specified_order() {
        let line = at_defaults(CLI, || tracing::info!(name = "opai", cache = "disk", "opai initialized"));

        // The order is the whole of the format's contract with a reader, so it is pinned by position rather than by
        // asking whether each part is present somewhere.
        let time = line.find("time=").unwrap();
        let level = line.find(" level=").unwrap();
        let app = line.find(" app=").unwrap();
        let target = line.find(" target=").unwrap();
        let msg = line.find(" msg=").unwrap();
        let name = line.find(" name=").unwrap();
        let cache = line.find(" cache=").unwrap();

        assert!(time < level && level < app && app < target && target < msg, "{line}");
        assert!(msg < name && name < cache, "the event's own fields are not after the message: {line}");

        assert!(line.contains("level=INFO "), "{line}");
        assert!(line.contains(r#"msg="opai initialized""#), "{line}");
        assert!(line.contains("name=opai"), "{line}");
        assert!(line.ends_with('\n'), "a record is not one line: {line:?}");
    }

    #[test]
    fn every_record_carries_the_application_as_the_single_instance_claim_spells_it() {
        // Over the three constants, so a fourth binary is covered by adding its constant in one place — and against
        // the same function a refusal names the holder with, so the two cannot drift.
        for app in [GUI, CLI, PERF] {
            let line = at_defaults(app, || tracing::info!("a record"));
            assert!(line.contains(&format!(" app={app} ")), "{app}: {line}");
        }
    }

    #[test]
    fn a_record_carries_an_embedders_own_identity_as_declared() {
        // The identity is no longer one of a fixed set, and what an embedder declared is what its records say —
        // spelled exactly as it was declared rather than mapped onto anything.
        for app in ["com.example.Photos", "io.vinicius.opai"] {
            let line = at_defaults(app, || tracing::info!("a record"));
            assert!(line.contains(&format!(" app={app} ")), "{app}: {line}");
        }
    }

    #[test]
    fn a_record_inside_a_span_renders_that_spans_fields() {
        // This is how ORT's records carry their `id` and `location`: the callback opens a span holding them and emits
        // the event with that span as its explicit parent, without entering it. The shape is `ort`'s own, from
        // `ort-2.0.0-rc.13/src/logging.rs`: a TRACE span, which the file's `ort=warn` floor would disable on its own.
        let line = at_defaults(GUI, || {
            let span = tracing::span!(
                target: "ort::logging",
                Level::TRACE,
                "ort",
                id = "io.vinicius.opai",
                location = "session_state.cc:1166"
            );
            tracing::event!(target: "ort::logging", parent: &span, Level::WARN, "some nodes were not assigned");
        });

        let record = line.lines().find(|l| l.contains("msg=")).unwrap();
        assert!(record.contains(r#"id=io.vinicius.opai"#), "{record}");
        assert!(record.contains("location=session_state.cc:1166"), "{record}");

        // After the message, which is where a reader looks for context rather than for the record itself.
        assert!(record.find(" msg=").unwrap() < record.find(" id=").unwrap(), "{record}");
    }

    #[test]
    fn a_runtime_diagnostic_reaches_the_file_with_everything_that_identifies_it() {
        // The shape `ort`'s C++ callback produces (`ort-2.0.0-rc.13/src/logging.rs`): an event under the target
        // `ort::logging`, whose explicit parent is a TRACE span carrying the runtime's own logger id and the location in
        // its source that produced the record. This is the one test that puts all three together — the target, the
        // span fields and the severity under the *default* filter — because that combination is what a reader of a bug
        // report actually needs and is what the `tracing` feature was turned on for. It cannot be driven through a real
        // runtime hermetically, so the span and the event are built exactly as the callback builds them.
        let line = at_defaults(GUI, || {
            let span = tracing::span!(
                target: "ort::logging",
                Level::TRACE,
                "ort",
                id = "io.vinicius.opai",
                location = "session_state.cc:1166 VerifyEachNodeIsAssignedToAnEp"
            );
            tracing::event!(
                target: "ort::logging",
                parent: &span,
                Level::WARN,
                "Some nodes were not assigned to the preferred execution providers"
            );
        });

        let record = line.lines().find(|l| l.contains("msg=")).expect("the diagnostic did not reach the file");

        // Self-identifying without an attribute anyone had to attach: the target says where it came from.
        assert!(record.contains(" target=ort::logging "), "{record}");
        assert!(record.contains("level=WARN"), "the runtime's severity was not preserved: {record}");
        assert!(record.contains(r#"msg="Some nodes were not assigned"#), "{record}");

        // The two fields the reference implementation reconstructed with a regex over ORT's text format, here having
        // never been text.
        assert!(record.contains("id=io.vinicius.opai"), "the record lost the runtime's logger id: {record}");
        assert!(
            record.contains(r#"location="session_state.cc:1166 VerifyEachNodeIsAssignedToAnEp""#),
            "the record lost the location in the runtime's source: {record}"
        );

        // And the record still says which application the process is, which is the whole reason the formatter
        // carries `app` rather than reading it off the event.
        assert!(record.contains(" app=gui "), "{record}");
    }

    #[test]
    fn a_value_is_quoted_when_it_would_otherwise_not_be_one_field() {
        let line = at_defaults(CLI, || {
            tracing::info!(
                plain = "disk",
                spaced = "two words",
                quoted = r#"he said "no""#,
                empty = "",
                "a message with spaces"
            )
        });

        assert!(line.contains(" plain=disk"), "an ordinary value was quoted: {line}");
        assert!(line.contains(r#" spaced="two words""#), "{line}");
        assert!(line.contains(r#" quoted="he said \"no\"""#), "{line}");
        assert!(line.contains(r#" empty="""#), "an empty value has to be visible: {line}");
        assert!(line.contains(r#" msg="a message with spaces""#), "{line}");
    }

    #[test]
    fn a_newline_in_a_value_cannot_split_a_record_in_two() {
        // The property behind the quoting: one record is one line, whatever is in it, or the divider count and every
        // other line-based reading of the file stops being true.
        let line = at_defaults(CLI, || tracing::error!(detail = "first\nsecond", "a failure"));

        assert_eq!(line.lines().count(), 1, "a value's newline split the record: {line:?}");
        assert!(line.contains(r#" detail="first\nsecond""#), "{line}");
    }

    #[test]
    fn the_default_keeps_info_and_drops_debug() {
        let line = at_defaults(CLI, || {
            tracing::debug!("a debug record");
            tracing::info!("an info record");
            tracing::warn!("a warning");
        });

        assert!(!line.contains("a debug record"), "debug was recorded under the default: {line}");
        assert!(line.contains("an info record"), "{line}");
        assert!(line.contains("a warning"), "{line}");
    }

    #[test]
    fn the_environment_can_ask_for_debug() {
        let line = rendered(CLI, "debug", || {
            tracing::debug!("a debug record");
            tracing::trace!("a trace record");
        });

        assert!(line.contains("a debug record"), "{line}");
        assert!(!line.contains("a trace record"), "{line}");
    }

    #[test]
    fn the_runtime_stays_at_warning_under_the_default() {
        // Two floors, and this is the subscriber's. The `ort` target at informational logs per-node assignment for
        // every node in a graph, which is what this keeps out of a file somebody is asked to attach.
        let line = at_defaults(CLI, || {
            tracing::info!(target: "ort", "a per-node assignment");
            tracing::warn!(target: "ort", "an execution provider declined to attach");
            tracing::info!(target: "opai", "this project's own record");
        });

        assert!(!line.contains("a per-node assignment"), "the runtime's info records reached the file: {line}");
        assert!(line.contains("an execution provider declined to attach"), "{line}");
        assert!(line.contains("this project's own record"), "{line}");
    }

    #[test]
    fn an_unreadable_filter_falls_back_to_the_default_rather_than_recording_nothing() {
        // A typo in an environment variable costs the extra detail that was asked for, not the log. Driven against
        // `EnvFilter` directly rather than by setting the variable, which is process-wide and shared with every other
        // test in this binary.
        let broken = EnvFilter::try_new("=====").err();
        assert!(broken.is_some(), "the test's own broken directive parsed");

        let filter = EnvFilter::try_new("=====").unwrap_or_else(|_| EnvFilter::new(DEFAULT_DIRECTIVES));
        assert_eq!(filter.to_string(), EnvFilter::new(DEFAULT_DIRECTIVES).to_string());
    }

    #[test]
    fn the_timestamp_is_local_time_to_milliseconds_with_an_offset() {
        let mut buffer = String::new();
        clock().format_time(&mut Writer::new(&mut buffer)).unwrap();
        let stamp = buffer;

        // `2026-09-14T10:04:12.481+02:00`, or `…Z`-less with a `-` offset. Checked by shape rather than by value,
        // since the value is the clock.
        assert_eq!(stamp.len(), "2026-09-14T10:04:12.481+02:00".len(), "{stamp}");
        assert_eq!(&stamp[4..5], "-");
        assert_eq!(&stamp[10..11], "T");
        assert_eq!(&stamp[19..20], ".", "the timestamp has no milliseconds: {stamp}");
        assert!(stamp[23..].starts_with(['+', '-']), "the timestamp carries no offset: {stamp}");
    }

    /// What `emit` sends to the collector's layer, beside the file's at `file_directives`: everything the collector's
    /// layer saw, in the form [`Observed`](super::super::observed::Observed) records.
    fn observed(file_directives: &str, emit: impl FnOnce()) -> super::super::observed::Observed {
        let observer = super::super::observed::Observer::default();
        let subscriber = observed_subscriber(CLI, std::io::sink, EnvFilter::new(file_directives), observer.clone());

        with_default(subscriber, emit);

        observer.observed()
    }

    /// What `emit` sends to the collector's layer, as `span <name>` or `<LEVEL> <target> <message>`.
    fn sent(file_directives: &str, emit: impl FnOnce()) -> Vec<String> {
        observed(file_directives, emit).log
    }

    #[test]
    fn the_collector_floor_does_not_move_when_the_file_asks_for_debug() {
        let sent = sent("debug", || {
            tracing::debug!("a debug record");
            tracing::info!("an info record");
        });

        assert!(!sent.iter().any(|r| r.contains("a debug record")), "{sent:?}");
        assert!(sent.iter().any(|r| r.contains("an info record")), "{sent:?}");
    }

    #[test]
    fn the_collector_takes_the_runtimes_warnings_and_not_its_information() {
        let sent = sent(DEFAULT_DIRECTIVES, || {
            tracing::info!(target: "ort::logging", "a per-node assignment");
            tracing::warn!(target: "ort::logging", "an execution provider declined to attach");
        });

        assert_eq!(sent, ["WARN ort::logging an execution provider declined to attach"]);
    }

    #[test]
    fn the_collector_is_given_the_runtimes_record_span() {
        let sent = sent(DEFAULT_DIRECTIVES, || {
            let span = tracing::span!(target: "ort::logging", Level::TRACE, "ort", id = "io.vinicius.opai");
            tracing::event!(target: "ort::logging", parent: &span, Level::WARN, "a diagnostic");
            // Any other TRACE span stays off: the widening is for that one shape.
            let _other = tracing::trace_span!(target: "ort::logging", "not_ort");
        });

        assert_eq!(sent, ["span ort", "WARN ort::logging a diagnostic"]);
    }

    #[test]
    fn the_collector_never_takes_telemetrys_own_export_failures() {
        let sent = sent("trace", || {
            tracing::error!(target: EXPORT_TARGET, "an error");
            tracing::warn!(target: EXPORT_TARGET, "a warning");
            tracing::info!(target: EXPORT_TARGET, "an info record");
            tracing::info!(target: "opai::telemetry", "telemetry started");
        });

        assert_eq!(sent, ["INFO opai::telemetry telemetry started"]);
    }

    #[test]
    fn a_record_marked_file_only_is_written_and_not_sent() {
        let emit = || {
            tracing::warn!(target: "gui::frontend", { { super::super::FILE_ONLY } = true }, "a window failure");
            tracing::warn!(target: "gui::frontend", "an unmarked failure");
        };

        let log = at_defaults(GUI, emit);
        let written = log.lines().find(|l| l.contains("a window failure")).expect("the marked record was not written");
        assert!(written.ends_with(" file_only=true"), "{written}");

        assert_eq!(sent(DEFAULT_DIRECTIVES, emit), ["WARN gui::frontend an unmarked failure"]);
    }

    #[test]
    fn a_record_inside_a_unit_span_carries_none_of_its_fields_even_at_debug() {
        use crate::telemetry::unit::unit_span;

        // `debug` rather than the default: the exclusion sits outside the environment's filter, so nothing a user
        // puts in `RUST_LOG` brings the collector's spans into their file.
        let log = rendered(CLI, "debug", || {
            let unit = unit_span!("enhancement", identity = "unit-span-identity", provider = "cpu");
            let _entered = unit.enter();
            tracing::info!(id = "dn_stockholm_fp32", "a record inside a unit");
            unit.record("outcome", "finished");

            // A context the file already had keeps decorating it, including inside a unit span.
            let build = tracing::info_span!("session_build", artifact = "dn_stockholm_fp32.onnx");
            build.in_scope(|| tracing::warn!("a runtime diagnostic during a build"));
        });

        let unit = log
            .lines()
            .find(|l| l.contains("a record inside a unit"))
            .expect("the record did not reach the file");
        assert!(unit.ends_with(" id=dn_stockholm_fp32"), "a unit span's field reached the line: {unit}");
        for field in ["identity=", "provider=", "outcome="] {
            assert!(!unit.contains(field), "{field} reached the line: {unit}");
        }

        let build = log.lines().find(|l| l.contains("a runtime diagnostic")).expect("the diagnostic did not arrive");
        assert!(build.ends_with(" artifact=dn_stockholm_fp32.onnx"), "{build}");
        assert!(!build.contains("identity="), "{build}");
    }

    #[test]
    fn the_collector_is_given_a_unit_span_with_the_file_at_its_default() {
        use crate::telemetry::unit::{TARGET, unit_span};

        let observed = observed(DEFAULT_DIRECTIVES, || {
            let unit = unit_span!("enhancement", identity = "abc");
            unit.in_scope(|| tracing::info!("inside the unit"));
        });

        let span = observed.only("enhancement");
        assert_eq!(span.target, TARGET);
        assert_eq!(span.field("identity"), Some("abc"));
        assert_eq!(observed.event("inside the unit").and_then(|event| event.span), Some(span.index));
    }

    #[test]
    fn a_level_renders_without_the_padding_a_terminal_would_want() {
        assert_eq!(level_name(Level::INFO), "INFO");
        assert_eq!(level_name(Level::WARN), "WARN");
        assert_eq!(level_name(Level::ERROR), "ERROR");
        assert_eq!(level_name(Level::DEBUG), "DEBUG");
        assert_eq!(level_name(Level::TRACE), "TRACE");

        for level in [Level::INFO, Level::WARN, Level::ERROR, Level::DEBUG, Level::TRACE] {
            assert_eq!(level_name(level).trim(), level_name(level), "a level is padded");
        }
    }
}
