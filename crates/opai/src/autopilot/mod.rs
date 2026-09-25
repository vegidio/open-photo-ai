//! Autopilot: what the application suggests for a photograph nobody has touched yet.
//!
//! An **analysis** reads a set of signals off one picture and concludes which enhancements it calls for.
//!
//! ```text
//! Opai::suggest(&Picture, Option<&[Family]>, Option<ExecuteOptions>) -> Suggestions
//!   |
//!   +-- suggest::<B>()        this module's body, not a recursion:
//!         |                   area -> scope -> geometry -> pixels -> face, then Family::ALL order
//!         +-- Scope           the families asked for, and so which of the reads below run
//!         +-- geometry()      two integers; installs nothing
//!         +-- pixels()        two bounded passes in one blocking task; installs nothing
//!         |     +-- pass      a strided grid of at most ~1M samples, transparent ones skipped
//!         |     |     +-- light      three brightness percentiles -> one score per direction
//!         |     |     +-- color      the Shades of Gray estimate of the light -> one score
//!         |     |     +-- monochrome the chroma left once each brightness level's tint is removed -> one score
//!         |     +-- blocks    at most 256 full-resolution 64x64 blocks, any touching transparency skipped
//!         |           +-- noise      Immerkaer's estimate over the flattest blocks -> one score
//!         |           +-- sharpness  the Crete-Roffet blur effect at the sharpest blocks -> one score
//!         +-- face()          Opai::execute(Detection::newyork(Fp32)) -> Faces
//! ```

// An analysis is the first thing that happens to a picture a user just opened, and it happens unprompted. That shapes
// every decision here: a suggestion that is wrong, missing or unexplained is the application's first impression, and a
// failure raised over it is an error about something nobody asked for.
//
// Seven signals: the four the reference reads, and monochrome, noise and sharpness, which it does not. Its autopilot
// never suggests colorization, denoise or sharpen, and they are **invented here**, a deliberate divergence: parity with
// an absent signal would leave unnoticed the three things a user most plainly needs done to an old scan or a poor
// capture. Noise and sharpness read full-resolution blocks rather than the strided grid, because grain and blur exist
// only between neighbouring pixels, and their measured rates are in the design of the OpenSpec change
// `add-autopilot-noise-sharpness`. Monochrome rides the strided grid beside light and colour and finds grayscale images
// only: a toned or colour-scanned print carries more colour than the haziest colour photographs do, and the
// measurements are in the design of the OpenSpec change `add-autopilot-colorization`.
//
// The light and colour signals are **replaced, not ported**. The reference's single-pass scan misses most of what it
// exists to find: measured over real renderings, its light rule fires on correctly exposed low-key and high-key
// photographs and misses ordinary underexposure, and its colour rule counts a pixel with no hue as red and finds a
// stronger cast less often than a weaker one. Many photographs therefore get a different light or colour suggestion
// here than in the Go app. The detectors that replace them were fitted on public datasets, and their
// measured rates are in the design of the OpenSpec change `add-autopilot-light-color`. They also measure a 16-bit
// source at 16 bits where the reference truncates it to 8, and they ignore transparent pixels where the reference
// counts them as the black they premultiply to.
//
// Deliberate divergences from the reference in behaviour, each explained where it is decided: a failed signal is
// reported rather than logged and dropped (`Suggestions`), a cancelled analysis is an error rather than a partial
// answer (`suggest`), the scale ladder is in the library (`geometry`), and an image with no area is told it needs
// nothing (`suggest`).

mod blocks;
mod color;
#[cfg(test)]
mod fixtures;
mod light;
mod monochrome;
mod noise;
mod pass;
mod scope;
mod sharpness;

use image::DynamicImage;
use imaging::tensor::Sampler;
use rust_sak::memo::Memo;
use tokio_util::sync::CancellationToken;

use crate::error::InferenceError;
use crate::image::Picture;
use crate::inference::execute::{ExecuteOptions, Executed, execute};
use crate::models::{Analysis, Detection, Family, FloatPrecision, Scale};
use crate::pipeline::Backend;
use crate::task::spawn_blocking;
use crate::telemetry::unit::{self, Outcome, Unit, unit_span};
use tracing::Instrument as _;

use blocks::Blocks;
use color::Cast;
use light::Light;
use monochrome::Monochrome;
use noise::Noise;
use pass::Pass;
use scope::Scope;
use sharpness::Sharpness;

/// One enhancement an analysis concluded a photograph calls for.
///
/// **The family, and whatever the pixels themselves determine about it — nothing else.** A suggestion names no model
/// variant and no precision: which of a family's models to run is a standing user preference, and this library has
/// nowhere to read one from.
///
/// Every arm names a family the enhancement path can actually apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suggestion {
    // An arm that carried a variant or a precision would either invent a default the user never chose or force the
    // preference into `opai` to be stored a second time, out of step with the copy the front end already holds. Once
    // a settings store exists here, turning a suggestion into an `Operation` belongs beside this module rather than
    // in a front end, where two front ends would answer it twice.
    //
    // Only one arm carries a parameter, because a suggestion carries only what the pixels determine. The scale an
    // upscale runs at is read off the same pixel count that decided to suggest upscaling at all, so it *is* determined
    // by the image. Face recovery's parameters are which faces to restore and at what fidelity, and "there are faces
    // in this photograph" determines neither, so its arm is a unit. Colorization, light adjustment and colour balance
    // are units too: which colorizer runs and which variant fixes a cast are the user's choice, and which way an
    // exposure is wrong is something Lyon decides for itself.
    //
    // That every arm is runnable is held by a test rather than by discipline:
    // `every_suggestion_names_a_family_the_enhancement_path_can_apply`.
    /// The photograph shows visible grain, in its brightness or in its colour.
    Denoise,

    /// The photograph has at least one detectable face in it.
    FaceRecovery,

    /// The photograph carries no colour at all: it is a grayscale image.
    Colorization,

    /// The photograph is underexposed or overexposed as a whole.
    LightAdjustment,

    /// The photograph's colours are shifted as a whole towards one hue: the light it was taken under was not neutral.
    ColorBalance,

    /// The photograph is genuinely blurred: even its sharpest part is soft.
    Sharpen,

    /// The photograph is small enough to be worth enlarging, by the factor its size calls for.
    Upscale {
        /// How much to enlarge by, read off the pixel count alone.
        scale: Scale,
    },
}

impl Suggestion {
    /// Which family this suggestion is for.
    pub const fn family(&self) -> Family {
        // A match over the arms rather than a field, for the reason `Operation::family` gives: the family cannot then
        // disagree with the value it is carried beside, and a third arm cannot compile without answering.
        match self {
            Self::Denoise => Family::Denoise,
            Self::FaceRecovery => Family::FaceRecovery,
            Self::Colorization => Family::Colorization,
            Self::LightAdjustment => Family::LightAdjustment,
            Self::ColorBalance => Family::ColorBalance,
            Self::Sharpen => Family::Sharpen,
            Self::Upscale { .. } => Family::Upscale,
        }
    }
}

/// What an analysis concluded: what the photograph calls for, and whether anything could not be looked at.
///
/// A signal that could not be read and a signal that concluded "nothing needed" produce the same absence from
/// [`suggested`](Self::suggested); only [`incomplete`](Self::incomplete) tells them apart.
///
/// **Cancellation is not in here.** [`Opai::suggest`](crate::Opai::suggest) returns
/// `Result<Suggestions, InferenceError>`, and a cancelled or abandoned run is that error.
#[derive(Debug)]
pub struct Suggestions {
    // `incomplete` is a field rather than an error, the same reasoning that put `ProviderReport` on `Enhanced`: an
    // outcome that is invisible in the result is an outcome nobody can ask about. A portrait given no face-recovery
    // suggestion because the detection model could not be installed looks exactly like a portrait the analysis
    // considered and passed over, and the reference — which logs the failure and returns a bare slice — cannot tell
    // them apart from its own return value.
    //
    // Failing the whole call instead would be worse still: a failure raised over an unprompted analysis is an error
    // about something the user never asked for, while the signals that *were* read are still worth acting on. A caller
    // that ignores this field behaves exactly as the reference does; the field makes the better behaviour possible.
    //
    // Deliberately not `#[non_exhaustive]`, for the reason `Executed` is not: the attribute would bar an external crate
    // from destructuring this at all, including the `let Suggestions { suggested, .. } = ..` form that is how a caller
    // takes the half it wants.
    /// What the photograph calls for, in [`Family::ALL`] order — the one order of families this library names.
    ///
    /// Empty is a legitimate answer rather than a refusal: a photograph that needs no work is the conclusion, not
    /// the absence of one.
    pub suggested: Vec<Suggestion>,

    /// The signal that could not be read, and why — or `None` where every signal was read.
    ///
    /// Present alongside a populated [`suggested`](Self::suggested): the signals that succeeded still come back.
    pub incomplete: Option<InferenceError>,
}

// Exact: 1 MP and 4 MP are common photograph sizes, so a boundary off by one pixel changes what a great many images
// are told they need.
/// The largest image an analysis suggests enlarging **fourfold**: 1 MP, inclusive.
const UPSCALE_4X_MAX_PIXELS: u64 = 1_048_576;

// The reference's `maxPixelsForUpscale`, `4 << 20`, and `defaultUpscaleScale`'s second bound — the same number
// reached twice in two languages.
/// The largest image an analysis suggests enlarging **twofold**: 4 MP, inclusive. Above it, nothing.
const UPSCALE_2X_MAX_PIXELS: u64 = 4_194_304;

/// What the image's size alone says about enlarging it, or `None` where it is already big enough.
///
/// The cheapest signal there is: it reads two integers and installs nothing.
///
/// An image with no area is **not** this function's to refuse: the analysis returns before reaching it. See
/// [`suggest`].
fn geometry(width: u32, height: u32) -> Option<Suggestion> {
    // Widened before multiplying: two `u32` extents multiply past `u32` at 65,536 x 65,536 — well inside what a
    // stitched panorama reaches — and a pixel count that wrapped would tell a very large photograph it was very small.
    let pixels = u64::from(width) * u64::from(height);

    // One ladder, where the reference splits it across two languages: Go's `shouldUpscale` decides *whether* to
    // upscale, against one bound of 4 MP; TypeScript's `defaultUpscaleScale` decides *how much*, against 1 MP and then
    // 4 MP, and returns a scale of 1 above the second — the same "no" the Go half already said, spelled as a factor
    // that changes nothing. It is one decision read off one number, and a second front end that had to restate the
    // TypeScript half is exactly how two front ends come to disagree about what the same photograph needs.
    let scale = if pixels <= UPSCALE_4X_MAX_PIXELS {
        4.0
    } else if pixels <= UPSCALE_2X_MAX_PIXELS {
        2.0
    } else {
        return None;
    };

    // `clamped` rather than `new`: both factors are literals well inside `Scale`'s range, so the constructor that
    // returns a `Result` would hand back an error this function has nothing to do with and no caller could act on.
    Some(Suggestion::Upscale { scale: Scale::clamped(scale) })
}

