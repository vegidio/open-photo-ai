//! The sharpen family: three variants, each one graph per precision, run at a per-run strength.

#[cfg(test)]
mod live;
pub(crate) mod moscow;
pub(crate) mod novgorod;
pub(crate) mod petersburg;
pub(crate) mod variant;

use serde::{Deserialize, Serialize};

use super::artifact::{ArtifactId, Family};
use super::operation::Operation;
use super::precision::{FloatPrecision, Precision};
use super::simple::single_artifact;
use super::strength::Strength;

use crate::models::filter;
use crate::pipeline::Backend;
use crate::pipeline::Shared;
use crate::providers::profile::EpProfile;
use imaging::tensor::Normalisation;

pub(crate) use variant::ALL;
pub use variant::SharpenVariant;

// `[0, 1]` is the reference's `standardize: false`, stated by this family for itself rather than borrowed from
// denoise's: the two agree because each of the reference's `process.go` files says so, not because one follows the
// other. Pinned as a literal and asserted below, because feeding a graph the wrong range is not an error a runtime
// reports: it is a worse photograph.
/// The range this family's graphs read and write: `[0, 1]`.
const RANGE: Normalisation = Normalisation::Unit;

/// One sharpen run: which model, and how much of its output to apply.
///
/// This value **is** the operation's identity, as [`super::upscale::Upscale`]'s is. Nothing here owns a native
/// resource: it is a plain `Copy` value describing *what* to run, which is what lets a front end hold and persist one
/// independently of any session built from it.
///
/// Build this; keep it as an [`Operation::Sharpen`] — see
/// [`Operation`] for which of the two to reach for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Sharpen {
    /// The model and its precision.
    variant: SharpenVariant,
    /// How much of the model's output this run applies, and part of the operation's identity: two values of it are
    /// two requests.
    strength: Strength,
}

impl Sharpen {
    /// The operation running `variant` at `strength`.
    pub const fn new(variant: SharpenVariant, strength: Strength) -> Self {
        Self { variant, strength }
    }

    /// Moscow at `precision`, applying `strength` of its output.
    ///
    /// Returns the carrier rather than `Self`, so a caller collects operations of several families into one chain
    /// with no per-element wrapping; the typed value stays reachable by matching it. Published at FP32 and FP16.
    pub fn moscow(precision: FloatPrecision, strength: Strength) -> Operation {
        Operation::Sharpen(Self::new(SharpenVariant::Moscow(precision), strength))
    }

    /// Petersburg at `precision`, applying `strength` of its output. Published at FP32 and FP16; see [`Sharpen::moscow`].
    pub fn stpetersburg(precision: FloatPrecision, strength: Strength) -> Operation {
        Operation::Sharpen(Self::new(SharpenVariant::Petersburg(precision), strength))
    }

    /// Novgorod at `precision`, applying `strength` of its output. Published at FP32 and FP16; see [`Sharpen::moscow`].
    pub fn novgorod(precision: FloatPrecision, strength: Strength) -> Operation {
        Operation::Sharpen(Self::new(SharpenVariant::Novgorod(precision), strength))
    }

    /// Which model this runs, and at which precision.
    pub const fn variant(self) -> SharpenVariant {
        self.variant
    }

    /// How much of the model's output this run applies, at the value it was quantized to.
    pub const fn strength(self) -> Strength {
        self.strength
    }

    /// The precision this runs at, widened to the common currency.
    pub fn precision(self) -> Precision {
        self.variant.precision()
    }

    /// The execution-provider tuning measured for the model this runs, at the precision it runs at.
    pub(crate) fn profile(self) -> EpProfile {
        self.variant.profile()
    }

    /// The name a user interface shows: `Moscow (FP32)`, without the strength.
    pub fn display_name(self) -> String {
        // The strength does not select the weights, so a name fixed at whichever value happened to load them first
        // would be wrong from the second run onwards.
        self.to_string()
    }

    /// What this run needs beyond the operation itself.
    pub const fn params(self) -> SharpenParams {
        SharpenParams { strength: self.strength }
    }

