//! The colour balance family: two variants, each one graph per precision, run at a per-run bias.

#[cfg(test)]
mod live;
pub(crate) mod mapping;
pub(crate) mod rio;
pub(crate) mod samples;
pub(crate) mod saopaulo;
pub(crate) mod variant;

use serde::{Deserialize, Serialize};

use super::artifact::{ArtifactId, Family};
use super::bias::Bias;
use super::operation::Operation;
use super::precision::{FloatPrecision, Precision};
use super::simple::single_artifact;

use crate::error::InferenceError;
use crate::pipeline::Backend;
use crate::pipeline::Shared;
use crate::providers::profile::EpProfile;

pub(crate) use variant::ALL;
pub use variant::ColorBalanceVariant;

/// One colour balance run: which model, and which direction and how far to shift the image.
///
/// This value **is** the operation's identity, as [`super::upscale::Upscale`]'s is. Nothing here owns a native
/// resource: it is a plain `Copy` value describing *what* to run, which is what lets a front end hold and persist one
/// independently of any session built from it.
///
/// # Which of the two to reach for
///
/// This type is the one to **build**: its constructor takes what a colour balance run takes and nothing else, which is
/// what makes a wrong pairing unwritable rather than refused. [`Operation`] — which
/// carries this as [`Operation::ColorBalance`] — is the one to **keep**:
/// stored, queued, serialized across a process boundary, or matched on to dispatch, by a holder that need not know
/// which of the eight families it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ColorBalance {
    /// The model and its precision.
    variant: ColorBalanceVariant,
    /// Which direction and how far this run shifts the image, and part of the operation's identity: two values of it
    /// are two requests.
    bias: Bias,
}

impl ColorBalance {
    /// The operation running `variant` at `bias`.
    pub const fn new(variant: ColorBalanceVariant, bias: Bias) -> Self {
        Self { variant, bias }
    }

    /// Rio at `precision`, shifting the image by `bias`. Published at FP32 and FP16.
    ///
    /// Returns the [`Operation`] carrier rather than `Self`, so a caller collects operations of several families into
    /// one chain with no per-element wrapping; the typed value stays reachable by matching the carrier.
    pub fn rio(precision: FloatPrecision, bias: Bias) -> Operation {
        Operation::ColorBalance(Self::new(ColorBalanceVariant::Rio(precision), bias))
    }

    /// São Paulo at `precision`, shifting the image by `bias`. Published at FP32 and FP16; see [`ColorBalance::rio`].
    pub fn saopaulo(precision: FloatPrecision, bias: Bias) -> Operation {
        Operation::ColorBalance(Self::new(ColorBalanceVariant::SaoPaulo(precision), bias))
    }

    /// Which model this runs, and at which precision.
    pub const fn variant(self) -> ColorBalanceVariant {
        self.variant
    }

    /// Which direction and how far this run shifts the image, at the value it was quantized to.
    pub const fn bias(self) -> Bias {
        self.bias
    }

    /// The precision this runs at, widened to the common currency.
    pub fn precision(self) -> Precision {
        self.variant.precision()
    }

    /// The execution-provider tuning measured for the model this runs, at the precision it runs at.
    pub(crate) fn profile(self) -> EpProfile {
        // Asked of the variant, because the variant is what names the graph a measurement was made against — and the
        // precision it was measured at travels inside it.
        self.variant.profile()
    }

    /// The name a user interface shows: `Rio (FP32)`, `São Paulo (FP16)`. The bias is not in it.
    pub fn display_name(self) -> String {
        // No bias: it does not select the weights, so a name fixed at whichever value happened to load them first
        // would be wrong from the second run onwards.
        self.to_string()
    }

    /// What this run needs beyond the operation itself.
    pub const fn params(self) -> ColorBalanceParams {
        ColorBalanceParams { bias: self.bias }
    }