/// What the two pixel passes measured, and so what the light, colour, monochrome, noise and sharpness signals conclude.
///
/// None of the signals can fail: they are statistics of pixels already in memory. A photograph with nothing left to
/// measure, because every pixel is transparent or it is too small to hold a block, concludes nothing for the signals
/// that could not measure it.
struct PixelSignals {
    /// How many samples the strided pass read, or `None` where it was not read.
    samples: Option<u64>,
    /// The light signal's measurements, or `None` where there were no samples.
    light: Option<Light>,
    /// The colour signal's measurements, or `None` where no sample carried colour evidence.
    cast: Option<Cast>,
    /// The monochrome signal's measurements, or `None` where too few samples had no channel clipped.
    monochrome: Option<Monochrome>,
    /// How many blocks the block pass read, or `None` where it was not read.
    blocks: Option<usize>,
    /// The noise signal's measurements, or `None` where too few blocks could be measured.
    noise: Option<Noise>,
    /// The sharpness signal's measurements, or `None` where too few blocks had detail.
    sharpness: Option<Sharpness>,
}

impl PixelSignals {
    /// Reads the passes over `source` that `scope` needs: the strided one, the block one, or both.
    ///
    /// The signals a pass that was not read feeds are `None`, which concludes nothing.
    fn read(source: &DynamicImage, scope: Scope) -> Self {
        // One sampler for both passes: a source it cannot borrow is widened to 16 bits in full, and once is enough.
        let sampler = Sampler::new(source);
        let (width, height) = (source.width(), source.height());

        Self::of(
            scope.needs_pass().then(|| pass::read_from(&sampler, width, height)).as_ref(),
            scope.needs_blocks().then(|| blocks::read_from(&sampler, width, height)).as_ref(),
            u64::from(width) * u64::from(height),
        )
    }

    fn of(pass: Option<&Pass>, blocks: Option<&Blocks>, pixels: u64) -> Self {
        let noise = blocks.and_then(Noise::measure);
        // A photograph whose noise could not be measured has no grain to gate out.
        let sharpness =
            blocks.and_then(|blocks| Sharpness::measure(blocks, noise.map_or(0.0, |noise| noise.luma), pixels));

        Self {
            samples: pass.map(|pass| pass.samples),
            light: pass.and_then(Light::measure),
            cast: pass.and_then(Cast::measure),
            monochrome: pass.and_then(Monochrome::measure),
            blocks: blocks.map(|blocks| blocks.blocks.len()),
            noise,
            sharpness,
        }
    }

    /// What the five signals concluded.
    fn suggestions(&self) -> impl Iterator<Item = Suggestion> {
        let monochrome = self.monochrome.is_some_and(|monochrome| monochrome.calls_for_colorization());
        let light = self.light.is_some_and(|light| light.calls_for_adjustment());
        let colour = self.cast.is_some_and(|cast| cast.calls_for_balance());
        let noise = self.noise.is_some_and(|noise| noise.calls_for_denoise());
        let sharpness = self.sharpness.is_some_and(|sharpness| sharpness.calls_for_sharpen());

        [
            noise.then_some(Suggestion::Denoise),
            monochrome.then_some(Suggestion::Colorization),
            light.then_some(Suggestion::LightAdjustment),
            colour.then_some(Suggestion::ColorBalance),
            sharpness.then_some(Suggestion::Sharpen),
        ]
        .into_iter()
        .flatten()
    }
}

/// Whether the photograph's exposure or colour is wrong, whether it has colour of its own, whether it is grainy and
/// whether it is blurred, read in the bounded passes over its pixels that `scope` needs.
///
/// No model, no installation and no network: nothing here can fail except by being withdrawn.
///
/// # Errors
///
/// [`InferenceError::Cancelled`] when `cancel` is cancelled before or during the pass, and
/// [`InferenceError::Shutdown`] when the runtime went down before it could run.
async fn pixels(source: &Picture, scope: Scope, cancel: &CancellationToken) -> Result<PixelSignals, InferenceError> {
    pixels_with(source, cancel, move |pixels| PixelSignals::read(pixels, scope)).await
}

/// [`pixels`], with the passes themselves a parameter so a test can cancel from inside them.
async fn pixels_with<F>(source: &Picture, cancel: &CancellationToken, read: F) -> Result<PixelSignals, InferenceError>
where
    F: FnOnce(&DynamicImage) -> PixelSignals + Send + 'static,
{
    // Checked on both sides of the passes rather than inside them. Each is a million samples at most, a few tens of
    // milliseconds, so a check per row would cost more than the latency it saves; the check after them is what makes a
    // cancellation that arrived during either pass fail the analysis rather than be answered.
    if cancel.is_cancelled() {
        return Err(InferenceError::Cancelled);
    }

    // Off the async runtime, which in the GUI is shared with the window: two passes of a million samples are tens of
    // milliseconds, long enough to stall an event loop. One blocking task for both, so the photograph is handed over
    // once.
    let pixels = source.shared_pixels();
    let signals = spawn_blocking::<_, InferenceError, _>(move || read(&pixels)).await?;

    if cancel.is_cancelled() {
        return Err(InferenceError::Cancelled);
    }

    Ok(signals)
}

/// A signal that could not be read, and what that costs the analysis.
enum Unread {
    // The two dispositions are decided in **one** match, in `face`, so they cannot drift apart: a later signal that
    // classified a cancellation as something to report beside its suggestions would silently hand a caller an answer
    // about a photograph the user has already moved on from.
    /// The question was withdrawn — the caller cancelled, or the runtime is going down. The analysis fails and
    /// returns nothing at all, including the suggestions it had already determined.
    Withdrawn(InferenceError),

    /// The signal could not be read, but the analysis stands: the signals that succeeded still come back, with this
    /// reported beside them as [`Suggestions::incomplete`].
    Reported(InferenceError),
}

/// Whether the photograph has anyone in it, by detecting the faces in it.
///
/// `options` is the caller's, passed through unchanged: the provider to detect on, progress for the one run this
/// makes, the cancellation the classification below rests on, and the cache flag a benchmark needs to defeat.
///
/// # Errors
///
/// [`Unread`], which is the classification itself — see its arms.
async fn face<B: Backend>(
    backend: &B,
    cache: Option<&Memo>,
    source: &Picture,
    options: ExecuteOptions,
) -> Result<Option<Suggestion>, Unread> {
    // Through `execute`, not around it. The reference reaches past its own public API into `facerecovery.GetDtModel`
    // and manages the lease by hand; going through the path a caller uses to detect faces directly buys three things
    // it cannot have. The store serves a repeated analysis of the same photograph without building a session. The
    // analysis *warms* the store for the face-recovery operation the suggestion turns into, so accepting a suggestion
    // is free rather than a second detection. And `execute` acquires its handle, holds it for the length of the call
    // and drops it before returning, so `Opai::release_sessions` has nothing outstanding from an analysis and an
    // analysis cannot outlive the session it ran on.
    //
    // FP32, as the reference chooses: there is no caller's precision here, because autopilot runs before any
    // face-recovery operation exists to take one from. It is the precision the operation this suggestion turns into
    // will be built at, since a suggested enhancement is added at its model's HD tier; detecting at anything else would
    // download a second detection graph to answer a question the first one already answered.
    let detection: Analysis = Detection::newyork(FloatPrecision::Fp32);

    // An empty set is a legitimate finding rather than a signal that could not be read — which is exactly the
    // distinction `execute` keeps by refusing to stand an empty `Faces` in for an error.
    match execute::<B, Analysis>(backend, cache, source, &detection, Some(options)).await {
        Ok(Executed { value: faces, .. }) => Ok((!faces.is_empty()).then_some(Suggestion::FaceRecovery)),
        Err(error @ (InferenceError::Cancelled | InferenceError::Shutdown)) => Err(Unread::Withdrawn(error)),
        Err(error) => Err(Unread::Reported(error)),
    }
}

/// Analyses `source` and hands back what it calls for: the body of [`Opai::suggest`](crate::Opai::suggest).
///
/// An image with no area is told it needs nothing, and nothing is installed for it. The suggestions are emitted in
/// [`Family::ALL`] order, the one order of families this library names.
///
/// `families` narrows the analysis: see [`Scope`]. A read no family in it needs is not made, a family outside it is
/// never suggested, and a signal that was not read never makes the analysis incomplete.
///
/// # Errors
///
/// [`InferenceError::Cancelled`] or [`InferenceError::Shutdown`], and then **no suggestions at all**, including the
/// ones already determined. Every other failure is a signal that could not be read and comes back in the result. See
/// [`Unread`].
pub(crate) async fn suggest<B: Backend>(
    backend: &B,
    cache: Option<&Memo>,
    source: &Picture,
    families: Option<&[Family]>,
    options: Option<ExecuteOptions>,
) -> Result<Suggestions, InferenceError> {
    // Wrapped, as `Opai::initialize` and `deps::install` are, so every outcome is recorded once, here, with how long
    // the analysis took — including the early return for an image with no area, and a withdrawal, which on its own
    // would leave nothing behind.
    let scope = Scope::of(families);
    let span = unit_span!("automatic_analysis", identity = source.identity(), asked = %asked(scope));

    let started = std::time::Instant::now();
    let outcome = suggest_inner(backend, cache, source, scope, options).instrument(span.clone()).await;
    let duration = started.elapsed();

    span.in_scope(|| match &outcome {
        Ok(concluded) => {
            record(source, scope, concluded, duration);
            unit::ended(Unit::AutoAnalysis, &span, duration, Outcome::Finished);
        }
        Err(error) => match error.stop_reason() {
            Some(reason) => {
                tracing::info!(identity = source.identity(), reason, ?duration, "automatic analysis stopped");
                unit::ended(Unit::AutoAnalysis, &span, duration, Outcome::Stopped);
            }
            // No path produces one today: every failure below is a signal that could not be read, and comes back in
            // the result. Recorded anyway, so that the day one does it is not silent.
            None => {
                tracing::warn!(identity = source.identity(), ?duration, %error, "automatic analysis failed");
                unit::ended(Unit::AutoAnalysis, &span, duration, Outcome::Failed { kind: error.kind(), error });
            }
        },
    });

    outcome
}

