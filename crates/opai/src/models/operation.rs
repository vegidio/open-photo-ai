//! The values that carry an operation of any family, so that holding, storing, sending or dispatching on one does not
//! mean knowing which family it is.
//!
//! **Two of them, one per run path.** [`Operation`] carries the seven families whose result is an image and is what
//! [`Opai::process`](crate::Opai::process) takes; [`Analysis`] carries the one family whose result is not and is what
//! [`Opai::execute`](crate::Opai::execute) takes. Which of the two an operation is carried in follows from the model
//! being named — every model's constructor returns the carrier for the path it runs on — so a caller naming a model
//! is never also asked which path it runs on and can never answer wrongly.
//!
//! The split is what makes both pairings unwritable rather than refused: an upscale cannot reach `execute`, and a
//! detection cannot be placed in a chain.

// Neither pairing is a refusal reported at run time, which is the whole point — a system that decided this while the
// model was already loaded would report as an error what was never a legitimate request.
//
// A file directly under `models/` because it belongs to no family: it is the seam over all eight. What neither
// carrier is is a ninth shape. Each arm holds that family's own type, unchanged, so a family that later diverges grows
// its own type rather than widening a shape the others share.

use serde::{Deserialize, Serialize};

use super::artifact::{ArtifactId, Family};
use super::color_balance::ColorBalance;
use super::colorization::Colorization;
use super::denoise::Denoise;
use super::detection::{Detection, DetectionVariant};
use super::face::Faces;
use super::face_recovery::FaceRecovery;
use super::light_adjustment::LightAdjustment;
use super::precision::Precision;
use super::sharpen::Sharpen;
use super::upscale::Upscale;

use crate::error::InferenceError;
use crate::pipeline::Backend;
use crate::pipeline::{Shared, SharedData};
use crate::providers::profile::EpProfile;

/// An operation of any family whose result is an image, and what [`Opai::process`](crate::Opai::process) takes.
///
/// [`Analysis`]' counterpart. Detection is **not** here: its result is a set of faces rather than an image, so there
/// is no enhanced picture for it to contribute to a chain and there never will be.
///
/// # Which of the two to reach for
///
/// There are two ways to name an operation, and they are for different moments. The **family's own type** —
/// [`Denoise`], [`Upscale`] and the other six — is what you *build*: its constructor takes exactly what that family
/// takes and nothing else, which is what makes a wrong pairing unwritable rather than refused. This type is what you
/// *keep*: store it, put it in a list of operations to apply, serialize it across a process boundary, match on it to
/// dispatch. Build typed, carry this.
///
/// Carrying one costs nothing that was guaranteed before. The arm holds the family's own value, so a
/// [`Operation::Denoise`] can only ever hold a `Denoise`, and everything this reports it reports by asking that
/// value.
///
/// # What it reports
///
/// [`family`](Self::family), [`precision`](Self::precision), [`display_name`](Self::display_name),
/// [`required_artifacts`](Self::required_artifacts) and [`cache_tag`](Self::cache_tag) — the five questions that
/// mean the same thing for all eight families, each answered by the arm. [`Analysis`] answers the same five, so a
/// consumer that only reports on a run can be written once over either. Anything only one family can answer stays
/// on that family's type: [`Upscale::resolve`] and the per-family `params` are
/// reached by matching this and asking the value inside.
///
/// # Across a process boundary
///
/// Serialized as a discriminated union tagged `family`, spelled exactly as [`Family`] spells itself, with the
/// family's own fields alongside — so a front end can build a chooser from it and send the choice back.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub enum Operation {
    // Don't grow an arm-per-family method here for a question seven families would have to invent an answer to.
    //
    // The serialized representation is a compatibility surface once anything persists one, and is pinned by the tests
    // below rather than left to whatever the derive happens to produce.
    /// A denoise run.
    Denoise(Denoise),
    /// A face recovery run, over faces a detection run found.
    FaceRecovery(FaceRecovery),
    /// A light adjustment run.
    LightAdjustment(LightAdjustment),
    /// A colour balance run.
    ColorBalance(ColorBalance),
    /// A colorization run.
    Colorization(Colorization),
    /// A sharpen run.
    Sharpen(Sharpen),
    /// An upscale run.
    Upscale(Upscale),
}

// The five methods below differ in nothing but the name of the method they call, and writing the seven arms out five
// times is five chances to wire an arm to another family's value — a mistake that compiles. Written once, an eighth
// image family is still a compile error in every one of them.
//
// `Operation::family` is deliberately not written through this: it is the one match that answers from the arm itself
// rather than forwarding, and there is nothing to call on the value it holds.
/// Forwards a question every family answers to the arm carrying it.
macro_rules! forward {
    ($operation:ident, $question:ident) => {
        match $operation {
            Self::Denoise(operation) => operation.$question(),
            Self::FaceRecovery(operation) => operation.$question(),
            Self::LightAdjustment(operation) => operation.$question(),
            Self::ColorBalance(operation) => operation.$question(),
            Self::Colorization(operation) => operation.$question(),
            Self::Sharpen(operation) => operation.$question(),
            Self::Upscale(operation) => operation.$question(),
        }
    };
}

impl Operation {
    /// Which family this operation belongs to.
    pub const fn family(&self) -> Family {
        // A match over the arms rather than a field, so the family cannot disagree with the value it is carried
        // beside, and a ninth arm cannot compile without answering.
        match self {
            Self::Denoise(_) => Family::Denoise,
            Self::FaceRecovery(_) => Family::FaceRecovery,
            Self::LightAdjustment(_) => Family::LightAdjustment,
            Self::ColorBalance(_) => Family::ColorBalance,
            Self::Colorization(_) => Family::Colorization,
            Self::Sharpen(_) => Family::Sharpen,
            Self::Upscale(_) => Family::Upscale,
        }
    }

    /// The precision this operation runs at, widened to the common currency artifact names are composed from.
    pub fn precision(&self) -> Precision {
        forward!(self, precision)
    }

    /// The name a user interface shows for this operation: `Stockholm (FP32)`, `Kyoto 4x (FP16)`.
    pub fn display_name(&self) -> String {
        forward!(self, display_name)
    }