    /// The pipeline this operation runs — the **family's contract seam**, where a variant becomes a run.
    ///
    /// # Errors
    ///
    /// **None today.** Both arms serve.
    pub(crate) fn pipeline<B: Backend>(self) -> Result<Shared<B>, InferenceError> {
        // **Two arms, because this family has two contracts**, which is what separates it from light adjustment's one.
        // Rio's graph returns one corrected rendering and its pipeline fits a single global mapping from it; São
        // Paulo's returns three weight planes and two synthesized renderings, so it is a second contract within this
        // family rather than a second set of weights for this one. Serving either through the other's pipeline would
        // render nonsense from correctly loaded weights, which is why each arm names its own model's file.
        //
        // Both arms take the same five arguments, and the difference between them is the function named — which is
        // the whole of what a contract seam is. Written out rather than folded behind a function pointer so that
        // the model each variant reaches is a name a reader follows rather than a value.
        //
        // Built from what `params` publishes rather than from this type's own fields, so a pipeline cannot be
        // constructed from something the library did not say a run needs. The square is the variant's: see
        // `ColorBalanceVariant::canvas`.
        let built = match self.variant {
            ColorBalanceVariant::Rio(_) => rio::process::pipeline::<B>(
                self.display_name(),
                self.artifact(),
                self.profile(),
                self.params(),
                self.variant.canvas(),
            ),
            ColorBalanceVariant::SaoPaulo(_) => saopaulo::process::pipeline::<B>(
                self.display_name(),
                self.artifact(),
                self.profile(),
                self.params(),
                self.variant.canvas(),
            ),
        };

        // A `Result` with nothing to refuse, kept rather than narrowed: a seam that cannot refuse is the exception here
        // rather than the rule, and re-widening a signature later is a worse trade than carrying an `Ok` for both arms.
        // `LightAdjustment::pipeline` and `FaceRecovery::pipeline` cannot fail because their families are served whole;
        // three of the families still to come will need theirs to refuse as they land one variant at a time.
        Ok(built)
    }

    /// The artifact serving this operation.
    pub fn artifact(self) -> ArtifactId {
        single_artifact(Family::ColorBalance, self.variant.codename(), self.precision())
    }

    /// The artifacts that must be on disk before this operation can run.
    ///
    /// Always exactly one: every colour balance variant is published as a single graph.
    pub fn required_artifacts(self) -> Vec<ArtifactId> {
        // A `Vec` for the shape `Upscale::required_artifacts` has, so that whoever downloads what an operation needs
        // asks every family the same question.
        vec![self.artifact()]
    }

    /// The key the run cache stores this operation's result under.
    ///
    /// Carries the bias, which the tag has to spell itself because it is written out by hand rather than derived from
    /// the identity: two biases are one model and two images, so a tag that left it out would serve the second run
    /// the first one's picture. A negative bias renders its sign, so it cannot share a tag with its positive
    /// counterpart — the adjustment they apply is opposite.
    pub fn cache_tag(self) -> String {
        // `b` rather than the `t` a strength writes, so a tag says which parameter it carries instead of the
        // reference's one shared `i=` for both. Written deliberately rather than derived from the serialized form, so
        // that renaming a field cannot silently invalidate every cached image, and hyphen-separated so it cannot be
        // mistaken for an underscore-separated artifact name.
        format!(
            "{}-{}-{}-b{}",
            Family::ColorBalance.prefix(),
            self.variant.codename(),
            self.variant.precision(),
            self.bias
        )
    }
}

// A typed value rather than a map of strings to anything. The reference implementation passes these as
// `map[string]any` under stringly-typed keys and substitutes a default on a missing key or a wrong type, so a typo
// there is a wrong image rather than an error; here a parameter that is not carried cannot be read.
/// What one colour balance run takes, in the shape its pipeline wants it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorBalanceParams {
    /// Which direction and how far this run shifts the image.
    pub bias: Bias,
}

impl std::fmt::Display for ColorBalance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.variant.label(), self.variant.precision().label())
    }
}

#[cfg(test)]
mod tests {
    use super::variant::tests::every_variant;
    use super::*;
    use crate::models::precision::FloatPrecision;
    use crate::models::test_support::hash_of;
    use crate::pipeline::Model;
    use crate::pipeline::test_support::NoBackend;

    fn bias(value: f64) -> Bias {
        Bias::new(value).expect("the test supplied a bias in range")
    }