/// [`suggest`]'s body, which records only what it measured: the outcome is its caller's to record.
async fn suggest_inner<B: Backend>(
    backend: &B,
    cache: Option<&Memo>,
    source: &Picture,
    scope: Scope,
    options: Option<ExecuteOptions>,
) -> Result<Suggestions, InferenceError> {
    // Generic in the backend for the reason `execute` is: everything decided here — the order of evaluation, the
    // classification of a failed signal, the ordering of the result — is then checked against a fake session on a
    // runner with no runtime.
    let options = options.unwrap_or_default();
    let (width, height) = source.dimensions();

    // First, and before any signal that would run a model. There is no enhancement of nothing: the enhancement path
    // already refuses such an image and the detection path already refuses to run over one, so suggesting work here
    // would be work refused the moment it was accepted — and that refusal, reported as `Suggestions::incomplete`,
    // would describe a degenerate input as a fault.
    //
    // The reference suggests upscaling for an image with no area, because its size test is a comparison against an
    // upper bound that zero passes. Its two scan-based heuristics each guard against an empty image explicitly, so
    // that outcome reads as incidental rather than intended, and it is not reproduced.
    //
    // The same shape, for a caller that asked for nothing any signal can suggest: an empty set, or detection alone.
    // Returning here, rather than trusting each read to check its own flag, is what keeps "no signal is read" true.
    if width == 0 || height == 0 || !scope.needs_anything() {
        return Ok(Suggestions { suggested: Vec::new(), incomplete: None });
    }

    // Checked here because a scope of upscale alone reaches neither the pixels nor the face signal, the two places a
    // cancellation is otherwise observed. For an analysis that is not narrowed the pixels' own first check is the same
    // check, so nothing changes for it.
    if options.cancel.is_cancelled() {
        return Err(InferenceError::Cancelled);
    }

    let mut suggested = Vec::new();
    let mut incomplete = None;

    // Cheapest first: two integers and no installation. Then the pixels, which cost two bounded passes and install
    // nothing, and only then the one signal that may have to download a model. Each runs only where the scope holds a
    // family it serves.
    if scope.needs_geometry() {
        suggested.extend(geometry(width, height));
    }

    if scope.needs_pass() || scope.needs_blocks() {
        let signals = pixels(source, scope, &options.cancel).await?;
        record_measurements(source, &signals);
        suggested.extend(signals.suggestions());
    }

    if scope.needs_face() {
        match face(backend, cache, source, options).await {
            Ok(read) => suggested.extend(read),
            // The question was withdrawn, so nothing comes back — not the geometry suggestion already determined
            // above. The reference's `shouldFaceRecovery` turns a cancelled context into `false` and returns that
            // suggestion.
            Err(Unread::Withdrawn(error)) => return Err(error),
            // Not recorded here: the detection's own "analysis failed" already holds the failure, and the conclusion
            // names the signal and its reason.
            Err(Unread::Reported(error)) => incomplete = Some(error),
        }
    }

    // In one place rather than read by read: a pass read for one family measures signals for others (light, when only
    // colorization was asked for), and a signal added to a pass later is kept out of a narrowed answer here without
    // anyone having to teach it the scope.
    suggested.retain(|suggestion| scope.includes(suggestion.family()));

    // The ordering is a property of the **family**, not of the sequence the signals happen to be read in. Sorting
    // against `Family::ALL` — the one order of families this library names — is what places a signal added later
    // without anyone having to remember where its push belongs, and the evaluation order above is a cost decision,
    // which is a different question entirely. That this order agrees with the reference's is pinned by
    // `the_order_suggestions_come_back_in_agrees_with_the_reference`.
    //
    // `position` cannot miss: `Family::ALL` is every family the library names, and `Suggestion::family` returns one
    // of them. The fallback is written rather than an `expect` so that a sort cannot panic on a photograph.
    suggested.sort_by_key(|suggestion| {
        Family::ALL
            .iter()
            .position(|family| *family == suggestion.family())
            .unwrap_or(Family::ALL.len())
    });

    Ok(Suggestions { suggested, incomplete })
}

/// The name the face signal is recorded under, so a reader can tell which signal could not be read.
const FACE_SIGNAL: &str = "face";

/// The families an analysis was asked about, as one field value: `all` for an analysis nobody narrowed.
fn asked(scope: Scope) -> String {
    if scope.narrowed() {
        scope.families().map(Family::label).collect::<Vec<_>>().join(", ")
    } else {
        String::from("all")
    }
}

/// Records what an analysis concluded, at `info`, naming the families it was asked to check, the families suggested
/// and how long it took — and, where the face signal could not be read, that signal and its reason.
///
/// **Written for every outcome, including an empty one.** An analysis that concluded a photograph needs nothing and
/// an analysis that never ran must not look alike in the file.
///
/// The fields are [`execute`]'s own, so one log reads the same for both paths.
fn record(source: &Picture, scope: Scope, concluded: &Suggestions, duration: std::time::Duration) {
    // At `info`, the default verbosity: an analysis installs models, opens sessions and runs inference without the
    // user having asked for anything, so an account of it belongs in the log a user attaches without being asked to
    // reproduce anything.
    //
    // The source's `identity` rather than its path, as `execute` records it: the path is a place the user chose, and
    // the identity is what ties this record to a second run over the same photograph an hour later.
    //
    // The families rather than the suggestions, which is what the spec asks for and what a reader can act on: the
    // scale an upscale carries is in the result the caller already has.
    //
    // Joined into one string rather than printed as a `Vec` through `?`, which would render the labels with escaped
    // quotes inside an already-quoted field value — unreadable in the file, and unparseable by anything reading the
    // record back.
    let families = concluded
        .suggested
        .iter()
        .map(|suggestion| suggestion.family().label())
        .collect::<Vec<_>>()
        .join(", ");

    // `asked` beside `families`, because a family missing from the second is otherwise ambiguous in a user's log: it
    // was checked and needed nothing, or it was never checked.
    let asked = asked(scope);

    // The face signal is the only one that can go unread today, so an incompleteness names it. Still `info`: the
    // detection that failed recorded the failure at `warn`, and this is the analysis's account of what it concluded
    // without it.
    tracing::info!(
        identity = source.identity(),
        count = concluded.suggested.len(),
        %asked,
        %families,
        ?duration,
        incomplete = concluded.incomplete.as_ref().map(|_| FACE_SIGNAL),
        error = concluded.incomplete.as_ref().map(tracing::field::display),
        "analysis suggested enhancements"
    );
}