    /// The artifacts that must be on disk before this operation can run.
    ///
    /// Which is the question whoever downloads what a queue of operations needs asks of every family alike — one
    /// artifact for seven of them, up to three for an upscale, and the caller need not know which it is holding.
    pub fn required_artifacts(&self) -> Vec<ArtifactId> {
        forward!(self, required_artifacts)
    }

    /// The key the run cache stores this operation's result under.
    ///
    /// Each family writes its own, and no two of them collide — the one namespace shared by all eight.
    pub fn cache_tag(&self) -> String {
        forward!(self, cache_tag)
    }

    /// The model this operation runs, as a metric is tagged with it: `dn_stockholm_fp32`, `up_kyoto_4x_fp16`.
    ///
    /// The name of the first artifact it needs, so a strength or a requested scale never reaches it and two operations
    /// differing only in one share it. An upscale whose scale selects the weights names the native scale of its first
    /// pass. The values are the published catalogue's, never [`cache_tag`](Self::cache_tag)'s.
    pub(crate) fn model_tag(&self) -> String {
        // Every operation needs at least one artifact, so the empty fallback is never reached.
        self.required_artifacts()
            .into_iter()
            .next()
            .map(|artifact| artifact.as_str().to_string())
            .unwrap_or_default()
    }

    /// The execution-provider tuning measured for the model this operation runs.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the cross-family answer the suite holds all eight to; a run reaches a profile through Model::sessions"
        )
    )]
    pub(crate) fn profile(&self) -> EpProfile {
        // Forwarded to the arm exactly as the questions above are, so a caller asks one type rather than matching
        // eight. `pub(crate)` rather than `pub` because what it returns is provider vocabulary this crate configures a
        // session with, not something a front end offers or stores.
        //
        // **Nothing in the running application asks this**, and that is the point of the seam it sits behind rather
        // than a sign it has rotted. A session is built from what `pipeline` hands back — pairs of an artifact and the
        // settings measured *for that artifact* — so a profile is never looked up beside the thing it belongs to. What
        // is left here is the cross-family answer the suite holds every one of the eight to: that each variant
        // declares a profile, and that only the measured ones declare anything but the default.
        forward!(self, profile)
    }

    /// The pipeline this operation runs — the **family** seam, and the whole of what the chain driver knows about
    /// models.
    ///
    /// # Errors
    ///
    /// Whatever the operation's own family seam refuses, as [`InferenceError::Unsupported`] naming the operation.
    /// Every family has a pipeline, and today no published variant is refused. The one refusal a seam can still make
    /// is [`UnsupportedReason::IncompleteGraphSet`](crate::error::UnsupportedReason::IncompleteGraphSet), upscale's
    /// guard against a diffusion variant declared without one of its graphs.
    pub(crate) fn pipeline<B: Backend>(&self) -> Result<Shared<B>, InferenceError> {
        // Not written through `forward!`, unlike the questions above, because the seams do not agree on a return
        // type: two keep a `Result` and five cannot refuse, and those five are wrapped here rather than made to
        // return a `Result` they have no failure to put in.
        //
        // Every arm hands straight over to its own family seam, which is where a family's variants and contracts are
        // told apart, and where a family that could not run a variant would say so, by an arm of its **own** seam
        // rather than by this one. This level knows only families, and the chain driver above it knows neither.
        //
        // There is no arm here for an operation that produces no image: detection is carried in `Analysis`, which
        // never reaches a chain. A refusal replaced by a type is the trade the carrier split exists to make.
        match self {
            Self::Upscale(upscale) => upscale.pipeline::<B>(),
            // The one family seam that cannot refuse, wrapped here rather than made to return a `Result` it
            // has no failure to put in — see `FaceRecovery::pipeline`.
            Self::FaceRecovery(recovery) => Ok(recovery.pipeline::<B>()),
            // The second family seam that cannot refuse, wrapped here for the same reason face recovery's is: both
            // of this family's variants run through one contract, so there is no state left for it to fail on.
            Self::LightAdjustment(adjustment) => Ok(adjustment.pipeline::<B>()),
            // The one family seam that keeps a `Result` it has nothing to refuse with — see
            // `ColorBalance::pipeline` for why it is kept rather than narrowed.
            Self::ColorBalance(balance) => balance.pipeline::<B>(),
            // All three variants run through one contract, so there is nothing left for this seam to refuse either.
            Self::Denoise(denoise) => Ok(denoise.pipeline::<B>()),
            // The same contract as denoise, shared through `models::filter`, and as little to refuse.
            Self::Sharpen(sharpen) => Ok(sharpen.pipeline::<B>()),
            // Wrapped for light adjustment's reason: all three variants run through one pipeline, the two contracts
            // told apart as data on the variant, so there is nothing for this seam to refuse.
            Self::Colorization(colorization) => Ok(colorization.pipeline::<B>()),
        }
    }
}

/// An operation of any family whose result is **not** an image, and what [`Opai::execute`](crate::Opai::execute)
/// takes.
///
/// [`Operation`]'s counterpart. [`Detection::newyork`] returns one of these;
/// the other nineteen constructors return an [`Operation`].
///
/// # What it reports
///
/// The same five questions [`Operation`] answers — [`family`](Self::family), [`precision`](Self::precision),
/// [`display_name`](Self::display_name), [`required_artifacts`](Self::required_artifacts) and
/// [`cache_tag`](Self::cache_tag) — each answered by the arm, so whoever installs what a run needs or labels what a
/// run is doing asks the same questions of either carrier.
///
/// # Neither carrier reaches the other's entry point
///
/// The `compile_fail` doctest on [`DataOperation`] pins one half of that: an operation whose result is an image does
/// not satisfy the bound `execute` takes. This is the other half. An analysis is not an [`Operation`], there is no
/// conversion into one and no arm of one to hold it, so it cannot be placed in the ordered chain
/// [`Opai::process`](crate::Opai::process) takes — which is the same statement the first makes, read from this end.
///
/// ```compile_fail,E0308
/// # use opai::{Detection, FloatPrecision, Operation};
/// // A chain is homogeneous, and this is the type `Opai::process` takes. The error is a mismatch rather than an
/// // unsatisfied bound: what refuses it is that the two carriers are two types, with nothing converting either way.
/// let chain: Vec<Operation> = vec![Detection::newyork(FloatPrecision::Fp32)];
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub enum Analysis {
    // One arm, and not speculative structure: it is not here for a second family whose result is not an image. It is
    // what lets every model's constructor return "the carrier for the path it runs on" without detection reading as an
    // exception: `Operation` could not be `execute`'s parameter, because one result type for seven image families is
    // either wrong or an enum, and an `Operation` carrying an `Upscale` would reach `execute` freely. The alternatives
    // are an eighth family that reads differently from the other seven, or a refusal reported at run time.
    //
    // A second arm here would force `execute`'s return to become an enum the caller matches on, or force a generic
    // bound back onto it. That cost is deferred rather than avoided.
    /// A face-detection run, whose result is the set of [`Faces`] found.
    Detection(Detection),
}

