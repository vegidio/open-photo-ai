//! The light adjustment family: two variants, each one graph per precision, run at a per-run bias.

#[cfg(test)]
mod live;
pub(crate) mod lyon;
pub(crate) mod paris;
pub(crate) mod process;
pub(crate) mod variant;

use serde::{Deserialize, Serialize};

use super::artifact::{ArtifactId, Family};
use super::bias::Bias;
use super::operation::Operation;
use super::precision::{FloatPrecision, Precision};
use super::simple::single_artifact;

use crate::pipeline::Backend;
use crate::pipeline::Shared;
use crate::providers::profile::EpProfile;

pub(crate) use variant::ALL;
pub use variant::LightAdjustmentVariant;

/// One light adjustment run: which model, and which direction and how far to shift the image.
///
/// This value **is** the operation's identity, as [`super::upscale::Upscale`]'s is. Nothing here owns a native
/// resource: it is a plain `Copy` value describing *what* to run, which is what lets a front end hold and persist one
/// independently of any session built from it.
///
/// Build this; keep it as an [`Operation::LightAdjustment`] — see
/// [`Operation`] for which of the two to reach for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LightAdjustment {
    /// The model and its precision.
    variant: LightAdjustmentVariant,
    /// Which direction and how far this run shifts the image, and part of the operation's identity: two values of it
    /// are two requests.
    bias: Bias,
}

impl LightAdjustment {
    /// The operation running `variant` at `bias`.
    pub const fn new(variant: LightAdjustmentVariant, bias: Bias) -> Self {
        Self { variant, bias }
    }

    /// Paris at `precision`, shifting the image by `bias`.
    ///
    /// Returns the carrier rather than `Self`, so a caller collects operations of several families into one chain
    /// with no per-element wrapping; the typed value stays reachable by matching it. Published at FP32 and FP16.
    pub fn paris(precision: FloatPrecision, bias: Bias) -> Operation {
        Operation::LightAdjustment(Self::new(LightAdjustmentVariant::Paris(precision), bias))
    }

    /// Lyon at `precision`, shifting the image by `bias`. Published at FP32 and FP16; see [`LightAdjustment::paris`].
    pub fn lyon(precision: FloatPrecision, bias: Bias) -> Operation {
        Operation::LightAdjustment(Self::new(LightAdjustmentVariant::Lyon(precision), bias))
    }

    /// Which model this runs, and at which precision.
    pub const fn variant(self) -> LightAdjustmentVariant {
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
        self.variant.profile()
    }

    /// The name a user interface shows: `Paris (FP32)`, without the bias.
    pub fn display_name(self) -> String {
        // The bias does not select the weights, so a name fixed at whichever value happened to load them first would
        // be wrong from the second run onwards.
        self.to_string()
    }

    /// What this run needs beyond the operation itself.
    pub const fn params(self) -> LightAdjustmentParams {
        LightAdjustmentParams { bias: self.bias }
    }

    /// The pipeline this operation runs — the **family's contract seam**, where a variant becomes a run.
    pub(crate) fn pipeline<B: Backend>(self) -> Shared<B> {
        // One arm, because the two variants differ in nothing that reaches `process` — not the normalisation, not the
        // session call, not the shape of the output. No `Result`, as `FaceRecovery::pipeline` has none and for the
        // same reason: every variant this family publishes has a graph, a square and a profile, so there is no state
        // here to refuse.
        //
        // Built from what `params` publishes rather than from this type's own fields, so a pipeline cannot be
        // constructed from something the library did not say a run needs. The square comes from the variant; see
        // `LightAdjustmentVariant::canvas`.
        process::pipeline::<B>(
            self.display_name(),
            self.artifact(),
            self.profile(),
            self.params(),
            self.variant.canvas(),
        )
    }

    /// The artifact serving this operation.
    pub fn artifact(self) -> ArtifactId {
        single_artifact(Family::LightAdjustment, self.variant.codename(), self.precision())
    }

    /// The artifacts that must be on disk before this operation can run.
    ///
    /// Always exactly one: every light adjustment variant is published as a single graph.
    pub fn required_artifacts(self) -> Vec<ArtifactId> {
        // A `Vec` to match `Upscale::required_artifacts`, so that whoever downloads what an operation needs asks every
        // family the same question.
        vec![self.artifact()]
    }