    #[test]
    fn both_variants_at_both_precisions_reach_a_pipeline_naming_their_own_artifact_and_profile() {
        // The family's contract seam, over every pairing it publishes. Two contracts behind it, and what a caller
        // sees is one question answered for both: a caller asks for either without knowing which contract it runs.
        //
        // The artifact and the profile are asserted together because they are the two things the arm carries
        // across, and each is silent when it is wrong: a pipeline built with the other variant's artifact loads the
        // wrong weights and renders a photograph, and one built with the other variant's profile renders the right
        // photograph on the wrong settings.
        for variant in every_variant() {
            let operation = ColorBalance::new(variant, bias(0.5));
            let built = operation.pipeline::<NoBackend>().unwrap_or_else(|error| {
                panic!("{} reached no pipeline: {error}", operation.display_name());
            });

            assert_eq!(
                Model::<NoBackend>::required(built.as_ref()).iter().map(ArtifactId::as_str).collect::<Vec<_>>(),
                vec![operation.artifact().as_str()],
                "{variant:?} asked for something other than its own one graph"
            );

            let sessions = Model::<NoBackend>::sessions(built.as_ref());
            assert_eq!(sessions.len(), 1, "{variant:?} asked for {} sessions", sessions.len());
            assert_eq!(sessions[0].0, &operation.artifact(), "{variant:?} would open another variant's weights");
            assert_eq!(*sessions[0].1, variant.profile(), "{variant:?} would open under another model's tuning");
        }
    }