impl Analysis {
    /// Which family this operation belongs to.
    pub const fn family(&self) -> Family {
        // A match over the arms rather than a field, for the reason `Operation::family` gives.
        match self {
            Self::Detection(_) => Family::Detection,
        }
    }

    /// The precision this operation runs at, widened to the common currency artifact names are composed from.
    pub fn precision(&self) -> Precision {
        match self {
            Self::Detection(operation) => operation.precision(),
        }
    }

    /// The name a user interface shows for this operation: `New York (FP32)`.
    pub fn display_name(&self) -> String {
        match self {
            Self::Detection(operation) => operation.display_name(),
        }
    }

    /// The artifacts that must be on disk before this operation can run.
    pub fn required_artifacts(&self) -> Vec<ArtifactId> {
        match self {
            Self::Detection(operation) => operation.required_artifacts(),
        }
    }

    /// The key the store keeps this operation's result under, in one namespace shared with every enhancement
    /// family's.
    pub fn cache_tag(&self) -> String {
        match self {
            Self::Detection(operation) => operation.cache_tag(),
        }
    }
    /// The execution-provider tuning measured for the model this operation runs: [`Operation::profile`]'s
    /// counterpart.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the cross-family answer the suite holds all eight to; a run reaches a profile through Model::sessions"
        )
    )]
    pub(crate) fn profile(&self) -> EpProfile {
        // `pub(crate)` for the reason `Operation::profile` gives: what it returns is provider vocabulary this crate
        // configures a session with, not something a front end offers or stores.
        match self {
            Self::Detection(operation) => operation.profile(),
        }
    }
}

/// The half of [`DataOperation`] this crate keeps to itself.
pub(crate) mod sealed {
    // Public inside a private module, which is the sealing: an external crate can name `DataOperation` as a bound but
    // cannot implement it, because it cannot name the supertrait. That is load-bearing rather than defensive —
    // `SharedData` and `Backend` are `pub(crate)`, so a public trait carrying the pipeline method would put
    // crate-private types into a public interface. This pattern lets `DataOperation` be named publicly as a bound
    // while the method it dispatches through stays inside the crate.

    use super::{Backend, InferenceError, SharedData};

    /// An operation whose result is not an image, and what running one reaches.
    pub trait DataModel {
        // Storable because a run on this path is **kept**: the same analysis of the same photograph is served from the
        // store rather than executed a second time, which is what makes a repeated detection instant rather than a
        // session build. The bound is therefore on the type rather than on `Opai::execute`'s signature, and the
        // difference is the whole point — a bound on the function would let an operation exist whose result cannot be
        // stored, and the first place anyone found out would be a call site. On the associated type it is a property
        // every implementor proves once, so the next family whose result is not an image is cached by existing rather
        // than by a change here.
        //
        // `Send + 'static` is part of the same statement: the result crosses to a blocking thread to be produced and
        // again to be written, and a type that could not make that crossing could not be produced on this path at all.
        /// What one run of this operation produces. [`Detection`](super::Detection) names
        /// [`Faces`](super::Faces).
        type Output: serde::Serialize + serde::de::DeserializeOwned + Send + 'static;

        // The one place the two spellings meet. The bar's label and the log's fields name a run through `Subject`, which
        // carries an operation of either path — so a detection reports exactly as an upscale does, and neither
        // reporting seam had to learn a second vocabulary for one idea.
        /// This operation as the value everything that reports on a run speaks.
        fn as_subject(&self) -> crate::models::Subject;

        // A **generic** method rather than an object-safe one, unlike `Operation::pipeline`: nothing here is held as a
        // trait object, because `execute` is monomorphised per operation type, which is what makes the binding between
        // an operation and its result type a compile-time one in the first place.
        /// The pipeline this operation runs — the family's contract seam, and the whole of what the execute driver
        /// knows about models.
        ///
        /// # Errors
        ///
        /// Whatever the family's own seam refuses.
        #[expect(
            private_bounds,
            private_interfaces,
            reason = "the seal is the point: this method speaks the crate-private `pipeline` vocabulary and is unreachable outside it"
        )]
        fn pipeline<B: Backend>(&self) -> Result<SharedData<B, Self::Output>, InferenceError>;
    }
}

/// An operation whose result is **not** an image, and which is therefore run through
/// [`Opai::execute`](crate::Opai::execute) rather than [`Opai::process`](crate::Opai::process).
///
/// **This crate is the only implementor**, and the trait is sealed so it stays that way: what an implementing type
/// provides is the vocabulary of this crate's own inference seam, which is not public API. What it gives a caller is
/// the bound on `execute` and the associated `Output` that bound resolves — so
/// `opai.execute(&picture, &analysis, None).await?.value` is a [`Faces`] with nothing to state and nothing to
/// unwrap, and an operation that produces an image cannot be handed to `execute` at all:
///
/// ```compile_fail
/// # use opai::{DataOperation, FloatPrecision, Scale, Upscale};
/// // An upscale produces an image, so it is carried in `Operation` rather than `Analysis` and the bound does not
/// // hold. It is also what `Opai::process` takes, which is the other half of the same statement.
/// fn only_data<O: DataOperation>(_operation: &O) {}
/// only_data(&Upscale::kyoto(FloatPrecision::Fp32, Scale::new(4.0).unwrap()));
/// ```
///
/// [`Analysis`] is the only implementor, with `Output = `[`Faces`].
///
/// It renders in rustdoc with a supertrait from a private module and no visible methods. What an implementing type
/// provides is: the result type a run of it produces, the [`Subject`](crate::Subject) it is reported as, and the
/// pipeline that runs it.
//
// The implementor is the carrier rather than `Detection` itself, which is what makes `execute`'s parameter one type
// per path rather than one type per family. The rustdoc rendering is the cost of not making the whole inference seam
// public to avoid the seal.
pub trait DataOperation: sealed::DataModel {}