/// Records what the pixel passes measured at `debug`, so that a suggestion reported as wrong can be explained from the
/// log without the photograph.
///
/// The brightness percentiles are on `[0, 1]` of encoded brightness. `angle` is the Shades of Gray estimate's distance
/// from neutral in degrees, and `red` and `blue` its chromaticity. `unclipped` is how many samples had no channel
/// clipped, and `residual` their chroma left once each brightness level's tint is removed, on `[0, 1]`. `luma_noise`
/// and `chroma_noise` are the flattest blocks' noise on the 0 to 255 scale, and `noise_luma` their median brightness.
/// `detailed` is how many blocks had detail, and `sharpest` their sharpness at the 90th percentile. Each score suggests
/// above zero. A signal with nothing to measure records nothing of its own.
fn record_measurements(source: &Picture, signals: &PixelSignals) {
    // `debug` rather than `info`: the conclusion is already recorded at `info`, and these are for whoever investigates
    // it.
    let light = signals.light;
    let cast = signals.cast;
    let monochrome = signals.monochrome;
    let noise = signals.noise;
    let sharpness = signals.sharpness;

    tracing::debug!(
        identity = source.identity(),
        samples = signals.samples,
        low = light.map(|light| light.low),
        median = light.map(|light| light.median),
        high = light.map(|light| light.high),
        under = light.map(|light| light.under()),
        over = light.map(|light| light.over()),
        angle = cast.map(|cast| cast.angle()),
        red = cast.map(|cast| cast.chromaticity()[0]),
        blue = cast.map(|cast| cast.chromaticity()[1]),
        cast = cast.map(|cast| cast.score()),
        unclipped = monochrome.map(|monochrome| monochrome.samples),
        residual = monochrome.map(|monochrome| monochrome.residual),
        monochrome = monochrome.map(|monochrome| monochrome.score()),
        blocks = signals.blocks,
        luma_noise = noise.map(|noise| noise.luma),
        chroma_noise = noise.map(|noise| noise.chroma),
        noise_luma = noise.map(|noise| noise.median),
        noise = noise.map(|noise| noise.score()),
        detailed = sharpness.map(|sharpness| sharpness.detailed),
        sharpest = sharpness.map(|sharpness| sharpness.sharpest),
        sharpness = sharpness.map(|sharpness| sharpness.score()),
        "the pixel signals measured"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::test_support::stub_backend_runs;

    use super::fixtures;

    use std::sync::{Arc, Mutex};

    use image::DynamicImage;
    use rust_sak::memo::{CacheOpts, Memo};

    use crate::cache::RunCache;
    use crate::error::SessionError;
    use crate::inference::process::identity_of_data;
    use crate::logging::{self, field, records};
    use crate::models::Subject;
    use crate::models::detection::newyork;
    use crate::models::face::{Confidence, Face, Point, Rect};
    use crate::models::{ArtifactId, Bias, Faces, Fidelity, FloatPrecision, Operation, UpscaleVariant};
    use crate::models::{
        ColorBalance, Colorization, Denoise, FaceRecovery, LightAdjustment, Sharpen, Strength, Upscale,
    };
    use crate::pipeline::session::{GraphShape, NamedOutput};
    use crate::pipeline::test_support::NoBackend;
    use crate::providers::ExecutionProvider;
    use crate::providers::profile::EpProfile;
    use crate::sessions::SessionHandle;
    use crate::task::lock;
    use tokio_util::sync::CancellationToken;

    /// A picture of `width` by `height` to analyse, correctly exposed, with no cast and with colour of its own, so that
    /// the pixel signals conclude nothing and what a test asserts is decided by its extents and its faces. It has an
    /// identity the store's slot is folded from.
    ///
    /// A ramp from black at the left to white at the right, in four bands down the frame of neutral, red, green and
    /// blue of equal strength: every tone is there in every band, the light is neutral, and equally bright parts differ
    /// in colour.
    fn picture(width: u32, height: u32) -> Picture {
        const TINTS: [[u32; 3]; 4] = [[100, 100, 100], [100, 80, 80], [80, 100, 80], [80, 80, 100]];

        let span = width.saturating_sub(1).max(1);
        let ramp = image::RgbImage::from_fn(width, height, |x, y| {
            let tint = TINTS[(y * 4 / height) as usize];
            image::Rgb(tint.map(|t| (x * 255 / span * t / 100) as u8))
        });

        picture_of(DynamicImage::ImageRgb8(ramp))
    }

    /// `pixels` as a picture to analyse, under the identity [`picture`]'s have.
    fn picture_of(pixels: DynamicImage) -> Picture {
        Picture::new("/pictures/portrait.jpg", Arc::new(pixels), "cafebabecafebabe")
    }

    /// One face, enough for the signal to conclude there is somebody in the photograph.
    fn a_face() -> Faces {
        Faces::new([Face::new(
            Rect::new(Point::new(10.0, 20.0), Point::new(110.0, 140.0)),
            [Point::new(0.0, 0.0); Face::LANDMARKS],
            Confidence::new(0.97).expect("a confidence in range"),
        )])
    }

    /// A store already holding `faces` as the answer to the detection this analysis will ask of `source`, which drives
    /// the face signal to a **result** with no ONNX Runtime anywhere.
    ///
    /// Because the lookup sits above the acquisition, nothing is installed and no session is opened — which is what
    /// lets [`NoBackend`], whose every method is `unreachable!`, be the backend for these tests. That the backend is
    /// never reached is the assertion, not a limitation.
    fn store_holding(source: &Picture, faces: &Faces) -> Memo {
        let memo = Memo::memory(CacheOpts::new()).expect("a memory store");
        let detection: Analysis = Detection::newyork(FloatPrecision::Fp32);
        // Derived by the same function the read and the write both call, so a hit here is a hit for the reason a
        // repeated analysis in production is one rather than because the suite arranged a special case.
        let key = identity_of_data(source.identity(), &Subject::Analysis(detection));

        RunCache::new(memo.clone(), source.identity()).put_value(&key, faces);

        memo
    }

    /// What a run reached for, so a test can say what was acquired rather than infer it from the result.
    #[derive(Default)]
    struct Log {
        /// Every artifact a session was asked for, in order.
        acquired: Vec<String>,
    }

    /// A backend that records what was asked of it, and answers the detection graph when a run reaches one.
    ///
    /// [`NoBackend`] covers every case where the acquisition must not happen at all, which it proves by panicking.
    /// This one covers what cannot be expressed that way: a model that will not open, a token cancelled while the
    /// signal is in flight, and — for the two tests that need a detection to actually *happen* rather than to be
    /// served — a graph that answers.
    struct Fake {
        log: Arc<Mutex<Log>>,
        /// An artifact whose session refuses to open, which is the face signal's "could not look".
        fails_to_open: Option<&'static str>,
        /// A token cancelled while a session is being acquired, which is how a run is stopped *in flight* rather
        /// than before it starts.
        cancels: Option<CancellationToken>,
        /// Which anchor the graph reports a confident face at, or `None` for a photograph with nobody in it.
        face_at: Option<usize>,
    }

    impl Fake {
        fn new() -> Self {
            Self { log: Arc::default(), fails_to_open: None, cancels: None, face_at: None }
        }

        /// One that answers with a face, for the runs that must reach the model rather than the store.
        fn finding_a_face() -> Self {
            Self { face_at: Some(0), ..Self::new() }
        }

        fn acquired(&self) -> Vec<String> {
            lock(&self.log).acquired.clone()
        }
    }

    impl Backend for Fake {
        /// The anchor this session's graph reports a face at, carried on the handle because that is all the run
        /// needs from it.
        type Session = Option<usize>;
        type Error = std::io::Error;

        async fn acquire(
            &self,
            artifact: &ArtifactId,
            _profile: &EpProfile,
            _requested: ExecutionProvider,
            _interest: &crate::sessions::Interest,
        ) -> Result<SessionHandle<Self::Session>, SessionError> {
            lock(&self.log).acquired.push(artifact.as_str().to_string());

            // Stopped while the model is being opened, which is the only way this suite reaches a cancellation the
            // pipeline itself observes: a token cancelled before the call is caught by the driver's own first check.
            if let Some(token) = &self.cancels {
                token.cancel();
            }

            if self.fails_to_open == Some(artifact.as_str()) {
                return Err(SessionError::Build {
                    artifact: artifact.as_str().to_string(),
                    provider: ExecutionProvider::Cpu,
                    source: Arc::new(std::io::Error::other("the graph would not load")),
                });
            }

            Ok(SessionHandle::held(self.face_at, ExecutionProvider::Cpu))
        }

        // Only the named-output seam below is answered: the detection graph is the one thing this suite runs.
        stub_backend_runs!("this suite drives the analysis, not a model"; run_tile, run_graph);

        /// The one seam a detection run takes, answered through the model's own fixture.
        fn run_named_outputs(
            handle: &SessionHandle<Self::Session>,
            _input: &[f32],
            _input_shape: GraphShape,
            outputs: &mut [NamedOutput<'_>],
        ) -> Result<(), Self::Error> {
            // The fixture that lives with the model whose tensor names and confidence layout it encodes, rather than a
            // second copy written here: two copies is two things that can quietly stop agreeing about the shape of
            // that graph's output.
            newyork::answer_graph(outputs, *handle.session());

            Ok(())
        }

        stub_backend_runs!("this suite drives the analysis, not a model"; run_weighted);
    }

    /// The face signal alone, over a store that already holds the answer, against the backend that proves nothing was
    /// acquired to produce it.
    async fn face_signal_over(source: &Picture, faces: &Faces) -> Result<Option<Suggestion>, Unread> {
        let memo = store_holding(source, faces);

        face::<NoBackend>(&NoBackend, Some(&memo), source, ExecuteOptions::default()).await
    }

    /// The spec's **"A portrait"**.
    #[tokio::test]
    async fn a_photograph_with_a_face_in_it_calls_for_face_recovery() {
        let source = picture(64, 48);

        let read = face_signal_over(&source, &a_face()).await;

        assert!(matches!(read, Ok(Some(Suggestion::FaceRecovery))));
    }

    /// The spec's **"A landscape with nobody in it"** — and that the absence is a conclusion rather than a failure,
    /// which is the distinction `execute` keeps by refusing to stand an empty set in for an error.
    #[tokio::test]
    async fn a_photograph_with_nobody_in_it_does_not_call_for_face_recovery() {
        let source = picture(64, 48);

        let read = face_signal_over(&source, &Faces::empty()).await;

        assert!(
            matches!(read, Ok(None)),
            "an empty finding is a conclusion, not a signal that could not be read"
        );
    }

    /// The spec's **"The detection model cannot be obtained"**, at the signal: the failure is classified as one to
    /// report beside the other signals rather than one that fails the call.
    #[tokio::test]
    async fn a_face_signal_whose_model_will_not_open_is_reported_rather_than_swallowed() {
        let backend = Fake { fails_to_open: Some("dt_newyork_fp32"), ..Fake::new() };
        let source = picture(64, 48);

        let read = face::<Fake>(&backend, None, &source, ExecuteOptions::default()).await;

        let Err(Unread::Reported(error)) = read else {
            panic!("a model that will not open is a signal that could not be read, not a withdrawn question");
        };

        assert!(
            matches!(error, InferenceError::Open { .. }),
            "the reason is the one the session layer gave: {error}"
        );
        assert_eq!(
            backend.acquired(),
            ["dt_newyork_fp32"],
            "the signal detects at the precision a suggestion is taken at"
        );
    }

    /// A whole analysis of `source`, over a store that already holds the detection's answer — so the backend that
    /// proves nothing was acquired can be the one it runs against.
    async fn analyse(source: &Picture, faces: &Faces) -> Result<Suggestions, InferenceError> {
        let memo = store_holding(source, faces);

        suggest::<NoBackend>(&NoBackend, Some(&memo), source, None, None).await
    }

    /// The spec's **"An image with no pixels"**: nothing is suggested, nothing is reported incomplete, and — the
    /// half that matters — **no detection is run over it**.
    ///
    /// Asserted against the fake's acquisition log rather than against the result, which is the whole point: an
    /// analysis that ran the detection and then discarded its answer would produce exactly the same `Suggestions`
    /// and would have installed a model and opened a session to do it.
    #[tokio::test]
    async fn an_image_with_no_area_is_told_it_needs_nothing_and_nothing_is_acquired_for_it() {
        let backend = Fake::new();
        let source = picture(0, 0);

        let concluded = suggest::<Fake>(&backend, None, &source, None, None)
            .await
            .expect("a degenerate image is not a failure");

        assert!(concluded.suggested.is_empty(), "there is no enhancement of nothing");
        assert!(concluded.incomplete.is_none(), "a degenerate input is not a signal that could not be read");
        assert!(backend.acquired().is_empty(), "the analysis reached a model for an image with no pixels");
    }

    /// One extent of zero is the same degenerate image as both being zero, and the reference's own size test — a
    /// comparison against an upper bound — passes every one of them.
    #[tokio::test]
    async fn an_image_with_one_extent_of_zero_is_told_it_needs_nothing() {
        let backend = Fake::new();

        for (width, height) in [(0, 48), (64, 0)] {
            let concluded = suggest::<Fake>(&backend, None, &picture(width, height), None, None)
                .await
                .expect("a degenerate image is not a failure");

            assert!(concluded.suggested.is_empty(), "{width}x{height} was told it needs something");
        }

        assert!(backend.acquired().is_empty(), "the analysis reached a model for an image with no pixels");
    }

    /// The spec's **"The caller cancels mid-analysis"**: the call fails, and **no** suggestions come back — not even
    /// the geometry one the analysis had already determined before the face signal started.
    ///
    /// The picture is small, so the geometry signal has certainly produced a suggestion by the time the token is
    /// cancelled inside the acquisition. An analysis that returned what it had would return that one.
    #[tokio::test]
    async fn a_cancelled_analysis_returns_nothing_including_what_it_had_already_determined() {
        let cancel = CancellationToken::new();
        let backend = Fake { cancels: Some(cancel.clone()), ..Fake::new() };
        let source = picture(64, 48);

        assert!(geometry(64, 48).is_some(), "the fixture must have a geometry suggestion to be able to leak one");

        let options = ExecuteOptions { cancel, ..Default::default() };
        let outcome = suggest::<Fake>(&backend, None, &source, None, Some(options)).await;

        assert!(matches!(outcome, Err(InferenceError::Cancelled)), "a withdrawn question gets no answer at all");
    }

    /// The spec's **"The same image analysed again"**: the second analysis is answered from the first one's
    /// detection rather than by running the model a second time.
    ///
    /// Driven through an *empty* store that the first analysis fills, rather than one the suite pre-loaded. That is
    /// the difference that matters here: a pre-loaded store shows that a hit is served, but only a run that wrote
    /// what it computed shows that the two ends of the analysis agree about the slot. The assertion is the
    /// acquisition log, because "was the model run again" is a question about what the run reached for, and a
    /// second detection that happened to produce the same faces would give an identical `Suggestions`.
    #[tokio::test]
    async fn a_second_analysis_of_the_same_photograph_is_answered_from_the_first_ones_detection() {
        let memo = Memo::memory(CacheOpts::new()).expect("a memory store");
        let backend = Fake::finding_a_face();
        let source = picture(640, 640);

        let first = suggest::<Fake>(&backend, Some(&memo), &source, None, None).await.expect("a first analysis");

        assert!(
            first.suggested.contains(&Suggestion::FaceRecovery),
            "the graph's face did not reach a suggestion"
        );
        assert_eq!(backend.acquired(), ["dt_newyork_fp32"], "the first analysis did not run the detection");

        let second = suggest::<Fake>(&backend, Some(&memo), &source, None, None).await.expect("a second analysis");

        assert_eq!(second.suggested, first.suggested, "the same photograph was told two different things");
        assert_eq!(
            backend.acquired(),
            ["dt_newyork_fp32"],
            "the second analysis ran the detection again instead of being served the first one's"
        );
    }

    /// The spec's **"The suggestion is accepted"**: the faces the accepted suggestion needs are the ones the
    /// analysis already found, rather than a second detection at a second precision.
    ///
    /// The second half of the call is what a front end does the moment the user accepts — ask for the faces the
    /// face-recovery operation takes — and it is `execute`, the public path, not a private seam of this module.
    /// That it acquires nothing is the whole claim the FP32 choice rests on: detecting at any other precision here
    /// would leave this second call to install and open a graph of its own.
    #[tokio::test]
    async fn accepting_a_face_recovery_suggestion_reuses_the_faces_the_analysis_found() {
        let memo = Memo::memory(CacheOpts::new()).expect("a memory store");
        let backend = Fake::finding_a_face();
        let source = picture(640, 640);

        let concluded = suggest::<Fake>(&backend, Some(&memo), &source, None, None).await.expect("an analysis");
        assert!(concluded.suggested.contains(&Suggestion::FaceRecovery));
        assert_eq!(backend.acquired(), ["dt_newyork_fp32"]);

        // Exactly what the analysis asked, asked again by the caller accepting its answer.
        let detection: Analysis = Detection::newyork(FloatPrecision::Fp32);
        let Executed { value: faces, .. } = execute::<Fake, Analysis>(&backend, Some(&memo), &source, &detection, None)
            .await
            .expect("the faces the analysis already found");

        assert_eq!(faces.len(), 1, "accepting the suggestion did not get the face the analysis found");
        assert_eq!(
            backend.acquired(),
            ["dt_newyork_fp32"],
            "accepting the suggestion ran a second detection instead of taking the analysis's"
        );
    }

    /// The spec's **"The detection model cannot be obtained"**, end to end: the other signals still come back, the
    /// result says the analysis was incomplete and why, and the call is **not** a failure.
    #[tokio::test]
    async fn a_signal_that_could_not_be_read_comes_back_beside_the_ones_that_could() {
        let backend = Fake { fails_to_open: Some("dt_newyork_fp32"), ..Fake::new() };
        let source = picture(64, 48);

        let concluded = suggest::<Fake>(&backend, None, &source, None, None)
            .await
            .expect("a signal that failed is not a failed call");

        assert_eq!(
            concluded.suggested,
            vec![Suggestion::Upscale { scale: Scale::clamped(4.0) }],
            "the geometry signal was read and still counts"
        );
        assert!(
            !concluded.suggested.contains(&Suggestion::FaceRecovery),
            "a signal that could not be read must not be reported as a conclusion"
        );

        let incomplete = concluded.incomplete.expect("a signal that could not be read is reported");
        assert!(matches!(incomplete, InferenceError::Open { .. }), "the reason is the one given: {incomplete}");
    }

    /// The other side of the same requirement: every signal read means nothing is reported incomplete, whether or
    /// not anything was suggested.
    #[tokio::test]
    async fn an_analysis_that_read_every_signal_reports_no_incompleteness() {
        // Large enough that the geometry signal suggests nothing, and no faces — so the answer is empty and the
        // analysis is nonetheless complete.
        let source = picture(4096, 4096);

        let concluded = analyse(&source, &Faces::empty()).await.expect("an analysis over a read store");

        assert!(concluded.suggested.is_empty(), "a photograph that needs no work is the answer");
        assert!(concluded.incomplete.is_none(), "nothing failed, so nothing is reported incomplete");
    }

    /// The spec's **"Several suggestions at once"**, pinned as an *ordering rule* rather than as a transcribed pair.
    ///
    /// The expectation is computed by sorting the families that came back against their position in `Family::ALL`
    /// and comparing that with the order they actually arrived in. Written this way because the ordering is a
    /// property of the family: a signal added later is placed by the one list rather than by where its push happened
    /// to be written, and a test asserting the literal pair `[FaceRecovery, Upscale]` would keep passing while that
    /// property quietly stopped holding.
    #[tokio::test]
    async fn suggestions_come_back_in_family_order() {
        // Both signals conclude: small enough to enlarge, and with somebody in it.
        let source = picture(64, 48);

        let concluded = analyse(&source, &a_face()).await.expect("an analysis over a read store");

        let produced: Vec<_> = concluded.suggested.iter().map(|suggestion| suggestion.family()).collect();
        assert_eq!(produced.len(), 2, "both signals should have concluded: {:?}", concluded.suggested);

        let mut expected = produced.clone();
        expected.sort_by_key(|family| {
            Family::ALL
                .iter()
                .position(|candidate| candidate == family)
                .expect("every family is in the one list")
        });

        assert_eq!(produced, expected, "the suggestions did not come back in `Family::ALL` order");

        // And that the order this fixture actually exercises is a non-trivial one — the signals are evaluated
        // geometry first, so an implementation that emitted them in evaluation order would fail here.
        assert_eq!(produced, [Family::FaceRecovery, Family::Upscale]);
    }

    /// A photograph that is small, two stops under and warm: every signal but the face one concludes from the pixels.
    fn dark_and_warm(width: u32, height: u32) -> Picture {
        picture_of(fixtures::rendered(&fixtures::scene(width, height), -2.0, fixtures::warm(1.0)))
    }

    /// A photograph with everything wrong that the pixels can show: out of focus, two stops under, warm, and grainy.
    fn everything_wrong(width: u32, height: u32) -> DynamicImage {
        let blurred = fixtures::defocused(&fixtures::detailed(width, height), 4.0);
        fixtures::grainy(&fixtures::rendered(&blurred, -2.0, fixtures::warm(1.0)), 5.0, 9)
    }

    /// The pixel signals' arms are placed by the same one list: all six families come back, in `Family::ALL` order.
    #[tokio::test]
    async fn the_pixel_signals_come_back_in_family_order_beside_the_others() {
        let source = picture_of(everything_wrong(512, 384));

        let concluded = analyse(&source, &a_face()).await.expect("an analysis over a read store");

        assert_eq!(
            concluded.suggested,
            [
                Suggestion::Denoise,
                Suggestion::FaceRecovery,
                Suggestion::LightAdjustment,
                Suggestion::ColorBalance,
                Suggestion::Sharpen,
                Suggestion::Upscale { scale: Scale::clamped(4.0) }
            ],
            "the six signals did not all conclude, in `Family::ALL` order"
        );
        assert!(concluded.incomplete.is_none());
    }

    /// The spec's **"A photograph too small to measure"** and **"A cut-out on a transparent background"**: neither
    /// denoise nor sharpen, and the analysis is neither a failure nor incomplete.
    #[tokio::test]
    async fn a_photograph_with_no_block_to_measure_is_not_suggested_denoise_or_sharpen() {
        let small = picture_of(fixtures::grainy(&fixtures::defocused(&fixtures::detailed(96, 60), 4.0), 8.0, 10));
        let cut_out = picture_of(fixtures::cut_out_of(&fixtures::detailed(512, 384)));

        for (name, source) in [("too small", small), ("cut out", cut_out)] {
            let concluded = analyse(&source, &Faces::empty()).await.expect("not a failure");

            assert!(
                !concluded.suggested.iter().any(|s| matches!(s, Suggestion::Denoise | Suggestion::Sharpen)),
                "{name}: {:?}",
                concluded.suggested
            );
            assert!(concluded.incomplete.is_none(), "{name}");
        }
    }

    /// The spec's **"No model is available"**: the pixel signals install and open nothing, so a face model that will
    /// not open costs the analysis the face signal alone.
    #[tokio::test]
    async fn a_face_model_that_will_not_open_leaves_the_pixel_signals_standing() {
        let backend = Fake { fails_to_open: Some("dt_newyork_fp32"), ..Fake::new() };
        let source = picture_of(everything_wrong(512, 384));

        let concluded = suggest::<Fake>(&backend, None, &source, None, None)
            .await
            .expect("a signal that failed is not a failed call");

        assert_eq!(
            concluded.suggested,
            [
                Suggestion::Denoise,
                Suggestion::LightAdjustment,
                Suggestion::ColorBalance,
                Suggestion::Sharpen,
                Suggestion::Upscale { scale: Scale::clamped(4.0) }
            ]
        );

        let incomplete = concluded.incomplete.expect("the face signal could not be read");
        assert!(
            matches!(&incomplete, InferenceError::Open { artifact, .. } if artifact == "dt_newyork_fp32"),
            "the analysis is incomplete for a reason other than the face model: {incomplete}"
        );
        assert_eq!(backend.acquired(), ["dt_newyork_fp32"], "the pixel signals reached for a model");
    }

    /// A grayscale photograph, two stops under, so that the light signal concludes beside the monochrome one.
    fn black_and_white(width: u32, height: u32) -> Picture {
        picture_of(fixtures::rendered(&fixtures::greyscale(width, height), -2.0, [1.0; 3]))
    }

    /// The spec's **"Colorization beside light adjustment"**: both come back, colorization first.
    #[tokio::test]
    async fn an_underexposed_grayscale_photograph_is_suggested_colorization_and_then_light_adjustment() {
        let concluded = analyse(&black_and_white(512, 384), &Faces::empty()).await.expect("an analysis");

        let position = |wanted: Suggestion| concluded.suggested.iter().position(|suggestion| *suggestion == wanted);
        let (colorization, light) = (position(Suggestion::Colorization), position(Suggestion::LightAdjustment));

        assert!(colorization.is_some() && light.is_some(), "{:?}", concluded.suggested);
        assert!(colorization < light, "light adjustment came first: {:?}", concluded.suggested);
        assert!(!concluded.suggested.contains(&Suggestion::ColorBalance), "a grey was read as cast");
        assert!(concluded.incomplete.is_none());
    }

    /// The spec's **"No model is available"**, for colorization: a face model that will not open costs the analysis
    /// the face signal, and nothing else.
    #[tokio::test]
    async fn a_face_model_that_will_not_open_leaves_the_colorization_suggestion_standing() {
        let backend = Fake { fails_to_open: Some("dt_newyork_fp32"), ..Fake::new() };

        let concluded = suggest::<Fake>(&backend, None, &black_and_white(512, 384), None, None)
            .await
            .expect("a signal that failed is not a failed call");

        assert!(concluded.suggested.contains(&Suggestion::Colorization), "{:?}", concluded.suggested);
        let incomplete = concluded.incomplete.expect("the face signal could not be read");
        assert!(
            matches!(&incomplete, InferenceError::Open { artifact, .. } if artifact == "dt_newyork_fp32"),
            "the analysis is incomplete for a reason other than the face model: {incomplete}"
        );
    }

    /// The spec's **"A cancelled analysis"**, during the passes: they run to their end, and what they measured is not
    /// answered, whichever signal was being read when the token was cancelled.
    #[tokio::test]
    async fn a_cancellation_during_the_passes_withdraws_the_analysis() {
        for (source, between) in [
            (picture_of(everything_wrong(512, 384)), false),
            (picture_of(everything_wrong(512, 384)), true),
            (black_and_white(512, 384), false),
            (black_and_white(512, 384), true),
        ] {
            let cancel = CancellationToken::new();
            let during = cancel.clone();

            let read = pixels_with(&source, &cancel, move |image| {
                // Before the strided pass, or after it and before the block pass.
                if !between {
                    during.cancel();
                }
                let pass = pass::read(image);
                if between {
                    during.cancel();
                }
                PixelSignals::of(
                    Some(&pass),
                    Some(&blocks::read(image)),
                    u64::from(image.width()) * u64::from(image.height()),
                )
            })
            .await;

            assert!(matches!(read, Err(InferenceError::Cancelled)), "a pass cancelled while it ran was answered");
        }
    }

    /// And through the whole analysis: a token already cancelled when the pass would start fails the call, and no
    /// suggestion comes back, not the geometry one determined before it.
    #[tokio::test]
    async fn a_cancelled_analysis_returns_nothing_from_the_pixel_signals() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let source = dark_and_warm(64, 48);

        let options = ExecuteOptions { cancel, ..Default::default() };
        let outcome = suggest::<NoBackend>(&NoBackend, None, &source, None, Some(options)).await;

        assert!(matches!(outcome, Err(InferenceError::Cancelled)), "a withdrawn question gets no answer at all");
    }

    /// The spec's **"A very large photograph"**: at 24 MP a photograph is read at a stride, and reaches the conclusions
    /// it reaches when it is small enough to be read in full.
    #[test]
    fn a_very_large_photograph_concludes_what_it_would_at_a_bounded_size() {
        let (small, large) = ((1024, 768), (6000, 4000));
        assert_eq!(pass::stride(small.0, small.1), 1, "the small frame is not read in full");
        assert!(pass::stride(large.0, large.1) > 1, "the large frame is not read at a stride");

        // Each variant as a photograph, so that the comparison covers a conclusion of each kind, not only "nothing".
        type Variant = fn(DynamicImage) -> DynamicImage;
        let variants: [(&str, Variant); 5] = [
            ("unaltered", |scene| scene),
            ("two stops under", |scene| fixtures::rendered(&scene, -2.0, [1.0; 3])),
            ("two stops over", |scene| fixtures::rendered(&scene, 2.0, [1.0; 3])),
            ("warm", |scene| fixtures::rendered(&scene, 0.0, fixtures::warm(1.0))),
            ("cool", |scene| fixtures::rendered(&scene, 0.0, fixtures::cool(1.0))),
        ];

        let signals = |image: &DynamicImage| PixelSignals::read(image, Scope::of(None));

        for (name, variant) in variants {
            // The same photograph enlarged, rather than drawn again at the larger size: every pixel of it is one of the
            // small frame's, so any difference in what is concluded is the stride's.
            let photograph = variant(fixtures::scene(small.0, small.1));
            let enlarged = photograph.resize_exact(large.0, large.1, image::imageops::FilterType::Nearest);
            let (bounded, full) = (signals(&photograph), signals(&enlarged));
            let concluded = |signals: &PixelSignals| signals.suggestions().collect::<Vec<_>>();

            assert_eq!(concluded(&full), concluded(&bounded), "{name}: the large frame concluded otherwise");

            // And not by luck at a boundary: what each signal measured agrees to within a couple of brightness bins and
            // a fraction of a degree.
            let (bounded_light, full_light) = (bounded.light.expect("samples"), full.light.expect("samples"));
            for (got, want) in [
                (full_light.low, bounded_light.low),
                (full_light.median, bounded_light.median),
                (full_light.high, bounded_light.high),
            ] {
                assert!(
                    (got - want).abs() <= 2.0 / pass::LUMA_BINS as f64,
                    "{name}: {full_light:?} != {bounded_light:?}"
                );
            }

            let (bounded_cast, full_cast) = (bounded.cast.expect("evidence"), full.cast.expect("evidence"));
            assert!(
                (full_cast.angle() - bounded_cast.angle()).abs() < 0.25,
                "{name}: {full_cast:?} != {bounded_cast:?}"
            );
        }
    }

    /// The spec's **"A very large photograph"** for the block pass: grainy and blurred photographs of tens of megapixels
    /// are suggested what a crop of them at their own resolution is.
    #[test]
    fn a_very_large_grainy_or_blurred_photograph_concludes_what_a_crop_of_it_does() {
        let sharp = fixtures::detailed(512, 384);
        let crops = [
            ("grainy", fixtures::grainy(&sharp, 8.0, 11)),
            ("blurred", fixtures::defocused(&sharp, 4.0)),
            ("sharp and clean", sharp.clone()),
        ];

        for (name, crop) in crops {
            // 12 by 12 copies is 28 MP, and its top-left corner is the crop itself.
            let large = fixtures::mirror_tiled(&crop, 12, 12);
            assert!(u64::from(large.width()) * u64::from(large.height()) > 25_000_000);

            let concluded = |image: &DynamicImage| {
                let signals = PixelSignals::read(image, Scope::of(None));
                signals
                    .suggestions()
                    .filter(|s| matches!(s, Suggestion::Denoise | Suggestion::Sharpen))
                    .collect::<Vec<_>>()
            };

            let (small, whole) = (concluded(&crop), concluded(&large));
            assert_eq!(whole, small, "{name}: the large photograph concluded otherwise");
            assert!(name != "grainy" || small == [Suggestion::Denoise], "{name}: {small:?}");
            assert!(name != "blurred" || small == [Suggestion::Sharpen], "{name}: {small:?}");
        }
    }

    /// The spec's **"The reference's order"**: over the families a user can add, the order suggestions come back in is
    /// the reference's present-and-apply order, as a checked fact.
    #[test]
    fn the_order_suggestions_come_back_in_agrees_with_the_reference() {
        // `Family::ALL` is a *declaration* order, and its own documentation says it is deliberately not a presentation
        // order; the chain the enhancement path runs applies operations in whatever order its caller supplies them and
        // imposes no family order either. So there is no "the order the enhancement path applies families" in this
        // crate to defer to — `Family::ALL` is simply the only canonical list there is, which is why the sort uses it.
        //
        // The reference does have one list that is both presented and applied, because an add menu offering families
        // in one order while the pipeline ran them in another is a silent inconsistency. Its reason holds on its own
        // merits: colorization replaces the photograph's colour, so exposure and colour are corrected on the colour it
        // produced rather than on colour it will discard. Asserting the agreement rather than describing it is what
        // makes this fail when either list moves.
        //
        // `['dn', 'fr', 'cl', 'la', 'cb', 'sh', 'up']` in the reference's
        // `cmd/gui/frontend/src/utils/enhancement.ts`, transcribed as families. Detection is absent from it because
        // it is not an enhancement a user adds, which is also why it is dropped from ours below.
        const REFERENCE: [Family; 7] = [
            Family::Denoise,
            Family::FaceRecovery,
            Family::Colorization,
            Family::LightAdjustment,
            Family::ColorBalance,
            Family::Sharpen,
            Family::Upscale,
        ];

        let ours: Vec<_> = Family::ALL.into_iter().filter(|family| *family != Family::Detection).collect();

        assert_eq!(ours, REFERENCE, "the order suggestions come back in disagrees with the reference's");
    }

    /// The spec's **"An analysis completes"**: the conclusion is at the default verbosity and names the families.
    ///
    /// `info` rather than `debug`, because this is the record a user attaches without being asked to reproduce
    /// anything — an analysis that installed a model and ran inference nobody requested has to be accountable in the
    /// file as it ships.
    #[tokio::test]
    async fn a_completed_analysis_records_what_it_concluded_at_the_default_verbosity() {
        let source = picture(64, 48);

        let faces = a_face();
        let (log, outcome) = logging::records_of("info", || analyse(&source, &faces)).await;
        let concluded = outcome.expect("an analysis over a read store");
        assert_eq!(concluded.suggested.len(), 2);

        let [record] = records(&log, "analysis suggested enhancements")[..] else {
            panic!("expected exactly one conclusion in:\n{log}");
        };

        assert_eq!(field(record, "count"), Some("2"));
        assert_eq!(field(record, "identity"), Some(source.identity()));

        let families = field(record, "families").expect("the conclusion names the families");
        assert!(families.contains("Face Recovery"), "the record does not name face recovery: {record}");
        assert!(families.contains("Upscale"), "the record does not name upscale: {record}");
    }

    /// What the pixel signals measured is recorded at `debug`, so a wrong suggestion can be explained from the log
    /// without the photograph, and it stays out of the default verbosity, where the conclusion already is.
    #[tokio::test]
    async fn the_pixel_signals_measurements_are_recorded_at_debug_and_not_at_info() {
        let source = dark_and_warm(64, 48);
        let faces = Faces::empty();

        let (log, outcome) = logging::records_of("debug", || analyse(&source, &faces)).await;
        outcome.expect("an analysis over a read store");

        let [record] = records(&log, "the pixel signals measured")[..] else {
            panic!("expected exactly one record of the measurements in:\n{log}");
        };

        assert_eq!(field(record, "identity"), Some(source.identity()));
        assert_eq!(field(record, "samples"), Some("3072"));
        for name in ["low", "median", "high", "under", "over", "angle", "red", "blue", "cast"] {
            let value = field(record, name).unwrap_or_else(|| panic!("the record does not give `{name}`: {record}"));
            assert!(
                value.trim_start_matches("Some(").trim_end_matches(')').parse::<f64>().is_ok(),
                "{name} = {value}"
            );
        }

        let (log, _) = logging::records_of("info", || analyse(&source, &faces)).await;
        assert!(
            records(&log, "the pixel signals measured").is_empty(),
            "the measurements reached `info`:\n{log}"
        );
    }

    /// What the monochrome signal measured is recorded beside the other pixel signals', at `debug` and not at `info`.
    #[tokio::test]
    async fn the_monochrome_measurements_are_recorded_at_debug_and_not_at_info() {
        let source = black_and_white(512, 384);
        let faces = Faces::empty();

        let (log, outcome) = logging::records_of("debug", || analyse(&source, &faces)).await;
        outcome.expect("an analysis over a read store");

        let [record] = records(&log, "the pixel signals measured")[..] else {
            panic!("expected exactly one record of the measurements in:\n{log}");
        };

        for name in ["unclipped", "residual", "monochrome"] {
            let value = field(record, name).unwrap_or_else(|| panic!("the record does not give `{name}`: {record}"));
            assert!(
                value.trim_start_matches("Some(").trim_end_matches(')').parse::<f64>().is_ok(),
                "{name} = {value}"
            );
        }

        let (log, _) = logging::records_of("info", || analyse(&source, &faces)).await;
        assert!(
            records(&log, "the pixel signals measured").is_empty(),
            "the measurements reached `info`:\n{log}"
        );
    }

    /// What the block pass measured is recorded beside the strided pass's, at `debug` and not at `info`.
    #[tokio::test]
    async fn the_block_pass_measurements_are_recorded_at_debug_and_not_at_info() {
        let source = picture_of(everything_wrong(512, 384));
        let faces = Faces::empty();

        let (log, outcome) = logging::records_of("debug", || analyse(&source, &faces)).await;
        outcome.expect("an analysis over a read store");

        let [record] = records(&log, "the pixel signals measured")[..] else {
            panic!("expected exactly one record of the measurements in:\n{log}");
        };

        assert_eq!(field(record, "blocks"), Some("48"));
        for name in ["luma_noise", "chroma_noise", "noise_luma", "noise", "detailed", "sharpest", "sharpness"] {
            let value = field(record, name).unwrap_or_else(|| panic!("the record does not give `{name}`: {record}"));
            assert!(
                value.trim_start_matches("Some(").trim_end_matches(')').parse::<f64>().is_ok(),
                "{name} = {value}"
            );
        }

        let (log, _) = logging::records_of("info", || analyse(&source, &faces)).await;
        assert!(
            records(&log, "the pixel signals measured").is_empty(),
            "the measurements reached `info`:\n{log}"
        );
    }

    /// The other half of the spec's **"An analysis completes"**: a photograph that needs nothing still produces a
    /// record.
    ///
    /// The case the requirement exists for — an analysis that concluded nothing and an analysis that never ran must
    /// not look alike in the file, and a record written only when there was something to say is exactly how they
    /// come to.
    #[tokio::test]
    async fn an_analysis_that_suggested_nothing_still_records_that_it_concluded() {
        let source = picture(4096, 4096);

        let none = Faces::empty();
        let (log, outcome) = logging::records_of("info", || analyse(&source, &none)).await;
        assert!(outcome.expect("an analysis over a read store").suggested.is_empty());

        let [record] = records(&log, "analysis suggested enhancements")[..] else {
            panic!("an analysis that concluded nothing wrote no record:\n{log}");
        };

        assert_eq!(field(record, "count"), Some("0"));
    }

    /// An image with no area returns before any signal runs, and still says so — the same requirement, over the one
    /// path that returns early.
    #[tokio::test]
    async fn an_analysis_of_an_image_with_no_area_still_records_that_it_concluded() {
        let backend = Fake::new();
        let source = picture(0, 0);

        let (log, outcome) = logging::records_of("info", || suggest::<Fake>(&backend, None, &source, None, None)).await;
        assert!(outcome.expect("a degenerate image is not a failure").suggested.is_empty());

        assert_eq!(records(&log, "analysis suggested enhancements").len(), 1, "no conclusion was recorded:\n{log}");
    }

    /// The spec's **"An analysis is incomplete"**: the signal that could not be read is named beside its reason, on
    /// the conclusion — and the failure itself is recorded once, by the detection that failed.
    #[tokio::test]
    async fn an_incomplete_analysis_records_the_failure_once_and_names_the_signal_on_its_conclusion() {
        let backend = Fake { fails_to_open: Some("dt_newyork_fp32"), ..Fake::new() };
        let source = picture(64, 48);

        let (log, outcome) = logging::records_of("info", || suggest::<Fake>(&backend, None, &source, None, None)).await;
        assert!(outcome.expect("a signal that failed is not a failed call").incomplete.is_some());

        let warnings: Vec<&str> = log.lines().filter(|line| line.contains("level=WARN")).collect();
        let [failed] = warnings[..] else {
            panic!("the failure was not recorded exactly once:\n{log}");
        };
        assert!(failed.contains(r#"msg="analysis failed""#), "{failed}");
        assert!(records(&log, "a signal could not be read").is_empty(), "{log}");

        // Both at one verbosity: a reader following an analysis gets the failure and the conclusion it reached anyway,
        // rather than having to raise the level to see one of them.
        let [concluded] = records(&log, "analysis suggested enhancements")[..] else {
            panic!("an incomplete analysis wrote no conclusion:\n{log}");
        };
        assert!(concluded.contains("level=INFO"), "{concluded}");
        assert_eq!(field(concluded, "count"), Some("1"), "the signals that were read still counted");
        assert_eq!(field(concluded, "incomplete"), Some(FACE_SIGNAL));
        assert!(
            field(concluded, "error").is_some_and(|reason| reason.contains("dt_newyork_fp32")),
            "the conclusion does not give the reason: {concluded}"
        );
        assert!(field(concluded, "duration").is_some(), "{concluded}");
    }

    #[tokio::test]
    async fn an_automatic_analysis_parents_the_face_detection_it_runs() {
        use crate::telemetry::metrics::CACHE_LOOKUPS;

        let source = picture(64, 48);
        let memo = store_holding(&source, &a_face());
        let families = [Family::FaceRecovery];

        // `>`: the store tests beside this one count analysis hits too, without the recording lock this holds.
        let hits = || CACHE_LOOKUPS.value(&[("kind", "analysis"), ("result", "hit")]);
        let before = hits();
        let (_, observed, outcome) = logging::traced_of("info", || {
            suggest::<NoBackend>(&NoBackend, Some(&memo), &source, Some(&families), None)
        })
        .await;
        outcome.unwrap();

        let automatic = observed.only("automatic_analysis");
        assert_eq!(automatic.field("outcome"), Some("finished"));
        assert_eq!(automatic.field("identity"), Some(source.identity()));
        assert_eq!(automatic.field("asked"), Some(Family::FaceRecovery.label()));

        let detection = observed.only("analysis");
        assert_eq!(detection.parent, Some(automatic.index), "the face detection is not the analysis's child");
        assert!(hits() > before, "the stored detection was not counted as a hit");
    }

    #[tokio::test]
    async fn a_withdrawn_automatic_analysis_ends_stopped_and_not_failed() {
        let backend = Fake::new();
        let source = picture(64, 48);
        let options = ExecuteOptions { cancel: CancellationToken::new(), ..Default::default() };
        options.cancel.cancel();

        let (_, observed, outcome) =
            logging::traced_of("info", || suggest::<Fake>(&backend, None, &source, None, Some(options))).await;
        assert!(matches!(outcome, Err(InferenceError::Cancelled)), "{:?}", outcome.err());

        let automatic = observed.only("automatic_analysis");
        assert_eq!(automatic.field("outcome"), Some("stopped"));
        assert_eq!(automatic.field("error"), None, "a withdrawal was marked failed");
    }

    /// The spec's **"An automatic analysis is withdrawn"**: a cancelled analysis names the photograph and says it
    /// stopped, and nothing says it failed.
    #[tokio::test]
    async fn a_cancelled_analysis_is_recorded_as_stopped_and_writes_no_warning() {
        let backend = Fake::new();
        let source = picture(64, 48);
        let options = ExecuteOptions { cancel: CancellationToken::new(), ..Default::default() };
        options.cancel.cancel();

        let (log, outcome) =
            logging::records_of("info", || suggest::<Fake>(&backend, None, &source, None, Some(options))).await;
        assert!(matches!(outcome, Err(InferenceError::Cancelled)), "{:?}", outcome.err());

        let [stopped] = records(&log, "automatic analysis stopped")[..] else {
            panic!("the withdrawal was not recorded:\n{log}");
        };
        assert!(stopped.contains("level=INFO"), "{stopped}");
        assert_eq!(field(stopped, "identity"), Some(source.identity()));
        assert_eq!(field(stopped, "reason"), Some("cancelled"));
        assert!(field(stopped, "duration").is_some(), "{stopped}");
        assert!(!log.contains("level=WARN"), "a withdrawal wrote a warning:\n{log}");
        assert!(records(&log, "analysis suggested enhancements").is_empty(), "{log}");
    }

    /// The other half of the same match: a withdrawn question is not a signal that could not be read.
    #[tokio::test]
    async fn a_cancelled_face_signal_withdraws_the_question_rather_than_reporting_a_failure() {
        let cancel = CancellationToken::new();
        let backend = Fake { cancels: Some(cancel.clone()), ..Fake::new() };
        let source = picture(64, 48);

        let options = ExecuteOptions { cancel, ..Default::default() };
        let read = face::<Fake>(&backend, None, &source, options).await;

        assert!(matches!(read, Err(Unread::Withdrawn(InferenceError::Cancelled))));
    }

    /// The spec's **"A set of two families"**: of the four things the photograph calls for, only the two asked for come
    /// back, in order, and no detection is run although the backend would find a face.
    #[tokio::test]
    async fn a_narrowed_analysis_suggests_only_the_families_asked_for_and_runs_no_detection() {
        let everything = Fake::finding_a_face();
        let concluded = suggest::<Fake>(&everything, None, &black_and_white(512, 384), None, None)
            .await
            .expect("an analysis");

        // What the fixture calls for when nothing is narrowed, so the narrowing below has something to leave out.
        let families: Vec<_> = concluded.suggested.iter().map(Suggestion::family).collect();
        assert_eq!(families, [Family::FaceRecovery, Family::Colorization, Family::LightAdjustment, Family::Upscale]);

        let backend = Fake::finding_a_face();
        let concluded = suggest::<Fake>(
            &backend,
            None,
            &black_and_white(512, 384),
            Some(&[Family::Upscale, Family::Colorization]),
            None,
        )
        .await
        .expect("a narrowed analysis");

        assert_eq!(
            concluded.suggested,
            [Suggestion::Colorization, Suggestion::Upscale { scale: Scale::clamped(4.0) }]
        );
        assert!(concluded.incomplete.is_none());
        assert!(backend.acquired().is_empty(), "a detection ran for an analysis that did not ask for faces");
    }

    /// The spec's **"Face recovery is not asked for and the detection model cannot be obtained"** and its converse: a
    /// model that would not open only makes the analysis incomplete when the analysis tried to open it.
    #[tokio::test]
    async fn a_face_model_that_will_not_open_makes_the_analysis_incomplete_only_when_faces_were_asked_for() {
        let source = picture(64, 48);
        let upscale = Suggestion::Upscale { scale: Scale::clamped(4.0) };

        let backend = Fake { fails_to_open: Some("dt_newyork_fp32"), ..Fake::new() };
        let concluded = suggest::<Fake>(&backend, None, &source, Some(&[Family::Upscale]), None)
            .await
            .expect("a signal that was not read is not a failure");

        assert_eq!(concluded.suggested, [upscale]);
        assert!(concluded.incomplete.is_none(), "a signal nobody asked for made the analysis incomplete");
        assert!(backend.acquired().is_empty());

        let backend = Fake { fails_to_open: Some("dt_newyork_fp32"), ..Fake::new() };
        let concluded = suggest::<Fake>(&backend, None, &source, Some(&[Family::Upscale, Family::FaceRecovery]), None)
            .await
            .expect("a signal that failed is not a failed call");

        assert_eq!(concluded.suggested, [upscale], "the other family in the set still came back");
        let incomplete = concluded.incomplete.expect("the face signal was asked for and could not be read");
        assert!(
            matches!(&incomplete, InferenceError::Open { artifact, .. } if artifact == "dt_newyork_fp32"),
            "{incomplete}"
        );
    }

    /// The spec's **"A family whose signal also serves another family"**: the block pass is read for sharpen, and
    /// the grain it also measures is not suggested.
    #[tokio::test]
    async fn a_signal_read_for_one_family_suggests_no_other() {
        let source = picture_of(everything_wrong(512, 384));

        // `NoBackend` panics if it is reached: face recovery was not asked for.
        let concluded = suggest::<NoBackend>(&NoBackend, None, &source, Some(&[Family::Sharpen]), None)
            .await
            .expect("a narrowed analysis");

        assert_eq!(concluded.suggested, [Suggestion::Sharpen]);
        assert!(concluded.incomplete.is_none());
    }

    /// The spec's **"An empty set"** and **"A set naming only face detection"**: nothing comes back, nothing is
    /// read, and the analysis is complete. That no pixel was read is the absence of the measurements' record; that no
    /// detection ran is `NoBackend`, which panics if it is reached.
    #[tokio::test]
    async fn a_set_with_nothing_suggestible_reads_nothing_and_is_complete() {
        let source = picture_of(everything_wrong(512, 384));

        for families in [&[][..], &[Family::Detection][..]] {
            let (log, outcome) =
                logging::records_of("debug", || suggest::<NoBackend>(&NoBackend, None, &source, Some(families), None))
                    .await;
            let concluded = outcome.expect("asking for nothing is not a failure");

            assert!(concluded.suggested.is_empty(), "{families:?}: {:?}", concluded.suggested);
            assert!(concluded.incomplete.is_none(), "{families:?}");
            assert!(records(&log, "the pixel signals measured").is_empty(), "{families:?} read the pixels:\n{log}");
            assert_eq!(records(&log, "analysis suggested enhancements").len(), 1, "{families:?}:\n{log}");
        }
    }

    /// The spec's **"A narrowed analysis is cancelled"**: cancelled while the face signal runs, it returns nothing.
    #[tokio::test]
    async fn a_narrowed_analysis_cancelled_during_the_face_signal_returns_nothing() {
        let cancel = CancellationToken::new();
        let backend = Fake { cancels: Some(cancel.clone()), ..Fake::finding_a_face() };

        let options = ExecuteOptions { cancel, ..Default::default() };
        let outcome =
            suggest::<Fake>(&backend, None, &picture(64, 48), Some(&[Family::FaceRecovery]), Some(options)).await;

        assert!(matches!(outcome, Err(InferenceError::Cancelled)), "a withdrawn question gets no answer at all");
        assert_eq!(backend.acquired(), ["dt_newyork_fp32"], "the face signal was not the one cancelled");
    }

    /// A scope of upscale alone reaches neither place a cancellation is otherwise observed, and is still withdrawn.
    #[tokio::test]
    async fn a_cancelled_analysis_narrowed_to_upscale_returns_nothing() {
        let cancel = CancellationToken::new();
        cancel.cancel();

        let options = ExecuteOptions { cancel, ..Default::default() };
        let outcome =
            suggest::<NoBackend>(&NoBackend, None, &picture(64, 48), Some(&[Family::Upscale]), Some(options)).await;

        assert!(matches!(outcome, Err(InferenceError::Cancelled)));
    }

    /// The design's guard against a signal added to a read whose `needs_*` does not name its family: every family an
    /// analysis can suggest is still suggested when it is the only one asked for, on a photograph that calls for it.
    #[tokio::test]
    async fn every_suggestible_family_is_suggested_when_it_is_asked_for_alone() {
        for family in Family::ALL.into_iter().filter(|family| *family != Family::Detection) {
            let source = match family {
                Family::Colorization => black_and_white(512, 384),
                _ => picture_of(everything_wrong(512, 384)),
            };
            let backend = Fake::finding_a_face();

            let concluded = suggest::<Fake>(&backend, None, &source, Some(&[family]), None)
                .await
                .expect("a narrowed analysis");

            let families: Vec<_> = concluded.suggested.iter().map(Suggestion::family).collect();
            assert_eq!(families, [family], "{family:?} alone was not suggested");
            assert!(concluded.incomplete.is_none(), "{family:?}");
            assert_eq!(!backend.acquired().is_empty(), family == Family::FaceRecovery, "{family:?}");
        }
    }

    /// The spec's **"The record of a narrowed analysis"**: the families asked for are named at `info`, in
    /// `Family::ALL` order, and an analysis nobody narrowed says so.
    #[tokio::test]
    async fn the_record_names_the_families_asked_for() {
        let source = black_and_white(512, 384);

        let (log, outcome) = logging::records_of("info", || {
            suggest::<NoBackend>(&NoBackend, None, &source, Some(&[Family::Upscale, Family::Colorization]), None)
        })
        .await;
        outcome.expect("a narrowed analysis");

        let [record] = records(&log, "analysis suggested enhancements")[..] else {
            panic!("expected exactly one conclusion in:\n{log}");
        };
        assert_eq!(field(record, "asked"), Some("Colorization, Upscale"), "{record}");
        assert_eq!(field(record, "families"), Some("Colorization, Upscale"), "{record}");

        let faces = Faces::empty();
        let (log, outcome) = logging::records_of("info", || analyse(&source, &faces)).await;
        outcome.expect("an analysis over a read store");

        let [record] = records(&log, "analysis suggested enhancements")[..] else {
            panic!("expected exactly one conclusion in:\n{log}");
        };
        assert_eq!(field(record, "asked"), Some("all"), "{record}");
    }

    /// An operation of `family`, built the way a front end that had the user's model choice would build one.
    ///
    /// The variant and precision are this test's own rather than a suggestion's — a suggestion names neither — and
    /// which one is picked does not matter to the question being asked: `Operation::pipeline` refuses by family, so
    /// any variant of a refused family is refused and any variant of a served one is served.
    fn operation_for(family: Family, scale: Scale) -> Operation {
        match family {
            Family::Denoise => Denoise::stockholm(FloatPrecision::Fp32, Strength::clamped(1.0)),
            Family::Sharpen => Sharpen::moscow(FloatPrecision::Fp32, Strength::clamped(1.0)),
            Family::FaceRecovery => FaceRecovery::athens(FloatPrecision::Fp32, Faces::empty(), Fidelity::MAXIMUM),
            Family::Colorization => Colorization::delhi(FloatPrecision::Fp32),
            Family::LightAdjustment => LightAdjustment::lyon(FloatPrecision::Fp32, Bias::clamped(0.0)),
            Family::ColorBalance => ColorBalance::rio(FloatPrecision::Fp32, Bias::clamped(0.0)),
            Family::Upscale => Upscale::tokyo(FloatPrecision::Fp32, scale),
            other => unreachable!("no suggestion names {other:?}, so nothing has to build an operation for it"),
        }
    }

    /// The spec's **"Nothing is suggested that the application cannot run"**, as a test rather than as discipline.
    ///
    /// A suggestion is an instruction the user is invited to accept with one gesture, and one that is refused the
    /// moment it is accepted is worse than no suggestion at all — the user is told the photograph needs something
    /// and then told the application will not do it.
    ///
    /// The rule is a standing one over whatever the application can run *at the time*, so it is asked of the
    /// enhancement path rather than of a transcribed list: an arm added here for a family the enhancement path
    /// refuses fails this, and a family that gains a pipeline later becomes suggestible with nothing here to change.
    ///
    /// It holds for a narrowed analysis too, with no test of its own: narrowing only removes suggestions, so a narrowed
    /// answer is a subset of the arms checked here.
    #[test]
    fn every_suggestion_names_a_family_the_enhancement_path_can_apply() {
        let scale = Scale::clamped(2.0);

        // Every arm, listed here rather than derived: the point is that adding an arm without adding it here is
        // caught, which the exhaustive match below is what does.
        let arms = [
            Suggestion::Denoise,
            Suggestion::FaceRecovery,
            Suggestion::Colorization,
            Suggestion::LightAdjustment,
            Suggestion::ColorBalance,
            Suggestion::Sharpen,
            Suggestion::Upscale { scale },
        ];

        // The exhaustive match that fails to compile when an arm is added and not listed above.
        for suggestion in arms {
            match suggestion {
                Suggestion::Denoise
                | Suggestion::FaceRecovery
                | Suggestion::Colorization
                | Suggestion::LightAdjustment
                | Suggestion::ColorBalance
                | Suggestion::Sharpen
                | Suggestion::Upscale { .. } => {}
            }

            let operation = operation_for(suggestion.family(), scale);

            assert_eq!(operation.family(), suggestion.family(), "the operation built is not of the suggested family");
            assert!(
                operation.pipeline::<NoBackend>().is_ok(),
                "{:?} is suggested but the enhancement path refuses its family",
                suggestion.family()
            );
        }
    }

    /// The accessor, arm by arm, so a rewiring that compiles is still caught.
    #[test]
    fn a_suggestion_names_its_own_family() {
        assert_eq!(Suggestion::Denoise.family(), Family::Denoise);
        assert_eq!(Suggestion::FaceRecovery.family(), Family::FaceRecovery);
        assert_eq!(Suggestion::Colorization.family(), Family::Colorization);
        assert_eq!(Suggestion::LightAdjustment.family(), Family::LightAdjustment);
        assert_eq!(Suggestion::ColorBalance.family(), Family::ColorBalance);
        assert_eq!(Suggestion::Sharpen.family(), Family::Sharpen);
        assert_eq!(Suggestion::Upscale { scale: Scale::clamped(4.0) }.family(), Family::Upscale);
    }

    /// The scale is part of the suggestion, and is the quantized value the family accepts rather than a bare number.
    #[test]
    fn an_upscale_suggestion_carries_the_scale_it_was_built_with() {
        let suggestion = Suggestion::Upscale { scale: Scale::clamped(4.0) };

        let Suggestion::Upscale { scale } = suggestion else {
            panic!("the arm built is not the arm matched");
        };

        assert_eq!(scale.get(), 4.0);
        assert_eq!(Upscale::tokyo(FloatPrecision::Fp32, scale).family(), Family::Upscale);
        // The variant the test built the operation from, named so the helper above cannot quietly change family.
        assert!(matches!(
            Upscale::tokyo(FloatPrecision::Fp32, scale),
            Operation::Upscale(upscale) if matches!(upscale.variant(), UpscaleVariant::Tokyo(_))
        ));
    }

    /// The scale a picture of `pixels` is told it needs, or `None`, expressed as a number a reader can compare
    /// against the spec's table.
    ///
    /// The extents are derived from the count rather than given, because the ladder reads the count and nothing
    /// else: a 1024x1024 image and a 1,048,576x1 one are the same input to it, and writing pairs would suggest
    /// otherwise.
    fn scale_for(pixels: u64) -> Option<f64> {
        let width = u32::try_from(pixels).expect("every boundary below fits a single row");

        geometry(width, 1).map(|suggestion| match suggestion {
            Suggestion::Upscale { scale } => scale.get(),
            other => panic!("the geometry signal produced {other:?}, which is not an upscale"),
        })
    }

    /// The lower boundary, which the spec pins exactly: at 1 MP the answer is 4x and not 2x.
    ///
    /// Both halves are asserted, because an off-by-one that moved the boundary one pixel down would still produce a
    /// suggestion — just the wrong one — and a test that only asked "is upscaling suggested" would pass.
    #[test]
    fn exactly_one_megapixel_is_told_it_needs_four_times() {
        assert_eq!(scale_for(1_048_576), Some(4.0));
        assert_ne!(scale_for(1_048_576), Some(2.0));
    }

    #[test]
    fn one_pixel_above_a_megapixel_is_told_it_needs_twice() {
        assert_eq!(scale_for(1_048_577), Some(2.0));
    }

    /// The upper boundary, inclusive: at 4 MP there is still something to suggest.
    #[test]
    fn exactly_four_megapixels_is_told_it_needs_twice() {
        assert_eq!(scale_for(4_194_304), Some(2.0));
    }

    /// One pixel above it, where the reference returns a scale of 1 — a factor that changes nothing — and this says
    /// nothing at all, which is the same answer without an operation to run.
    #[test]
    fn one_pixel_above_four_megapixels_is_told_it_needs_nothing() {
        assert_eq!(scale_for(4_194_305), None);
    }

    #[test]
    fn the_ladder_reads_the_area_rather_than_either_extent() {
        assert_eq!(geometry(1024, 1024), geometry(2048, 512));
        assert_eq!(geometry(1024, 1024), Some(Suggestion::Upscale { scale: Scale::clamped(4.0) }));
    }

    #[test]
    fn a_pixel_count_beyond_a_u32_does_not_wrap_into_a_suggestion() {
        // 65_536 * 65_536 is exactly 2^32, which is 0 in `u32` arithmetic — and 0 is at the bottom of the ladder.
        assert_eq!(geometry(65_536, 65_536), None);
    }

    /// An empty set of suggestions and one that could not be completed are different values, which is the whole
    /// reason [`Suggestions`] is a struct rather than a `Vec`.
    #[test]
    fn nothing_needed_and_could_not_tell_are_not_the_same_value() {
        let nothing_needed = Suggestions { suggested: Vec::new(), incomplete: None };
        let could_not_tell = Suggestions { suggested: Vec::new(), incomplete: Some(InferenceError::Cancelled) };

        assert!(nothing_needed.suggested.is_empty());
        assert!(nothing_needed.incomplete.is_none());

        assert!(could_not_tell.suggested.is_empty());
        assert!(could_not_tell.incomplete.is_some());
    }
}