    /// The pipeline this operation runs — the **family's contract seam**, where a variant becomes a run.
    pub(crate) fn pipeline<B: Backend>(self) -> Shared<B> {
        // One arm, because the three variants differ in nothing that reaches `filter` but their weights, their guard
        // and their profile — not the tiling, not the normalisation, not what the strength means. No `Result`: there
        // is no state here to refuse.
        //
        // Built from what `params` publishes rather than from this type's own fields, so a pipeline cannot be
        // constructed from something the library did not say a run needs. The guard comes from the variant; see
        // `SharpenVariant::guard`.
        filter::pipeline::<B>(
            self.display_name(),
            self.artifact(),
            self.profile(),
            RANGE,
            self.params().strength,
            self.variant.guard(),
        )
    }

    /// The artifact serving this operation.
    pub fn artifact(self) -> ArtifactId {
        single_artifact(Family::Sharpen, self.variant.codename(), self.precision())
    }

    /// The artifacts that must be on disk before this operation can run.
    ///
    /// Always exactly one: every sharpen variant is published as a single graph.
    pub fn required_artifacts(self) -> Vec<ArtifactId> {
        // A `Vec` to match `Upscale::required_artifacts`, so that whoever downloads what an operation needs asks every
        // family the same question.
        vec![self.artifact()]
    }

    /// The key the run cache stores this operation's result under.
    ///
    /// Carries the strength, which the tag has to spell itself because it is written out by hand rather than derived
    /// from the identity: two strengths are one model and two images, so a tag that left it out would serve the
    /// second run the first one's picture. Hyphen-separated, so it cannot be mistaken for an underscore-separated
    /// artifact name.
    pub fn cache_tag(self) -> String {
        // Written deliberately rather than derived from the serialized form, so that renaming a field cannot silently
        // invalidate every cached image.
        format!(
            "{}-{}-{}-t{}",
            Family::Sharpen.prefix(),
            self.variant.codename(),
            self.variant.precision(),
            self.strength
        )
    }
}

/// What one sharpen run takes, in the shape its pipeline wants it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharpenParams {
    // A typed value rather than a map of strings to anything, for the reason `UpscaleParams` gives.
    /// How much of the model's output this run applies.
    pub strength: Strength,
}

impl std::fmt::Display for Sharpen {
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

    fn strength(value: f64) -> Strength {
        Strength::new(value).expect("the test supplied a strength in range")
    }

    #[test]
    fn the_graph_is_fed_the_unit_range_rather_than_the_signed_one() {
        // The literal, pinned rather than left to be read off a call, for the reason `RANGE` gives.
        assert_eq!(RANGE, Normalisation::Unit, "the family's graphs were switched to the signed range");
    }

    #[test]
    fn every_variant_names_the_artifact_it_is_published_as() {
        // Transcribed from the reference implementation's `models/ids_test.go`, which pins these as literals because
        // they key the on-disk cache and name the downloaded files.
        let expected = [
            (SharpenVariant::Moscow(FloatPrecision::Fp32), "sh_moscow_fp32"),
            (SharpenVariant::Moscow(FloatPrecision::Fp16), "sh_moscow_fp16"),
            (SharpenVariant::Petersburg(FloatPrecision::Fp32), "sh_petersburg_fp32"),
            (SharpenVariant::Petersburg(FloatPrecision::Fp16), "sh_petersburg_fp16"),
            (SharpenVariant::Novgorod(FloatPrecision::Fp32), "sh_novgorod_fp32"),
            (SharpenVariant::Novgorod(FloatPrecision::Fp16), "sh_novgorod_fp16"),
        ];

        for (variant, name) in expected {
            assert_eq!(Sharpen::new(variant, strength(1.0)).artifact().as_str(), name);
        }
    }

    #[test]
    fn an_operation_requires_exactly_one_artifact() {
        for variant in every_variant() {
            let required = Sharpen::new(variant, strength(0.5)).required_artifacts();

            assert_eq!(required.len(), 1, "{variant:?} required something other than one artifact");
            assert_eq!(required[0], Sharpen::new(variant, strength(0.5)).artifact());
        }
    }

