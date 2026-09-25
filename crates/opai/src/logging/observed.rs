//! What reached the collector's layer in a test: spans with their parents, links and fields, and records.
//!
//! It stands beside `o11y`'s bridge under the same filter, because the bridge sends nothing until `o11y` is
//! initialised, which happens once per process, in `telemetry`'s export test. What is asserted here is what the
//! bridge would have been handed.

use std::fmt;
use std::sync::{Arc, Mutex};

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

use crate::task::lock;

/// The layer. Clones share what they observed.
#[derive(Debug, Clone, Default)]
pub(crate) struct Observer(Arc<Mutex<Observed>>);

impl Observer {
    /// A copy of everything observed so far.
    pub(crate) fn observed(&self) -> Observed {
        lock(&self.0).clone()
    }
}

/// Everything one [`Observer`] saw, in arrival order.
#[derive(Debug, Clone, Default)]
pub(crate) struct Observed {
    /// `span <name>` for each span and `<LEVEL> <target> <message>` for each record, as they arrived.
    pub(crate) log: Vec<String>,
    /// Every span, indexed by [`SeenSpan::index`].
    pub(crate) spans: Vec<SeenSpan>,
    /// Every record.
    pub(crate) events: Vec<SeenEvent>,
}

/// One span as the collector's layer saw it.
#[derive(Debug, Clone)]
pub(crate) struct SeenSpan {
    /// Its position in [`Observed::spans`]. Not `tracing`'s `Id`, which is reused once a span closes.
    pub(crate) index: usize,
    pub(crate) name: &'static str,
    pub(crate) target: &'static str,
    /// The nearest enclosing span this layer also saw.
    pub(crate) parent: Option<usize>,
    /// The spans this one was linked to with `follows_from`.
    pub(crate) links: Vec<usize>,
    /// Its fields, those it was created with and those recorded later, the later value winning.
    pub(crate) fields: Vec<(String, String)>,
    pub(crate) closed: bool,
}

impl SeenSpan {
    /// The value of `key`, or `None` where it was never recorded.
    pub(crate) fn field(&self, key: &str) -> Option<&str> {
        self.fields.iter().find(|(name, _)| name == key).map(|(_, value)| value.as_str())
    }
}

/// One record as the collector's layer saw it.
#[derive(Debug, Clone)]
pub(crate) struct SeenEvent {
    pub(crate) message: String,
    /// The span it was emitted in, where this layer saw one.
    pub(crate) span: Option<usize>,
}

impl Observed {
    /// Every span named `name`.
    pub(crate) fn named(&self, name: &str) -> Vec<&SeenSpan> {
        self.spans.iter().filter(|span| span.name == name).collect()
    }

    /// The one span named `name`, panicking with everything observed where there is not exactly one.
    pub(crate) fn only(&self, name: &str) -> &SeenSpan {
        match self.named(name).as_slice() {
            [span] => span,
            spans => panic!("expected one {name} span, found {}: {:#?}", spans.len(), self.spans),
        }
    }

    /// How many spans were created under `target`.
    pub(crate) fn count_under(&self, target: &str) -> usize {
        self.spans.iter().filter(|span| span.target == target).count()
    }

    /// The record whose message is `message`, where there is one.
    pub(crate) fn event(&self, message: &str) -> Option<&SeenEvent> {
        self.events.iter().find(|event| event.message == message)
    }
}

/// Which [`SeenSpan`] a `tracing` span is, kept in the span's extensions.
struct Index(usize);

impl<S> Layer<S> for Observer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let parent = span.parent().and_then(|parent| parent.extensions().get::<Index>().map(|index| index.0));

        let mut fields = Fields::default();
        attrs.record(&mut fields);

        let mut observed = lock(&self.0);
        let index = observed.spans.len();
        let metadata = attrs.metadata();
        observed.log.push(format!("span {}", metadata.name()));
        observed.spans.push(SeenSpan {
            index,
            name: metadata.name(),
            target: metadata.target(),
            parent,
            links: Vec::new(),
            fields: fields.0,
            closed: false,
        });
        drop(observed);

        span.extensions_mut().insert(Index(index));
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(index) = index_of(&ctx, id) else { return };
        let mut fields = Fields::default();
        values.record(&mut fields);

        let mut observed = lock(&self.0);
        let seen = &mut observed.spans[index].fields;
        for (name, value) in fields.0 {
            match seen.iter_mut().find(|(existing, _)| *existing == name) {
                Some((_, slot)) => *slot = value,
                None => seen.push((name, value)),
            }
        }
    }

    fn on_follows_from(&self, id: &Id, follows: &Id, ctx: Context<'_, S>) {
        if let (Some(index), Some(link)) = (index_of(&ctx, id), index_of(&ctx, follows)) {
            lock(&self.0).spans[index].links.push(link);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let message = fields.0.into_iter().find(|(name, _)| name == "message").map(|(_, value)| value);
        let message = message.unwrap_or_default();
        let span = ctx.event_span(event).and_then(|span| span.extensions().get::<Index>().map(|index| index.0));

        let metadata = event.metadata();
        let mut observed = lock(&self.0);
        observed.log.push(format!("{} {} {message}", metadata.level(), metadata.target()));
        observed.events.push(SeenEvent { message, span });
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        if let Some(index) = index_of(&ctx, &id) {
            lock(&self.0).spans[index].closed = true;
        }
    }
}

/// The [`SeenSpan`] index of `id`, where this layer saw it.
fn index_of<S>(ctx: &Context<'_, S>, id: &Id) -> Option<usize>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    ctx.span(id)?.extensions().get::<Index>().map(|index| index.0)
}

/// A field set rendered to strings: a string as it is, anything else through `Debug`, which is `Display` for a
/// `tracing::field::display` value.
#[derive(Default)]
struct Fields(Vec<(String, String)>);

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push((field.name().to_string(), value.to_string()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.0.push((field.name().to_string(), format!("{value:?}")));
    }
}
