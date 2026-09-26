//! Fixtures the tests in more than one of this crate's modules need.

// Written rather than committed: these tests depend on no file in the repository and the source is a photograph the
// decoder actually opens.
//
// Crate tier because two modules need it: the enhancement chain's tests and `faces`' tests admit the same photograph
// the same way, and a copy beside the second caller is what this avoids.

use std::sync::{Arc, Mutex};

use crate::images::Opened;

/// A real image on disk, admitted, and the identity it was admitted under.
pub(crate) fn admitted(dir: &tempfile::TempDir, opened: &Opened) -> String {
    let path = dir.path().join("holiday.png");
    let pixels = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(8, 6, |x, y| {
        image::Rgb([(x * 8) as u8, (y * 8) as u8, 96])
    }));

    opai::image::save_blocking(&pixels, &path, opai::ImageFormat::Png, None).expect("writable");

    tauri::async_runtime::block_on(crate::images::files::describe_and_admit(vec![path.clone()], opened))
        .expect("the runtime is alive");

    opai::image::identity_blocking(&path).expect("a written file has an identity")
}

// Crate tier because every fake of the library's seams needs them: `faces`' detectors, and the enhancers in `enhance`
// and `export`.
/// The provider report every fake hands back. None of them is what any test asserts about; it is the shape `Enhanced`
/// and `Executed` require.
pub(crate) fn no_providers() -> opai::ProviderReport {
    opai::ProviderReport { requested: opai::ExecutionProvider::Auto, actual: Vec::new() }
}

/// What a fake detection hands back: `value`, and [`no_providers`].
pub(crate) fn executed<T>(value: T) -> opai::Executed<T> {
    opai::Executed { value, providers: no_providers() }
}

/// A face at the given box, with every landmark on its top-left corner, as a detector would report it.
pub(crate) fn boxed(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> opai::Face {
    use opai::{Confidence, Face, Point, Rect};

    Face::new(
        Rect::new(Point::new(min_x, min_y), Point::new(max_x, max_y)),
        [Point::new(min_x, min_y); Face::LANDMARKS],
        Confidence::new(0.9).expect("0.9 is inside the permitted range"),
    )
}

// Crate tier because two modules read back what they logged: `autopilot`'s incomplete analysis and `update`'s failed
// check.
/// A log sink the test can read back, shared between the subscriber and the assertions.
#[derive(Debug, Clone, Default)]
pub(crate) struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    /// Everything written so far.
    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().expect("unpoisoned")).into_owned()
    }
}