impl<T: sealed::DataModel> DataOperation for T {}

// The lint fires on every method of this impl, and both spellings of the seal fire it: a sealed trait whose methods
// speak this crate's own inference vocabulary — `Backend`, `SharedData` — puts `pub(crate)` types into a trait rustc
// computes as reachable at `pub`, because `Detection` is public and the trait is a `pub` item inside a `pub(crate)`
// module. Collapsing the two traits into one moves the same warning onto `DataOperation`'s supertrait bound rather
// than removing it.
//
// Nothing is actually reachable: an external crate cannot name `sealed::DataModel`, so it can neither call these nor
// implement them. The alternative that would satisfy the lint is making the whole inference seam public, which is the
// trade the seal exists to avoid.
#[expect(
    private_bounds,
    private_interfaces,
    reason = "the seal is the point: these methods speak the crate-private `pipeline` vocabulary and are unreachable outside it"
)]
impl sealed::DataModel for Analysis {
    // One arm, so one concrete type. A second arm is what would force this to become an enum — see `Analysis`.
    /// A set of faces — the [`DetectionOutput`](super::face::DetectionOutput) of the one family this carrier holds —
    /// so a caller's `execute` result is a [`Faces`] with nothing to state and nothing to unwrap.
    type Output = Faces;

    fn as_subject(&self) -> crate::models::Subject {
        crate::models::Subject::Analysis(*self)
    }