    /// The key the run cache stores this operation's result under.
    ///
    /// Carries the bias, which the tag has to spell itself because it is written out by hand rather than derived from
    /// the identity: two biases are one model and two images, so a tag that left it out would serve the second run
    /// the first one's picture. A negative bias renders its sign, so it cannot share a tag with its positive
    /// counterpart — the adjustment they apply is opposite. Hyphen-separated, so it cannot be mistaken for an
    /// underscore-separated artifact name.
    pub fn cache_tag(self) -> String {
        // Written deliberately rather than derived from the serialized form, so that renaming a field cannot silently
        // invalidate every cached image. `b` rather than the `t` a strength writes, so a tag says which parameter it
        // carries instead of the reference's one shared `i=` for both.
        format!(
            "{}-{}-{}-b{}",
            Family::LightAdjustment.prefix(),
            self.variant.codename(),
            self.variant.precision(),
            self.bias
        )
    }
}

/// What one light adjustment run takes, in the shape its pipeline wants it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LightAdjustmentParams {
    // A typed value rather than a map of strings to anything, for the reason `UpscaleParams` gives.
    /// Which direction and how far this run shifts the image.
    pub bias: Bias,
}

impl std::fmt::Display for LightAdjustment {
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

    fn bias(value: f64) -> Bias {
        Bias::new(value).expect("the test supplied a bias in range")
    }

    #[test]
    fn every_variant_names_the_artifact_it_is_published_as() {
        // Transcribed from the reference implementation's `models/ids_test.go`, which pins these as literals because
        // they key the on-disk cache and name the downloaded files.
        let expected = [
            (LightAdjustmentVariant::Paris(FloatPrecision::Fp32), "la_paris_fp32"),
            (LightAdjustmentVariant::Paris(FloatPrecision::Fp16), "la_paris_fp16"),
            (LightAdjustmentVariant::Lyon(FloatPrecision::Fp32), "la_lyon_fp32"),
            (LightAdjustmentVariant::Lyon(FloatPrecision::Fp16), "la_lyon_fp16"),
        ];

        for (variant, name) in expected {
            assert_eq!(LightAdjustment::new(variant, bias(0.5)).artifact().as_str(), name);
        }
    }

    #[test]
    fn an_operation_requires_exactly_one_artifact() {
        for variant in every_variant() {
            let required = LightAdjustment::new(variant, bias(0.5)).required_artifacts();

            assert_eq!(required.len(), 1, "{variant:?} required something other than one artifact");
            assert_eq!(required[0], LightAdjustment::new(variant, bias(0.5)).artifact());
        }
    }

    #[test]
    fn the_bias_never_reaches_the_artifact_name() {
        // The reference pins this too: the intensity is a per-run parameter and must never leak into the identity or
        // the file it resolves to. A negative one is the case a `%v` in a name would spell with a `-`.
        for value in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            let operation = LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias(value));