impl std::io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("unpoisoned").extend_from_slice(bytes);

        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Captured {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

// Crate tier because every module this crate wraps a command in reads back the spans it opened: `command` for the
// wrapper itself, `task` for the blocking hop, and each command's own tests for what its body records inside the span.
/// A layer that keeps every span and record it sees, with parents, in arrival order. Clones share what they saw.
///
/// `opai` keeps one of these for its own tests, crate-private, so this crate keeps its own rather than widening
/// `opai`'s API for a test.
#[derive(Debug, Clone, Default)]
pub(crate) struct Recorder(Arc<Mutex<Recorded>>);

impl Recorder {
    /// A copy of everything seen so far.
    pub(crate) fn recorded(&self) -> Recorded {
        self.0.lock().expect("unpoisoned").clone()
    }
}

/// Everything one [`Recorder`] saw.
#[derive(Debug, Clone, Default)]
pub(crate) struct Recorded {
    /// Every span, indexed by [`SeenSpan::index`].
    pub(crate) spans: Vec<SeenSpan>,
    /// Every record.
    pub(crate) events: Vec<SeenEvent>,
}

/// One span as the recorder saw it.
#[derive(Debug, Clone)]
pub(crate) struct SeenSpan {
    /// Its position in [`Recorded::spans`]. Not `tracing`'s `Id`, which is reused once a span closes.
    pub(crate) index: usize,
    pub(crate) name: &'static str,
    pub(crate) target: &'static str,
    /// The nearest enclosing span.
    pub(crate) parent: Option<usize>,
    /// Its fields: those it was created with, then those recorded later, the later value winning. A field declared
    /// `Empty` is absent until it is recorded.
    pub(crate) fields: Vec<(String, String)>,
}

impl SeenSpan {
    /// The value of `key`, or `None` where it was never recorded.
    pub(crate) fn field(&self, key: &str) -> Option<&str> {
        field(&self.fields, key)
    }
}

/// One record as the recorder saw it.
#[derive(Debug, Clone)]
pub(crate) struct SeenEvent {
    pub(crate) level: tracing::Level,
    pub(crate) target: String,
    /// Its own fields, `message` among them.
    pub(crate) fields: Vec<(String, String)>,
    /// The span it was emitted in.
    pub(crate) span: Option<usize>,
    /// That span's fields at the moment the record was emitted, which is what the file writes onto the record.
    pub(crate) span_fields: Vec<(String, String)>,
}

impl SeenEvent {
    /// The value of `key` among the record's own fields.
    pub(crate) fn field(&self, key: &str) -> Option<&str> {
        field(&self.fields, key)
    }
}

impl Recorded {
    /// The one span named `name`, panicking with everything seen where there is not exactly one.
    pub(crate) fn only(&self, name: &str) -> &SeenSpan {
        match self.spans.iter().filter(|span| span.name == name).collect::<Vec<_>>().as_slice() {
            [span] => span,
            spans => panic!("expected one {name} span, found {}: {:#?}", spans.len(), self.spans),
        }
    }

    /// Whether the span at `index` has `ancestor` somewhere above it.
    pub(crate) fn descends_from(&self, index: usize, ancestor: usize) -> bool {
        let mut parent = self.spans[index].parent;

        while let Some(current) = parent {
            if current == ancestor {
                return true;
            }
            parent = self.spans[current].parent;
        }

        false
    }

    /// The records at `level`.
    pub(crate) fn at(&self, level: tracing::Level) -> Vec<&SeenEvent> {
        self.events.iter().filter(|event| event.level == level).collect()
    }
}

/// `work`, run with a [`Recorder`] as this thread's subscriber, and what it saw.
///
/// Thread-local, so tests running in parallel do not see each other's records. Work handed to a blocking thread
/// through [`crate::task::spawn_blocking`] carries the subscriber with it.
pub(crate) fn recorded<T>(work: impl FnOnce() -> T) -> (Recorded, T) {
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
/// Without a global subscriber a callsite that a parallel test reaches first, on a thread with no subscriber of its
/// own, can cache `Interest::never` — `tracing-core` computes a callsite's interest before adding it to the list a new
/// subscriber's registration rebuilds, so a recorder installed in between never gets asked about it. Its spans are then
/// dropped under every [`recorded`] for the rest of the process. Answering `sometimes` for every callsite makes each
/// span ask the subscriber current on its thread instead, which is the recorder wherever one is installed.
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
        let parent = span.parent().and_then(|parent| parent.extensions().get::<Index>().map(|index| index.0));

        let mut fields = Fields::default();
        attrs.record(&mut fields);

        let mut recorded = self.0.lock().expect("unpoisoned");
        let index = recorded.spans.len();
        let metadata = attrs.metadata();
        recorded.spans.push(SeenSpan {
            index,
            name: metadata.name(),
            target: metadata.target(),
            parent,
            fields: fields.0,
        });
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

    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let span = ctx.event_span(event).and_then(|span| span.extensions().get::<Index>().map(|index| index.0));

        let mut recorded = self.0.lock().expect("unpoisoned");
        let span_fields = span.map(|index| recorded.spans[index].fields.clone()).unwrap_or_default();
        let metadata = event.metadata();
        recorded.events.push(SeenEvent {
            level: *metadata.level(),
            target: metadata.target().to_string(),
            fields: fields.0,
            span,
            span_fields,
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
