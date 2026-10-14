//! A recorder the tests of more than one of this crate's modules read back what they traced with.
//!
//! The GUI keeps the same layer for its own tests, crate-private, so this crate keeps a trimmed copy of it rather than
//! widening a crate's API for a test: spans and records, with their fields, in arrival order.

use std::sync::{Arc, Mutex};

/// A layer that keeps every span and record it sees, in arrival order. Clones share what they saw.
#[derive(Debug, Clone, Default)]
pub struct Recorder(Arc<Mutex<Recorded>>);

impl Recorder {
    /// A copy of everything seen so far.
    pub fn recorded(&self) -> Recorded {
        self.0.lock().expect("unpoisoned").clone()
    }
}

/// Everything one [`Recorder`] saw.
#[derive(Debug, Clone, Default)]
pub struct Recorded {
    /// Every span, in the order they were opened.
    pub spans: Vec<SeenSpan>,
    /// Every record.
    pub events: Vec<SeenEvent>,
}

/// One span as the recorder saw it.
#[derive(Debug, Clone)]
pub struct SeenSpan {
    pub name: &'static str,
    pub target: &'static str,
    /// Its fields: those it was created with, then those recorded later, the later value winning. A field declared
    /// `Empty` is absent until it is recorded.
    pub fields: Vec<(String, String)>,
}

impl SeenSpan {
    /// The value of `key`, or `None` where it was never recorded.
    pub fn field(&self, key: &str) -> Option<&str> {
        field(&self.fields, key)
    }
}

/// One record as the recorder saw it.
#[derive(Debug, Clone)]
pub struct SeenEvent {
    pub level: tracing::Level,
    pub target: String,
    /// Its own fields, `message` among them.
    pub fields: Vec<(String, String)>,
}

impl SeenEvent {
    /// The value of `key` among the record's own fields.
    pub fn field(&self, key: &str) -> Option<&str> {
        field(&self.fields, key)
    }
}

impl Recorded {
    /// The one span named `name`, panicking with everything seen where there is not exactly one.
    pub fn only(&self, name: &str) -> &SeenSpan {
        match self.spans.iter().filter(|span| span.name == name).collect::<Vec<_>>().as_slice() {
            [span] => span,
            spans => panic!("expected one {name} span, found {}: {:#?}", spans.len(), self.spans),
        }
    }
}

/// `work`, run with a [`Recorder`] as this thread's subscriber, and what it saw.
///
/// Thread-local, so tests running in parallel do not see each other's records.
pub fn recorded<T>(work: impl FnOnce() -> T) -> (Recorded, T) {
    use tracing_subscriber::layer::SubscriberExt;

    static UNDECIDED: std::sync::Once = std::sync::Once::new();
    UNDECIDED.call_once(|| {
        // Ignored where something else already set one: any global subscriber keeps callsites from caching `never`.
        let _ = tracing::subscriber::set_global_default(Undecided);
        tracing::callsite::rebuild_interest_cache();
    });

    let recorder = Recorder::default();
    let answer = tracing::subscriber::with_default(tracing_subscriber::registry().with(recorder.clone()), work);
    (recorder.recorded(), answer)
}

/// The process's global subscriber under test: it records nothing, and never lets a callsite decide for good.
///
/// Without one, a callsite a parallel test reaches first on a thread with no subscriber of its own can cache
/// `Interest::never`, and its records are then dropped under every [`recorded`] for the rest of the process. See the
/// GUI's `test_support` for the whole account.
struct Undecided;

impl tracing::Subscriber for Undecided {
    fn register_callsite(&self, _: &'static tracing::Metadata<'static>) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::sometimes()
    }

    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        false
    }

    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        // Unreachable: nothing is enabled, so no span is ever created here.
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}

    fn event(&self, _: &tracing::Event<'_>) {}

    fn enter(&self, _: &tracing::span::Id) {}

    fn exit(&self, _: &tracing::span::Id) {}
}

/// Which [`SeenSpan`] a `tracing` span is, kept in the span's extensions.
struct Index(usize);

impl<S> tracing_subscriber::Layer<S> for Recorder
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let Some(span) = ctx.span(id) else { return };

        let mut fields = Fields::default();
        attrs.record(&mut fields);

        let mut recorded = self.0.lock().expect("unpoisoned");
        let index = recorded.spans.len();
        let metadata = attrs.metadata();
        recorded
            .spans
            .push(SeenSpan { name: metadata.name(), target: metadata.target(), fields: fields.0 });
        drop(recorded);

        span.extensions_mut().insert(Index(index));
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let Some(index) = ctx.span(id).and_then(|span| span.extensions().get::<Index>().map(|index| index.0)) else {
            return;
        };
        let mut fields = Fields::default();
        values.record(&mut fields);

        let mut recorded = self.0.lock().expect("unpoisoned");
        let seen = &mut recorded.spans[index].fields;
        for (name, value) in fields.0 {
            match seen.iter_mut().find(|(existing, _)| *existing == name) {
                Some((_, slot)) => *slot = value,
                None => seen.push((name, value)),
            }
        }
    }

    fn on_event(&self, event: &tracing::Event<'_>, _: tracing_subscriber::layer::Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);

        let metadata = event.metadata();
        self.0.lock().expect("unpoisoned").events.push(SeenEvent {
            level: *metadata.level(),
            target: metadata.target().to_string(),
            fields: fields.0,
        });
    }
}

/// The value of `key` in `fields`.
fn field<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fields.iter().find(|(name, _)| name == key).map(|(_, value)| value.as_str())
}

/// A field set rendered to strings: a string as it is, anything else through `Debug`, which is `Display` for a
/// `tracing::field::display` value.
#[derive(Default)]
struct Fields(Vec<(String, String)>);

impl tracing::field::Visit for Fields {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.0.push((field.name().to_string(), value.to_string()));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push((field.name().to_string(), format!("{value:?}")));
    }
}