            assert_eq!(operation.artifact().as_str(), "la_paris_fp32", "bias {value} reached the name");
        }
    }

    #[test]
    fn two_biases_of_one_variant_are_two_operations() {
        // The question a consumer holding two operations actually has: did the user change anything? A bias is
        // part of the request, so two of them are two requests.
        //
        // Nothing is reloaded by their differing. A resident session is filed under the artifact it was opened
        // from and the provider it was opened on — see `crate::sessions` — so both of these need `la_paris_fp32`
        // and the second finds the weights the first opened, however the two compare.
        let negative = LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias(-0.5));
        let positive = LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias(1.0));

        assert_ne!(negative, positive, "two biases of one variant read as one request");
        assert_ne!(hash_of(&negative), hash_of(&positive), "two different operations hashed alike");

        // And the other half of the property: one request built twice is one operation.
        assert_eq!(negative, LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias(-0.5)));
    }

    #[test]
    fn an_operation_is_usable_as_a_lookup_key_directly() {
        // Keyed on the whole request, so a differing bias is a differing key — which is what a consumer asking
        // "have I seen this request before?" wants. What must not reload is the *weights*, and that is decided by
        // `required_artifacts` rather than by this comparison; the two operations below name the same artifact.
        let mut seen = std::collections::HashMap::new();
        let one = LightAdjustment::new(LightAdjustmentVariant::Lyon(FloatPrecision::Fp16), bias(0.5));
        let other = LightAdjustment::new(LightAdjustmentVariant::Lyon(FloatPrecision::Fp16), bias(-1.0));

        seen.insert(one, "requested");

        assert_eq!(seen.get(&one), Some(&"requested"), "one request did not find itself");
        assert_eq!(seen.get(&other), None, "a different bias found another request's entry");
        assert_eq!(one.required_artifacts(), other.required_artifacts(), "the two would load different weights");
    }

    #[test]
    fn two_precisions_are_different_operations() {
        let fp32 = LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias(0.5));
        let fp16 = LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp16), bias(0.5));

        assert_ne!(fp32, fp16, "two precisions served by different artifacts compared as one model");
    }

    #[test]
    fn two_variants_are_never_the_same_operation() {
        let paris = LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias(0.5));
        let lyon = LightAdjustment::new(LightAdjustmentVariant::Lyon(FloatPrecision::Fp32), bias(0.5));

        assert_ne!(paris, lyon);
    }

    #[test]
    fn a_negative_bias_reports_a_different_cache_tag_from_its_positive_counterpart() {
        // The two apply the adjustment in opposite directions, so they are two images — and they name one set of
        // weights, which is what makes the tag rather than the artifact the thing that has to tell them apart.
        for value in [0.001, 0.35, 0.5, 1.0] {
            let brighter = LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias(value));
            let darker = LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias(-value));

            assert_eq!(
                brighter.required_artifacts(),
                darker.required_artifacts(),
                "the premise is that one graph serves both"
            );
            assert_ne!(brighter.cache_tag(), darker.cache_tag(), "a bias of ±{value} shared one cache key");
        }
    }

    #[test]
    fn the_cache_tag_is_the_written_one_rather_than_whatever_serialization_produces() {
        // Pinned here so that renaming a field is a failing test rather than a silent invalidation of every cached
        // image on every user's disk.
        assert_eq!(
            LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias(0.5)).cache_tag(),
            "la-paris-fp32-b0.5"
        );
        assert_eq!(
            LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias(-0.5)).cache_tag(),
            "la-paris-fp32-b-0.5"
        );
        assert_eq!(
            LightAdjustment::new(LightAdjustmentVariant::Lyon(FloatPrecision::Fp16), bias(0.0)).cache_tag(),
            "la-lyon-fp16-b0"
        );
    }

    #[test]
    fn a_repeated_operation_reports_a_stable_tag() {
        let operation = LightAdjustment::new(LightAdjustmentVariant::Lyon(FloatPrecision::Fp32), bias(-0.35));

        assert_eq!(operation.cache_tag(), operation.cache_tag());
        assert_eq!(
            operation.cache_tag(),
            LightAdjustment::new(LightAdjustmentVariant::Lyon(FloatPrecision::Fp32), bias(-0.35)).cache_tag()
        );
    }

    #[test]
    fn no_two_operations_producing_different_images_share_one_tag() {
        let mut seen = std::collections::HashMap::new();

        for variant in every_variant() {
            for value in [-1.0, -0.5, -0.001, 0.0, 0.001, 0.5, 1.0] {
                let operation = LightAdjustment::new(variant, bias(value));

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
                let tag = LightAdjustment::new(variant, bias(value)).cache_tag();

                assert!(!tag.contains('_'), "{tag} is spelled the way an artifact name is");
            }
        }
    }

    #[test]
    fn a_display_name_is_the_label_and_the_precision_and_carries_no_bias() {
        for value in [-1.0, 0.0, 0.5] {
            let operation = LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias(value));

            assert_eq!(operation.display_name(), "Paris (FP32)");
            assert_eq!(operation.to_string(), operation.display_name());
        }

        assert_eq!(
            LightAdjustment::new(LightAdjustmentVariant::Lyon(FloatPrecision::Fp16), bias(0.5)).display_name(),
            "Lyon (FP16)"
        );
    }

    #[test]
    fn a_pipeline_built_for_each_variant_names_that_variants_one_artifact_and_one_session() {
        // The family's seam, checked for every variant rather than one: both reach the same contract, so a seam
        // that had regressed to naming one model would still hand back a working pipeline for the other. Asked
        // through `Model`, which is the whole of what the chain driver asks a pipeline before it runs.
        use crate::pipeline::Model;
        use crate::pipeline::test_support::NoBackend;

        for variant in every_variant() {
            let operation = LightAdjustment::new(variant, bias(0.5));
            let pipeline = operation.pipeline::<NoBackend>();

            assert_eq!(
                Model::<NoBackend>::required(pipeline.as_ref()),
                [operation.artifact()],
                "{variant:?} asked for something other than its own one artifact"
            );

            let sessions = Model::<NoBackend>::sessions(pipeline.as_ref());
            assert_eq!(sessions.len(), 1, "{variant:?} asked for {} sessions", sessions.len());
            assert_eq!(*sessions[0].0, operation.artifact(), "{variant:?} paired the wrong artifact");
            assert_eq!(*sessions[0].1, operation.profile(), "{variant:?} was opened under another graph's profile");
        }
    }

    #[test]
    fn one_session_serves_every_bias_of_one_variant() {
        // The property that makes dragging a control free of installs and opens: the bias is applied after the graph,
        // so every position of a slider asks for the same artifact under the same settings. The identity half of this
        // is pinned above; this is the pipeline half.
        use crate::pipeline::Model;
        use crate::pipeline::test_support::NoBackend;

        let variant = LightAdjustmentVariant::Paris(FloatPrecision::Fp16);
        let first = LightAdjustment::new(variant, bias(-1.0)).pipeline::<NoBackend>();
        let second = LightAdjustment::new(variant, bias(1.0)).pipeline::<NoBackend>();

        assert_eq!(Model::<NoBackend>::required(first.as_ref()), Model::<NoBackend>::required(second.as_ref()));
    }

    #[test]
    fn a_run_is_handed_the_bias_the_identity_does_not_carry() {
        let operation = LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias(-0.35));

        assert_eq!(operation.params(), LightAdjustmentParams { bias: bias(-0.35) });
    }

    #[test]
    fn an_operation_round_trips_through_its_serialized_form() {
        for variant in every_variant() {
            for value in [-1.0, -0.35, 0.0, 0.5, 1.0] {
                let operation = LightAdjustment::new(variant, bias(value));
                let json = serde_json::to_string(&operation).expect("an operation serializes");
                let back: LightAdjustment = serde_json::from_str(&json).expect("an operation deserializes");

                assert_eq!(back, operation, "{json} did not round-trip to an equal operation");
                assert_eq!(back.variant(), operation.variant(), "{json} lost its variant or precision");
                assert_eq!(back.bias(), operation.bias(), "{json} lost its bias");
                assert_eq!(back.cache_tag(), operation.cache_tag(), "{json} round-tripped to a different image");
            }
        }
    }

    #[test]
    fn the_serialized_form_names_its_variant_rather_than_positioning_it() {
        // Tagged by name so that adding a family or a variant later cannot invalidate anything already persisted.
        let operation = LightAdjustment::new(LightAdjustmentVariant::Paris(FloatPrecision::Fp32), bias(-0.5));
        let json = serde_json::to_string(&operation).expect("an operation serializes");

        assert_eq!(json, r#"{"variant":{"codename":"paris","precision":"fp32"},"bias":-0.5}"#);
    }

    #[test]
    fn a_serialized_operation_pairing_a_variant_with_int8_is_refused() {
        let json = r#"{"variant":{"codename":"paris","precision":"int8"},"bias":0.5}"#;

        assert!(
            serde_json::from_str::<LightAdjustment>(json).is_err(),
            "deserialization produced a light adjustment at a precision it is not published in"
        );
    }

    #[test]
    fn a_serialized_operation_carrying_an_out_of_range_bias_is_refused() {
        for json in [
            r#"{"variant":{"codename":"paris","precision":"fp32"},"bias":2.0}"#,
            r#"{"variant":{"codename":"paris","precision":"fp32"},"bias":-2.0}"#,
        ] {
            assert!(
                serde_json::from_str::<LightAdjustment>(json).is_err(),
                "deserialization produced an operation whose bias could not have been constructed"
            );
        }
    }
}