    /// The family's model, through the family's one contract.
    fn pipeline<B: Backend>(&self) -> Result<SharedData<B, Self::Output>, InferenceError> {
        // The counterpart of `Operation::pipeline`'s upscale arm, at one remove: this carrier's arm hands to the
        // family, and the family's match names the pipeline. A second detection model is one arm in that inner match
        // and a directory of its own, and the outer match over the carrier does not change.
        match self {
            Self::Detection(detection) => match detection.variant() {
                DetectionVariant::NewYork(_) => Ok(super::detection::newyork::pipeline::<B>(*detection)),
            },
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::models::bias::Bias;
    use crate::models::color_balance::ColorBalanceVariant;
    use crate::models::colorization::ColorizationVariant;
    use crate::models::denoise::DenoiseVariant;
    use crate::models::face_recovery::{FaceRecovery, FaceRecoveryVariant};
    use crate::models::fidelity::Fidelity;
    use crate::models::light_adjustment::LightAdjustmentVariant;
    use crate::models::precision::FloatPrecision;
    use crate::models::scale::Scale;
    use crate::models::sharpen::SharpenVariant;
    use crate::models::strength::Strength;
    use crate::models::test_support::{every_analysis, every_operation};
    use crate::models::upscale::UpscaleVariant;
    use crate::models::upscale::osaka::precision::OsakaPrecision;
    use crate::pipeline::test_support::NoBackend;

    #[test]
    fn a_detection_run_produces_faces_and_the_binding_is_the_operations_own() {
        // The requirement `data-inference` states as "the result type is the operation's own, not the caller's to
        // choose", checked the only way it can be: by naming the type the trait resolves to. A caller has no second
        // place to state it, so there is nothing to state wrongly — and the `compile_fail` doctest on
        // `DataOperation` is the other half, that an operation producing an image cannot reach this path at all.
        fn faces_of<O: DataOperation<Output = Faces>>(_operation: &O) {}

        faces_of(&Detection::newyork(FloatPrecision::Fp32));

        // And the same statement as an equality the compiler checks rather than as a bound it happens to satisfy.
        // The bound is on the **carrier**, which is what makes `execute`'s parameter one type per path: a caller
        // holding an `Analysis` has a `Faces` coming back with nothing to unwrap.
        let produced: <Analysis as sealed::DataModel>::Output = Faces::empty();
        assert!(produced.is_empty());
    }

    #[test]
    fn naming_a_model_decides_which_path_it_runs_on() {
        // The requirement `data-inference` states as "naming a model decides its path". A caller names New York and
        // gets the carrier `execute` takes; it names Kyoto and gets the carrier `process` takes. There is no third
        // argument saying which, so there is nothing to answer wrongly — and the two carriers are different types,
        // so neither can be handed to the other's entry point. That half is checked by the compiler rather than
        // here: `Opai::execute` takes `&Analysis` and `Opai::process` takes `&[Operation]`.
        let analysis = Detection::newyork(FloatPrecision::Fp32);
        let enhancement = Upscale::kyoto(FloatPrecision::Fp32, Scale::new(4.0).expect("a scale in range"));

        assert_eq!(analysis, Analysis::Detection(Detection::new(DetectionVariant::NewYork(FloatPrecision::Fp32))));
        assert!(matches!(enhancement, Operation::Upscale(_)));
    }

    #[test]
    fn a_data_operation_reports_itself_as_the_subject_the_records_name_it_by() {
        // The one place the two spellings meet: a run's progress label, its log fields and its failure all speak
        // `Subject`, so the driver needs this and must get the value carrying the very operation it was handed.
        let detection = Detection::newyork(FloatPrecision::Fp16);

        assert_eq!(sealed::DataModel::as_subject(&detection), crate::models::Subject::Analysis(detection));
        assert_eq!(sealed::DataModel::as_subject(&detection).cache_tag(), detection.cache_tag());
    }

    #[test]
    fn the_analysis_carrier_reports_exactly_what_the_typed_operation_reports() {
        // The same contract `Operation`'s arms are held to: carrying an operation changes none of its answers.
        for precision in [FloatPrecision::Fp32, FloatPrecision::Fp16] {
            let typed = Detection::new(DetectionVariant::NewYork(precision));
            let carried = Analysis::Detection(typed);

            assert_eq!(carried.family(), Family::Detection);
            assert_eq!(carried.precision(), typed.precision());
            assert_eq!(carried.display_name(), typed.display_name());
            assert_eq!(carried.required_artifacts(), typed.required_artifacts());
            assert_eq!(carried.cache_tag(), typed.cache_tag());
            assert_eq!(carried.profile(), typed.profile());
        }
    }

    #[test]
    fn a_detection_row_builds_the_carrier_for_its_own_path() {
        // The catalogue's inverse, split the same way the carriers are: resolving detection's row and calling the
        // constructor it names produces an `Analysis`, which is the value `execute` takes — and there is no way for
        // it to produce an `Operation`, because `Detection::newyork` does not return one.
        let variant =
            DetectionVariant::from_codename("newyork", Precision::Fp32).expect("detection publishes New York at FP32");

        match variant {
            DetectionVariant::NewYork(precision) => {
                assert_eq!(Detection::newyork(precision), Analysis::Detection(Detection::new(variant)));
            }
        }

        // An unpublished pairing names no variant at all, so there is nothing to call a constructor on.
        assert_eq!(DetectionVariant::from_codename("newyork", Precision::Int8), None);
        assert_eq!(DetectionVariant::from_codename("helsinki", Precision::Fp32), None);
    }

    #[test]
    fn one_rio_session_serves_every_bias() {
        // The `color-balance` spec's own scenario, at the seam that would break it: the bias is applied after everything the model
        // contributed, so dragging a control through a range of values must reach one artifact under one profile.
        // A pipeline that had folded the bias into the model's identity would reload the weights per drag frame.
        let artifacts: std::collections::BTreeSet<String> = [-1.0, -0.25, 0.0, 0.5, 1.0]
            .into_iter()
            .map(|value| {
                let operation = ColorBalance::rio(FloatPrecision::Fp16, Bias::clamped(value));
                let pipeline = operation.pipeline::<NoBackend>().expect("Rio reaches a pipeline");

                crate::pipeline::Model::<NoBackend>::required(pipeline.as_ref())
                    .iter()
                    .map(|artifact| artifact.as_str().to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect();

        assert_eq!(
            artifacts,
            ["cb_rio_fp16".to_string()].into_iter().collect::<std::collections::BTreeSet<_>>(),
            "two biases of one variant would open two sets of weights"
        );
    }

    #[test]
    fn a_chain_can_carry_no_operation_that_produces_something_other_than_an_image() {
        // A chain is a `[Operation]`, and the one family whose result is not an image is carried in `Analysis`, so
        // there is nothing to put in a chain and no refusal for `plan` to report. The statement the compiler makes is `plan`'s parameter type; what is left
        // to check here is that nothing the carrier *can* hold produces anything but a picture — a family added to
        // `Operation` whose result was not an image would have to fail somewhere, and this is where.
        //
        // Asked of the whole spread rather than of one arm, so an eighth image family is covered by existing.
        for operation in every_operation() {
            assert_ne!(
                operation.family(),
                Family::Detection,
                "{operation:?} put an operation that produces no image into a chain's carrier"
            );
        }
    }

    /// The fidelity the fixtures below carry where the value is not what they are about: [`Fidelity::MAX`], the
    /// default a front end offers.
    fn fidelity() -> Fidelity {
        Fidelity::new(Fidelity::MAX).expect("the maximum is in range")
    }

    /// One operation of each of the seven families this carrier holds, paired with the family it belongs to.
    ///
    /// Seven rather than eight: detection is carried in [`Analysis`], and the tests that need all eight say so by
    /// reaching across both carriers — see `every_family_can_be_carried_by_the_carrier_for_its_own_path`.
    fn one_of_each() -> Vec<(Operation, Family)> {
        let strength = Strength::new(0.5).expect("the test supplied a strength in range");
        let bias = Bias::new(-0.5).expect("the test supplied a bias in range");
        let scale = Scale::new(2.5).expect("the test supplied a scale in range");

        vec![
            (
                Operation::Denoise(Denoise::new(DenoiseVariant::Stockholm(FloatPrecision::Fp32), strength)),
                Family::Denoise,
            ),
            (FaceRecovery::athens(FloatPrecision::Fp16, Faces::empty(), fidelity()), Family::FaceRecovery),
            (
                Operation::LightAdjustment(LightAdjustment::new(
                    LightAdjustmentVariant::Paris(FloatPrecision::Fp16),
                    bias,
                )),
                Family::LightAdjustment,
            ),
            (
                Operation::ColorBalance(ColorBalance::new(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp16), bias)),
                Family::ColorBalance,
            ),
            (
                Operation::Colorization(Colorization::new(ColorizationVariant::Delhi(FloatPrecision::Fp32))),
                Family::Colorization,
            ),
            (
                Operation::Sharpen(Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength)),
                Family::Sharpen,
            ),
            (
                Operation::Upscale(Upscale::new(UpscaleVariant::Osaka(OsakaPrecision::Int8), scale)),
                Family::Upscale,
            ),
        ]
    }

    #[test]
    fn every_family_can_be_carried_by_the_carrier_for_its_own_path() {
        for (operation, family) in one_of_each() {
            assert_eq!(operation.family(), family, "{operation:?} reported a family it does not belong to");
        }

        // Every family, and each of them once, **across both carriers**: the seven whose result is an image and the
        // one whose result is not. A list that had drifted or repeated an arm would pass the per-operation check
        // above and still leave a family uncarried.
        let mut carried: Vec<Family> = one_of_each()
            .iter()
            .map(|(operation, _)| operation.family())
            .chain(every_analysis().iter().map(Analysis::family).take(1))
            .collect();
        let mut declared = Family::ALL.to_vec();
        carried.sort_unstable_by_key(|family| format!("{family:?}"));
        declared.sort_unstable_by_key(|family| format!("{family:?}"));
        assert_eq!(carried, declared, "a family the library names cannot be carried, or was carried twice");
    }

    #[test]
    fn each_arm_reports_exactly_what_the_typed_operation_reports() {
        // The whole contract of the seam: carrying an operation changes none of its answers. Written against the
        // typed values rather than against literals, so the two cannot be edited into agreeing by accident.
        let strength = Strength::new(0.5).expect("in range");
        let bias = Bias::new(-0.5).expect("in range");
        let scale = Scale::new(2.5).expect("in range");

        let detection = Detection::new(DetectionVariant::NewYork(FloatPrecision::Fp32));
        let denoise = Denoise::new(DenoiseVariant::Stockholm(FloatPrecision::Fp32), strength);
        let recovery = match FaceRecovery::athens(FloatPrecision::Fp16, Faces::empty(), fidelity()) {
            Operation::FaceRecovery(operation) => operation,
            other => panic!("Athens built {other:?}"),
        };
        let light = LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp16), bias);
        let colour = ColorBalance::new(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp16), bias);
        let colorization = Colorization::new(ColorizationVariant::Delhi(FloatPrecision::Fp32));
        let sharpen = Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength);
        let upscale = Upscale::new(UpscaleVariant::Osaka(OsakaPrecision::Int8), scale);