    #[test]
    fn no_colour_balance_operation_is_refused() {
        // Over the whole family: every variant, at every precision, at every position of the control. The seam
        // returns a `Result` (see `ColorBalance::pipeline` for why), so "nothing is refused" is a property to assert
        // rather than something the signature says.
        for variant in every_variant() {
            for value in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                let operation = ColorBalance::new(variant, bias(value));

                assert!(
                    operation.pipeline::<NoBackend>().is_ok(),
                    "{} at a bias of {value} was refused",
                    operation.display_name()
                );
            }
        }
    }

    #[test]
    fn one_session_serves_every_bias_of_either_variant() {
        // The spec's own scenario, at the seam that would break it: the bias is applied after everything the model
        // contributed, so dragging a control through a range of values must reach one artifact. A pipeline that had
        // folded the bias into the model's identity would reload the weights per drag frame.
        //
        // Asked of **both** variants, because São Paulo's contract is the second one built against this rule and
        // the graph it opens is five times the size of Rio's.
        for variant in every_variant() {
            let artifacts: std::collections::BTreeSet<String> = [-1.0, -0.25, 0.0, 0.5, 1.0]
                .into_iter()
                .map(|value| {
                    let operation = ColorBalance::new(variant, bias(value));
                    let built = operation.pipeline::<NoBackend>().expect("both variants reach a pipeline");

                    Model::<NoBackend>::required(built.as_ref())
                        .iter()
                        .map(|artifact| artifact.as_str().to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .collect();

            assert_eq!(
                artifacts,
                [ColorBalance::new(variant, bias(0.0)).artifact().as_str().to_string()]
                    .into_iter()
                    .collect::<std::collections::BTreeSet<_>>(),
                "two biases of {variant:?} would open two sets of weights"
            );
        }
    }

    #[test]
    fn every_variant_names_the_artifact_it_is_published_as() {
        // Transcribed from the reference implementation's `models/ids_test.go`, which pins these as literals because
        // they key the on-disk cache and name the downloaded files.
        let expected = [
            (ColorBalanceVariant::Rio(FloatPrecision::Fp32), "cb_rio_fp32"),
            (ColorBalanceVariant::Rio(FloatPrecision::Fp16), "cb_rio_fp16"),
            (ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp32), "cb_saopaulo_fp32"),
            (ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp16), "cb_saopaulo_fp16"),
        ];

        for (variant, name) in expected {
            assert_eq!(ColorBalance::new(variant, bias(0.5)).artifact().as_str(), name);
        }
    }

    #[test]
    fn sao_paulos_codename_and_label_are_held_apart() {
        // The one variant where the two differ by more than capitalisation, and the one place an accent could reach a
        // file name, a download URL or a cache directory if the wrong one were composed from.
        let operation = ColorBalance::new(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp16), bias(0.5));

        assert_eq!(operation.display_name(), "São Paulo (FP16)");
        assert_eq!(operation.artifact().as_str(), "cb_saopaulo_fp16");
        assert!(operation.artifact().as_str().is_ascii(), "an accent reached the artifact name");
        assert!(
            !operation.cache_tag().contains('ã'),
            "an accent reached the cache tag: {}",
            operation.cache_tag()
        );
    }

    #[test]
    fn an_operation_requires_exactly_one_artifact() {
        for variant in every_variant() {
            let required = ColorBalance::new(variant, bias(0.5)).required_artifacts();

            assert_eq!(required.len(), 1, "{variant:?} required something other than one artifact");
            assert_eq!(required[0], ColorBalance::new(variant, bias(0.5)).artifact());
        }
    }

    #[test]
    fn the_bias_never_reaches_the_artifact_name() {
        for value in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            let operation = ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp32), bias(value));

            assert_eq!(operation.artifact().as_str(), "cb_rio_fp32", "bias {value} reached the name");
        }
    }

    #[test]
    fn two_biases_of_one_variant_are_two_operations() {
        // The question a consumer holding two operations actually has: did the user change anything? A bias is
        // part of the request, so two of them are two requests. What must not reload is the weights, and that is
        // decided by the artifact a run names — both of these name `cb_rio_fp16` — rather than by this comparison.
        let negative = ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp16), bias(-0.5));
        let positive = ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp16), bias(1.0));

        assert_ne!(negative, positive, "two biases of one variant read as one request");
        assert_ne!(hash_of(&negative), hash_of(&positive), "two different operations hashed alike");

        // And the other half of the property: one request built twice is one operation.
        assert_eq!(negative, ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp16), bias(-0.5)));
    }

    #[test]
    fn an_operation_is_usable_as_a_lookup_key_directly() {
        // Keyed on the whole request, so a differing bias is a differing key — which is what a consumer asking
        // "have I seen this request before?" wants. What must not reload is the *weights*, and that is decided by
        // `required_artifacts` rather than by this comparison; the two operations below name the same artifact.
        let mut seen = std::collections::HashMap::new();
        let one = ColorBalance::new(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp32), bias(0.5));
        let other = ColorBalance::new(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp32), bias(-1.0));

        seen.insert(one, "requested");

        assert_eq!(seen.get(&one), Some(&"requested"), "one request did not find itself");
        assert_eq!(seen.get(&other), None, "a different bias found another request's entry");
        assert_eq!(one.required_artifacts(), other.required_artifacts(), "the two would load different weights");
    }

    #[test]
    fn two_precisions_are_different_operations() {
        let fp32 = ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp32), bias(0.5));
        let fp16 = ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp16), bias(0.5));

        assert_ne!(fp32, fp16, "two precisions served by different artifacts compared as one model");
    }

    #[test]
    fn two_variants_are_never_the_same_operation() {
        let rio = ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp32), bias(0.5));
        let sao_paulo = ColorBalance::new(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp32), bias(0.5));

        assert_ne!(rio, sao_paulo);
    }

    #[test]
    fn a_negative_bias_reports_a_different_cache_tag_from_its_positive_counterpart() {
        for value in [0.001, 0.5, 1.0] {
            let warmer = ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp16), bias(value));
            let cooler = ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp16), bias(-value));

            assert_eq!(warmer.required_artifacts(), cooler.required_artifacts(), "one graph serves both");
            assert_ne!(warmer.cache_tag(), cooler.cache_tag(), "a bias of ±{value} shared one cache key");
        }
    }

    #[test]
    fn the_cache_tag_is_the_written_one_rather_than_whatever_serialization_produces() {
        assert_eq!(
            ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp16), bias(-0.5)).cache_tag(),
            "cb-rio-fp16-b-0.5"
        );
        assert_eq!(
            ColorBalance::new(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp32), bias(0.5)).cache_tag(),
            "cb-saopaulo-fp32-b0.5"
        );
    }

    #[test]
    fn a_repeated_operation_reports_a_stable_tag() {
        // What lets a second run of the same operation be served from cache rather than recomputed.
        let operation = ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp16), bias(0.5));

        assert_eq!(operation.cache_tag(), operation.cache_tag());
        assert_eq!(
            operation.cache_tag(),
            ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp16), bias(0.5)).cache_tag()
        );
    }

    #[test]
    fn no_two_operations_producing_different_images_share_one_tag() {
        let mut seen = std::collections::HashMap::new();

        for variant in every_variant() {
            for value in [-1.0, -0.5, -0.001, 0.0, 0.001, 0.5, 1.0] {
                let operation = ColorBalance::new(variant, bias(value));

                if let Some(previous) = seen.insert(operation.cache_tag(), operation) {
                    panic!("{previous:?} and {operation:?} share the cache tag {}", operation.cache_tag());
                }
            }
        }
    }

    #[test]
    fn a_cache_tag_can_never_be_mistaken_for_an_artifact_name() {
        for variant in every_variant() {
            for value in [-1.0, 0.0, 1.0] {
                let tag = ColorBalance::new(variant, bias(value)).cache_tag();

                assert!(!tag.contains('_'), "{tag} is spelled the way an artifact name is");
            }
        }
    }

    #[test]
    fn a_display_name_is_the_label_and_the_precision_and_carries_no_bias() {
        for value in [-1.0, 0.0, 0.5] {
            let operation = ColorBalance::new(ColorBalanceVariant::Rio(FloatPrecision::Fp32), bias(value));

            assert_eq!(operation.display_name(), "Rio (FP32)");
            assert_eq!(operation.to_string(), operation.display_name());
        }
    }

    #[test]
    fn a_run_is_handed_the_bias_the_identity_does_not_carry() {
        let operation = ColorBalance::new(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp16), bias(-0.35));

        assert_eq!(operation.params(), ColorBalanceParams { bias: bias(-0.35) });
    }

    #[test]
    fn an_operation_round_trips_through_its_serialized_form() {
        for variant in every_variant() {
            for value in [-1.0, -0.35, 0.0, 0.5, 1.0] {
                let operation = ColorBalance::new(variant, bias(value));
                let json = serde_json::to_string(&operation).expect("an operation serializes");
                let back: ColorBalance = serde_json::from_str(&json).expect("an operation deserializes");

                assert_eq!(back, operation, "{json} did not round-trip to an equal operation");
                assert_eq!(back.variant(), operation.variant(), "{json} lost its variant or precision");
                assert_eq!(back.bias(), operation.bias(), "{json} lost its bias");
                assert_eq!(back.cache_tag(), operation.cache_tag(), "{json} round-tripped to a different image");
            }
        }
    }

    #[test]
    fn the_serialized_form_names_its_variant_by_its_codename() {
        // Tagged by name so that adding a family or a variant later cannot invalidate anything already persisted —
        // and by the codename, so nothing persisted carries an accent.
        let operation = ColorBalance::new(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp16), bias(0.5));
        let json = serde_json::to_string(&operation).expect("an operation serializes");

        assert_eq!(json, r#"{"variant":{"codename":"saopaulo","precision":"fp16"},"bias":0.5}"#);
    }

    #[test]
    fn a_serialized_operation_pairing_a_variant_with_int8_is_refused() {
        let json = r#"{"variant":{"codename":"rio","precision":"int8"},"bias":0.5}"#;

        assert!(
            serde_json::from_str::<ColorBalance>(json).is_err(),
            "deserialization produced a colour balance at a precision it is not published in"
        );
    }

    #[test]
    fn a_serialized_operation_carrying_an_out_of_range_bias_is_refused() {
        for json in [
            r#"{"variant":{"codename":"rio","precision":"fp32"},"bias":2.0}"#,
            r#"{"variant":{"codename":"rio","precision":"fp32"},"bias":-2.0}"#,
        ] {
            assert!(
                serde_json::from_str::<ColorBalance>(json).is_err(),
                "deserialization produced an operation whose bias could not have been constructed"
            );
        }
    }
}