    #[test]
    fn the_strength_never_reaches_the_artifact_name() {
        // The reference pins this too: the intensity is a per-run parameter and must never leak into the identity or
        // the file it resolves to.
        for value in [0.0, 0.5, 1.0, 3.0] {
            let operation = Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength(value));

            assert_eq!(operation.artifact().as_str(), "sh_moscow_fp32", "strength {value} reached the name");
        }
    }

    #[test]
    fn two_strengths_of_one_variant_are_two_operations() {
        // The question a consumer holding two operations actually has: did the user change anything? A strength is
        // part of the request, so two of them are two requests.
        //
        // Nothing is reloaded by their differing. A resident session is filed under the artifact it was opened from
        // and the provider it was opened on — see `crate::sessions` — so both of these need `sh_moscow_fp32` and the
        // second finds the weights the first opened, however the two compare.
        let soft = Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength(0.5));
        let strong = Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength(2.5));

        assert_ne!(soft, strong, "two strengths of one variant read as one request");
        assert_ne!(hash_of(&soft), hash_of(&strong), "two different operations hashed alike");

        // And the other half of the property: one request built twice is one operation.
        assert_eq!(soft, Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength(0.5)));
        assert_eq!(
            hash_of(&soft),
            hash_of(&Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength(0.5)))
        );
    }

    #[test]
    fn an_operation_is_usable_as_a_lookup_key_directly() {
        // Keyed on the whole request, so a differing strength is a differing key — which is what a consumer asking
        // "have I seen this request before?" wants. What must not reload is the *weights*, and that is decided by
        // `required_artifacts` rather than by this comparison; the two operations below name the same artifact.
        let mut seen = std::collections::HashMap::new();
        let quiet = Sharpen::new(SharpenVariant::Novgorod(FloatPrecision::Fp16), strength(0.5));
        let loud = Sharpen::new(SharpenVariant::Novgorod(FloatPrecision::Fp16), strength(2.0));

        seen.insert(quiet, "requested");

        assert_eq!(seen.get(&quiet), Some(&"requested"), "one request did not find itself");
        assert_eq!(seen.get(&loud), None, "a different strength found another request's entry");
        assert_eq!(quiet.required_artifacts(), loud.required_artifacts(), "the two would load different weights");
    }

    #[test]
    fn two_precisions_are_different_operations() {
        let fp32 = Sharpen::new(SharpenVariant::Petersburg(FloatPrecision::Fp32), strength(1.0));
        let fp16 = Sharpen::new(SharpenVariant::Petersburg(FloatPrecision::Fp16), strength(1.0));

        assert_ne!(fp32, fp16, "two precisions served by different artifacts compared as one model");
    }

    #[test]
    fn two_variants_are_never_the_same_operation() {
        let moscow = Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength(1.0));
        let novgorod = Sharpen::new(SharpenVariant::Novgorod(FloatPrecision::Fp32), strength(1.0));

        assert_ne!(moscow, novgorod);
    }

    #[test]
    fn two_strengths_do_not_collide_in_the_cache() {
        // They name one set of weights and produce different images, so the tag has to tell them apart. The identity
        // does too, and the tag is still the answer to "the same pixels" rather than a restatement of it: the two are
        // separate questions that happen to agree here.
        let soft = Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength(0.5));
        let strong = Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength(2.5));

        assert_eq!(
            soft.required_artifacts(),
            strong.required_artifacts(),
            "the premise is that one graph serves both"
        );
        assert_ne!(soft.cache_tag(), strong.cache_tag(), "two strengths shared a cache key");
    }

    #[test]
    fn the_cache_tag_is_the_written_one_rather_than_whatever_serialization_produces() {
        // Pinned here so that renaming a field is a failing test rather than a silent invalidation of every cached
        // image on every user's disk.
        assert_eq!(
            Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength(0.5)).cache_tag(),
            "sh-moscow-fp32-t0.5"
        );
        assert_eq!(
            Sharpen::new(SharpenVariant::Novgorod(FloatPrecision::Fp16), strength(1.0)).cache_tag(),
            "sh-novgorod-fp16-t1"
        );
    }

    #[test]
    fn a_repeated_operation_reports_a_stable_tag() {
        let operation = Sharpen::new(SharpenVariant::Petersburg(FloatPrecision::Fp32), strength(1.5));

        assert_eq!(operation.cache_tag(), operation.cache_tag());
        assert_eq!(
            operation.cache_tag(),
            Sharpen::new(SharpenVariant::Petersburg(FloatPrecision::Fp32), strength(1.5)).cache_tag()
        );
    }

    #[test]
    fn no_two_operations_producing_different_images_share_one_tag() {
        let mut seen = std::collections::HashMap::new();

        for variant in every_variant() {
            for value in [0.0, 0.5, 1.0, 1.5, 2.0, 3.0] {
                let operation = Sharpen::new(variant, strength(value));

                if let Some(previous) = seen.insert(operation.cache_tag(), operation) {
                    panic!("{previous:?} and {operation:?} share the cache tag {}", operation.cache_tag());
                }
            }
        }
    }

    #[test]
    fn a_cache_tag_can_never_be_mistaken_for_an_artifact_name() {
        for variant in every_variant() {
            let tag = Sharpen::new(variant, strength(1.0)).cache_tag();

            assert!(!tag.contains('_'), "{tag} is spelled the way an artifact name is");
        }
    }

    #[test]
    fn a_display_name_is_the_label_and_the_precision_and_carries_no_strength() {
        for value in [0.0, 0.5, 2.5] {
            let operation = Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength(value));

            assert_eq!(operation.display_name(), "Moscow (FP32)");
            assert_eq!(operation.to_string(), operation.display_name());
        }

        assert_eq!(
            Sharpen::new(SharpenVariant::Novgorod(FloatPrecision::Fp16), strength(1.0)).display_name(),
            "Novgorod (FP16)"
        );
    }

    #[test]
    fn a_run_is_handed_the_strength_the_identity_does_not_carry() {
        let operation = Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength(2.5));

        assert_eq!(operation.params(), SharpenParams { strength: strength(2.5) });
    }

    #[test]
    fn an_operation_round_trips_through_its_serialized_form() {
        for variant in every_variant() {
            let operation = Sharpen::new(variant, strength(0.5));
            let json = serde_json::to_string(&operation).expect("an operation serializes");
            let back: Sharpen = serde_json::from_str(&json).expect("an operation deserializes");

            assert_eq!(back, operation, "{json} did not round-trip to an equal operation");
            assert_eq!(back.variant(), operation.variant(), "{json} lost its variant or precision");
            assert_eq!(back.strength(), operation.strength(), "{json} lost its strength");
            assert_eq!(back.cache_tag(), operation.cache_tag(), "{json} round-tripped to a different image");
        }
    }

    #[test]
    fn the_serialized_form_names_its_variant_rather_than_positioning_it() {
        // Tagged by name so that adding a family or a variant later cannot invalidate anything already persisted.
        let json = serde_json::to_string(&Sharpen::new(SharpenVariant::Moscow(FloatPrecision::Fp32), strength(0.5)))
            .expect("an operation serializes");

        assert_eq!(json, r#"{"variant":{"codename":"moscow","precision":"fp32"},"strength":0.5}"#);
    }

    #[test]
    fn a_serialized_operation_pairing_a_variant_with_int8_is_refused() {
        let json = r#"{"variant":{"codename":"moscow","precision":"int8"},"strength":0.5}"#;

        assert!(
            serde_json::from_str::<Sharpen>(json).is_err(),
            "deserialization produced a sharpen at a precision it is not published in"
        );
    }

    #[test]
    fn a_serialized_operation_carrying_an_out_of_range_strength_is_refused() {
        let json = r#"{"variant":{"codename":"moscow","precision":"fp32"},"strength":4.0}"#;

        assert!(
            serde_json::from_str::<Sharpen>(json).is_err(),
            "deserialization produced an operation whose strength could not have been constructed"
        );
    }

    #[test]
    fn a_pipeline_built_for_each_variant_names_that_variants_one_artifact_and_one_session() {
        // The family's seam, checked for every variant and precision rather than one: all three reach the same
        // contract, so a seam that had regressed to naming one model would still hand back a working pipeline for the
        // others. Asked through `Model`, which is the whole of what the chain driver asks a pipeline before it runs.
        use crate::pipeline::Model;
        use crate::pipeline::test_support::NoBackend;

        for variant in every_variant() {
            let operation = Sharpen::new(variant, strength(0.5));
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
    fn one_session_serves_every_strength_of_one_variant() {
        // The pipeline half of `the_strength_never_reaches_the_artifact_name`: every position of a slider asks for
        // the same artifact under the same settings, so dragging it installs and opens nothing.
        use crate::pipeline::Model;
        use crate::pipeline::test_support::NoBackend;

        let variant = SharpenVariant::Novgorod(FloatPrecision::Fp16);
        let first = Sharpen::new(variant, strength(0.0)).pipeline::<NoBackend>();
        let second = Sharpen::new(variant, strength(3.0)).pipeline::<NoBackend>();

        assert_eq!(Model::<NoBackend>::required(first.as_ref()), Model::<NoBackend>::required(second.as_ref()));
        assert_eq!(Model::<NoBackend>::sessions(first.as_ref()), Model::<NoBackend>::sessions(second.as_ref()));
    }

    // What only this family knows, run through its own seam against the shared fake: which of its models are guarded.
    // `filter`'s own suite proves the mechanism once; these prove the seam hands it to the right models.

    /// The pipeline for `variant` at a strength of 1, built through the family's own seam.
    fn sharpening(variant: SharpenVariant) -> Shared<crate::pipeline::test_support::Fake> {
        Sharpen::new(variant, strength(1.0)).pipeline()
    }

    #[test]
    fn a_tile_petersburg_diverges_on_keeps_the_photographs_own_pixels() {
        // Fails if the seam stops handing `SharpenVariant::guard` to the pipeline: the first tile would then be the
        // model's, with one white pixel in it.
        use crate::pipeline::test_support::{PAIR, original, session};
        use imaging::ChannelDepth;
        use imaging::tensor::Sampler;
        use imaging::test_support::photograph;

        let source = photograph(PAIR.0, PAIR.1);
        let sampler = Sampler::new(&source);

        for precision in FloatPrecision::ALL {
            let handle = session(Some((0, 4.0)));
            let produced = sharpening(SharpenVariant::Petersburg(precision))
                .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
                .expect("a sharpen over a photograph");
            let produced = produced.as_rgb8().expect("eight-bit");

            for (x, y) in [(0, 0), (10, 150), (200, 30)] {
                assert_eq!(produced.get_pixel(x, y).0, original::<u8>(&sampler, x, y), "{precision:?} ({x}, {y})");
            }
        }
    }

    #[test]
    fn moscow_and_novgorod_pass_an_exploding_tile_through() {
        // The same backend that trips Petersburg's guard, and nothing is kept: every region is the model's.
        use crate::pipeline::test_support::{PAIR, darkened, session};
        use imaging::ChannelDepth;
        use imaging::tensor::Sampler;
        use imaging::test_support::photograph;

        let source = photograph(PAIR.0, PAIR.1);
        let sampler = Sampler::new(&source);

        for variant in [SharpenVariant::Moscow(FloatPrecision::Fp32), SharpenVariant::Novgorod(FloatPrecision::Fp16)] {
            let handle = session(Some((0, 4.0)));
            let produced = sharpening(variant)
                .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
                .expect("a sharpen over a photograph");
            let produced = produced.as_rgb8().expect("eight-bit");

            for (x, y) in [(0, 0), (10, 150), (200, 30), (400, 100)] {
                assert_eq!(
                    produced.get_pixel(x, y).0,
                    darkened::<u8>(&sampler, x, y),
                    "{variant:?} kept ({x}, {y}) rather than using the model's output"
                );
            }
        }
    }
}