        macro_rules! forwards {
            ($operation:expr, $arm:expr) => {{
                let carried = $arm;

                assert_eq!(carried.precision(), $operation.precision(), "{carried:?} forwarded another precision");
                assert_eq!(carried.display_name(), $operation.display_name(), "{carried:?} forwarded another name");
                assert_eq!(
                    carried.required_artifacts(),
                    $operation.required_artifacts(),
                    "{carried:?} forwarded other artifacts"
                );
                assert_eq!(carried.cache_tag(), $operation.cache_tag(), "{carried:?} forwarded another cache tag");
            }};
        }

        forwards!(detection, Analysis::Detection(detection));
        forwards!(denoise, Operation::Denoise(denoise));
        forwards!(recovery, Operation::FaceRecovery(recovery.clone()));
        forwards!(light, Operation::LightAdjustment(light));
        forwards!(colour, Operation::ColorBalance(colour));
        forwards!(colorization, Operation::Colorization(colorization));
        forwards!(sharpen, Operation::Sharpen(sharpen));
        forwards!(upscale, Operation::Upscale(upscale));
    }

    #[test]
    fn what_a_carried_operation_reports_is_named_for_the_family_it_reports() {
        // The cross-family property the forwarding makes checkable at all: the artifacts and the cache tag of an
        // operation are spelled with its own family's prefix, so no arm can be wired to the wrong family's value.
        for (operation, _) in one_of_each() {
            let prefix = operation.family().prefix();

            assert!(
                operation.cache_tag().starts_with(&format!("{prefix}-")),
                "{operation:?} reported the cache tag {} under another family's prefix",
                operation.cache_tag()
            );

            for artifact in operation.required_artifacts() {
                assert!(
                    artifact.as_str().starts_with(&format!("{prefix}_")),
                    "{operation:?} required {artifact}, which is named for another family"
                );
            }
        }
    }

    #[test]
    fn a_published_row_names_the_constructor_that_builds_it() {
        // The round trip, one family of each shape. The library's inverse is the family's own `from_codename`; what
        // follows it is a match over the variant it returns, calling that model's constructor with the parameters
        // the catalogue published for it.
        let variant = ColorizationVariant::from_codename("jaipur", Precision::Fp16)
            .expect("colorization publishes Jaipur at FP16");
        assert_eq!(
            Colorization::jaipur(FloatPrecision::Fp16),
            Operation::Colorization(Colorization::new(variant)),
            "the constructor and the variant named different operations"
        );

        let variant = DenoiseVariant::from_codename("malmo", Precision::Fp16).expect("denoise publishes Malmo at FP16");
        let strength = Strength::new(2.5).expect("in range");
        assert_eq!(
            Denoise::malmo(FloatPrecision::Fp16, strength),
            Operation::Denoise(Denoise::new(variant, strength))
        );

        // Osaka, whose precision contract is its own: the row publishes INT8, and the constructor takes the type
        // that can express it.
        let variant = UpscaleVariant::from_codename("osaka", Precision::Int8).expect("Osaka is published at INT8");
        let scale = Scale::new(2.5).expect("in range");
        assert_eq!(Upscale::osaka(OsakaPrecision::Int8, scale), Operation::Upscale(Upscale::new(variant, scale)));
    }

    #[test]
    fn a_carried_operation_round_trips_through_its_serialized_form() {
        let faces = Faces::new([crate::models::face::tests::face_at(12.34, 56.78, 90.12, 34.56)]);
        let mut operations: Vec<Operation> = one_of_each().into_iter().map(|(operation, _)| operation).collect();
        // The empty selection `one_of_each` carries proves nothing about a selection that is not empty, and the
        // faces are the one field of the eight that is neither a scalar nor fixed in length.
        operations.push(FaceRecovery::athens(FloatPrecision::Fp16, faces, fidelity()));

        for operation in operations {
            let json = serde_json::to_string(&operation).expect("an operation serializes");
            let back: Operation = serde_json::from_str(&json).expect("an operation deserializes");

            assert_eq!(back, operation, "{json} did not round-trip to an equal operation");
            assert_eq!(back.family(), operation.family(), "{json} lost the family it named");
            assert_eq!(back.precision(), operation.precision(), "{json} lost its precision");
            // Equality already compares the per-run parameter. The tag is checked as well because it is written out
            // by hand rather than derived, so it is what says the strength, bias, scale or selection survived the trip
            // into the key the run cache stores the image under, rather than being defaulted back in.
            assert_eq!(back.cache_tag(), operation.cache_tag(), "{json} round-tripped to a different image");
        }
    }

    #[test]
    fn the_serialized_form_is_tagged_by_the_family_and_spelled_as_the_family_spells_itself() {
        // The tag is what a front end switches on, and a second spelling of a family would be a second vocabulary.
        for (operation, family) in one_of_each() {
            let json = serde_json::to_string(&operation).expect("an operation serializes");
            let tag = serde_json::to_string(&family).expect("a family serializes");

            assert!(
                json.starts_with(&format!("{{\"family\":{tag},")) || json == format!("{{\"family\":{tag}}}"),
                "{json} is not tagged {tag}, which is how {family:?} spells itself"
            );
        }
    }

    #[test]
    fn the_serialized_form_carries_the_familys_own_fields_beside_the_tag() {
        // Pinned for one family of each shape, because a persisted operation makes this representation a
        // compatibility surface: the tag, the field names and the way a variant names itself are all things a front
        // end will have written to disk, and a derive rearranged later would invalidate them silently.
        // Colorization stands in for the parameterless shape, detection being on the other carrier — whose own
        // serialized form is pinned beside it below.
        let parameterless =
            Operation::Colorization(Colorization::new(ColorizationVariant::Delhi(FloatPrecision::Fp32)));
        assert_eq!(
            serde_json::to_string(&parameterless).expect("an operation serializes"),
            r#"{"family":"colorization","variant":{"codename":"delhi","precision":"fp32"}}"#
        );

        // The analysis carrier is tagged the same way and by the same rule, so a front end switches on one field
        // whichever path a run took.
        assert_eq!(
            serde_json::to_string(&Detection::newyork(FloatPrecision::Fp32)).expect("an analysis serializes"),
            r#"{"family":"detection","variant":{"codename":"newyork","precision":"fp32"}}"#
        );

        let scalar = Operation::Denoise(Denoise::new(
            DenoiseVariant::Stockholm(FloatPrecision::Fp32),
            Strength::new(0.5).expect("in range"),
        ));
        assert_eq!(
            serde_json::to_string(&scalar).expect("an operation serializes"),
            r#"{"family":"denoise","variant":{"codename":"stockholm","precision":"fp32"},"strength":0.5}"#
        );

        // The envelope only: what a face itself is spelled as is pinned in `face.rs`, and repeating a landmark's
        // coordinates here would pin one family's tests to another's rounding.
        let carrying = FaceRecovery::athens(
            FloatPrecision::Fp16,
            Faces::new([crate::models::face::tests::face_at(12.34, 56.78, 90.12, 34.56)]),
            fidelity(),
        );
        let json = serde_json::to_string(&carrying).expect("an operation serializes");
        assert!(
            json.starts_with(
                r#"{"family":"face_recovery","variant":{"codename":"athens","precision":"fp16"},"faces":[{"#
            ),
            "{json}"
        );

        assert_eq!(
            serde_json::to_string(&FaceRecovery::athens(FloatPrecision::Fp16, Faces::empty(), fidelity()))
                .expect("an operation serializes"),
            r#"{"family":"face_recovery","variant":{"codename":"athens","precision":"fp16"},"faces":[],"fidelity":1.0}"#
        );
    }

    #[test]
    fn a_serialized_operation_naming_a_pairing_that_was_never_published_is_refused() {
        // The guarantee the typed families give, still holding after a trip through a process boundary: the tag
        // chooses the arm, and the arm's own variant refuses what it is not published in.
        for json in [
            r#"{"family":"upscale","variant":{"codename":"osaka","precision":"fp32"},"scale":2.5}"#,
            r#"{"family":"denoise","variant":{"codename":"stockholm","precision":"int8"},"strength":0.5}"#,
            r#"{"family":"denoise","variant":{"codename":"stockholm","precision":"fp32"},"strength":4.0}"#,
            r#"{"family":"denoise","variant":{"codename":"moscow","precision":"fp32"},"strength":0.5}"#,
            r#"{"family":"helsinki","variant":{"codename":"stockholm","precision":"fp32"},"strength":0.5}"#,
        ] {
            assert!(
                serde_json::from_str::<Operation>(json).is_err(),
                "deserialization produced an operation from {json}, which names nothing published"
            );
        }
    }

    #[test]
    fn every_variant_of_every_family_answers_for_its_execution_provider_profile() {
        // Driven through the seam over all eight families, so the hook is exercised across every one of them from
        // the moment it exists rather than only on the family that happens to declare something. What each answer
        // *is* belongs to the family that measured it; what this pins is that there is an answer to ask for.
        let mut families = Vec::new();

        for operation in every_operation() {
            // Asked rather than compared: the point is that the arm forwards to the value it holds, which is what an
            // eighth image family added without an answer would fail to compile.
            let _ = operation.profile();

            if !families.contains(&operation.family()) {
                families.push(operation.family());
            }
        }

        // The other carrier, so the spread is over all eight families rather than the seven one of them holds.
        for analysis in every_analysis() {
            let _ = analysis.profile();

            if !families.contains(&analysis.family()) {
                families.push(analysis.family());
            }
        }

        assert_eq!(families.len(), 8, "the spread did not reach all eight families: {families:?}");
    }

    #[test]
    fn an_operations_profile_is_the_one_its_variant_declares() {
        // The forwarding itself, checked against the variant directly for one family of each shape — a `Copy`
        // family, the one whose per-run input is not a scalar, and the one that carries its own precision contract.
        let strength = Strength::new(1.0).expect("1.0 is in range");
        let scale = Scale::new(4.0).expect("4.0 is in range");
        let denoise_variant = DenoiseVariant::Stockholm(FloatPrecision::Fp16);
        let upscale_variant = UpscaleVariant::Kyoto(FloatPrecision::Fp16);
        let face_variant = FaceRecoveryVariant::Athens(FloatPrecision::Fp32);

        assert_eq!(Operation::Denoise(Denoise::new(denoise_variant, strength)).profile(), denoise_variant.profile());
        assert_eq!(Operation::Upscale(Upscale::new(upscale_variant, scale)).profile(), upscale_variant.profile());
        assert_eq!(
            FaceRecovery::athens(FloatPrecision::Fp32, Faces::empty(), fidelity()).profile(),
            face_variant.profile()
        );
    }

    #[test]
    fn only_the_measured_arms_declare_a_profile_across_the_whole_library() {
        // Across all eight families rather than only the ones that declare something, which is what makes this catch
        // a profile applied to the wrong arm or to a precision it was not measured at.
        let measured = [
            UpscaleVariant::Tokyo(FloatPrecision::Fp16),
            UpscaleVariant::Tokyo(FloatPrecision::Fp32),
            UpscaleVariant::Kyoto(FloatPrecision::Fp16),
            UpscaleVariant::Saitama(FloatPrecision::Fp16),
            UpscaleVariant::Osaka(OsakaPrecision::Fp16),
            UpscaleVariant::Osaka(OsakaPrecision::Int8),
        ];

        // New York is measured at **both** precisions, unlike every upscale arm below — which is the whole of what
        // makes it a traversal of its own carrier rather than an arm in the one below. See `DetectionVariant::profile`
        // for why the layout is declared at FP32 as well as FP16.
        for analysis in every_analysis() {
            assert_ne!(
                analysis.profile(),
                EpProfile::default(),
                "{} lost its measured profile",
                analysis.display_name()
            );
        }

        for operation in every_operation() {
            // Compared on the variant rather than on the operation: the scale is part of an upscale's identity but
            // not of its tuning, so every scale a variant is asked for carries the same profile. Face recovery is
            // the same point — the faces and the fidelity are part of a run's identity and not of the graph a
            // measurement was taken against, so every selection carries its variant's one profile. Both of its
            // variants are measured, and their two profiles deliberately disagree; which settings each declares
            // is pinned in the model's own directory rather than here.
            let measured_arm = match &operation {
                Operation::Upscale(upscale) => measured.contains(&upscale.variant()),
                Operation::FaceRecovery(_) => true,
                // Both variants, at FP16 **only**, which is the one family here that splits by precision on its own
                // carrier rather than inside a list of upscale arms. Every figure behind either declaration is an
                // FP16 measurement; see each model's own directory for which settings and why they disagree.
                Operation::LightAdjustment(adjustment) => adjustment.precision() == Precision::Fp16,
                // Rio at FP16 alone, and the arm is written as a variant test rather than a precision one for a
                // reason this family is the first to need: **São Paulo is unmeasured at both precisions**, so a
                // precision-only answer here would demand a profile of a graph nobody has run a sweep against.
                // Rio's own settings are the opposite of light adjustment's on the same hardware; see `rio`.
                Operation::ColorBalance(balance) => {
                    matches!(balance.variant(), ColorBalanceVariant::Rio(FloatPrecision::Fp16))
                }
                // Gothenburg and Malmö at FP16, both ported from the reference. Stockholm's arms are whatever its own
                // sweep adopted, which `stockholm` pins; they are compared against it here rather than against the
                // default, so the outcome of that measurement is the model's to state.
                Operation::Denoise(denoise) => match denoise.variant() {
                    DenoiseVariant::Stockholm(precision) => {
                        assert_eq!(
                            operation.profile(),
                            super::super::denoise::stockholm::profile(precision.into()),
                            "{} is not the profile its model declares",
                            operation.display_name()
                        );
                        continue;
                    }
                    DenoiseVariant::Gothenburg(precision) | DenoiseVariant::Malmo(precision) => {
                        precision == FloatPrecision::Fp16
                    }
                },
                // Moscow and Novgorod at FP16, both ported from the reference. Petersburg's arms are the outcome of
                // the reference's sweep, which `petersburg` pins, and are compared against it for the reason
                // Stockholm's are.
                Operation::Sharpen(sharpen) => match sharpen.variant() {
                    SharpenVariant::Petersburg(precision) => {
                        assert_eq!(
                            operation.profile(),
                            super::super::sharpen::petersburg::profile(precision.into()),
                            "{} is not the profile its model declares",
                            operation.display_name()
                        );
                        continue;
                    }
                    SharpenVariant::Moscow(precision) | SharpenVariant::Novgorod(precision) => {
                        precision == FloatPrecision::Fp16
                    }
                },
                // All three at FP16 only, ported from the reference. Delhi's is the reference's declaration beside
                // Mumbai's rather than a measurement of its own graph; see `delhi`.
                Operation::Colorization(colorization) => colorization.precision() == Precision::Fp16,
            };

            if measured_arm {
                assert_ne!(
                    operation.profile().tuning(),
                    EpProfile::default(),
                    "{} lost its measured profile",
                    operation.display_name()
                );
                continue;
            }

            assert_eq!(
                operation.profile().tuning(),
                EpProfile::default(),
                "{} declared a profile nothing measured",
                operation.display_name()
            );
        }
    }

    #[test]
    fn only_the_models_webgpu_mishandles_carry_a_webgpu_setting() {
        // The other half of what `tuning` takes out, across all eight families: the three graphs with nodes the plugin
        // gets wrong pin exactly those, at both precisions, Osaka declines the provider outright, and nothing else
        // says anything to WebGPU at all.
        for analysis in every_analysis() {
            let profile = analysis.profile();
            assert!(profile.webgpu_cpu_nodes.is_empty() && !profile.webgpu_declined, "{}", analysis.display_name());
        }

        for operation in every_operation() {
            let profile = operation.profile();
            // `cb_saopaulo_fp16`: the codename is the model tag's second field.
            let tag = operation.model_tag();
            let codename = tag.split('_').nth(1).expect("every model tag names its codename");

            let pinned = matches!(codename, "paris" | "saopaulo" | "jaipur");
            assert_eq!(
                !profile.webgpu_cpu_nodes.is_empty(),
                pinned,
                "{} pins {:?}",
                operation.display_name(),
                profile.webgpu_cpu_nodes
            );
            assert_eq!(profile.webgpu_declined, codename == "osaka", "{}", operation.display_name());
        }
    }

    #[test]
    fn a_model_tag_names_the_weights_and_never_a_run_parameter() {
        let strength = |value| Strength::new(value).expect("the test supplied a strength in range");
        let scale = |value| Scale::new(value).expect("the test supplied a scale in range");

        let gentle = Denoise::stockholm(FloatPrecision::Fp32, strength(0.2));
        let strong = Denoise::stockholm(FloatPrecision::Fp32, strength(0.9));
        assert_ne!(gentle.cache_tag(), strong.cache_tag());
        assert_eq!(gentle.model_tag(), "dn_stockholm_fp32");
        assert_eq!(strong.model_tag(), gentle.model_tag());

        // One set of Osaka weights serves every scale, so the scale is a parameter and stays out.
        let osaka = |value| Upscale::osaka(OsakaPrecision::Fp16, scale(value)).model_tag();
        assert_eq!(osaka(2.0), osaka(3.5));
        assert!(!osaka(2.0).contains('x'), "{}", osaka(2.0));

        // Kyoto's scale selects its weights, which are named by the native scale of the first pass, not the request.
        let kyoto = |value| Upscale::kyoto(FloatPrecision::Fp16, scale(value)).model_tag();
        assert_eq!(kyoto(4.0), "up_kyoto_4x_fp16");
        assert_eq!(kyoto(8.0), "up_kyoto_4x_fp16");
        assert_eq!(kyoto(1.5), "up_kyoto_2x_fp16");
    }

    #[test]
    fn every_operation_has_a_model_tag() {
        for operation in every_operation() {
            let tag = operation.model_tag();
            assert!(tag.starts_with(operation.family().prefix()), "{operation:?} is tagged {tag:?}");
        }
    }
}
