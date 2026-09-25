//! The denoise family: three variants, each one graph per precision, run at a per-run strength.

pub(crate) mod gothenburg;
#[cfg(test)]
mod live;
pub(crate) mod malmo;
pub(crate) mod stockholm;
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
pub use variant::DenoiseVariant;

// `[0, 1]` is the reference's `standardize: false`. Pinned as a literal here and asserted in the tests below, because
// feeding a graph the wrong range is not an error a runtime reports: it is a worse photograph.
/// The range this family's graphs read and write: `[0, 1]`.
const RANGE: Normalisation = Normalisation::Unit;

/// One denoise run: which model, and how much of its output to apply.
///
/// This value **is** the operation's identity, as [`super::upscale::Upscale`]'s is. Nothing here owns a native
/// resource: it is a plain `Copy` value describing *what* to run, which is what lets a front end hold and persist one
/// independently of any session built from it.
///
/// Build this; keep it as an [`Operation::Denoise`] — see
/// [`Operation`] for which of the two to reach for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Denoise {
    /// The model and its precision.
    variant: DenoiseVariant,
    /// How much of the model's output this run applies, and part of the operation's identity: two values of it are
    /// two requests.
    strength: Strength,
}

impl Denoise {
    /// The operation running `variant` at `strength`.
    pub const fn new(variant: DenoiseVariant, strength: Strength) -> Self {
        Self { variant, strength }
    }

    /// Stockholm at `precision`, applying `strength` of its output.
    ///
    /// Returns the carrier rather than `Self`, so a caller collects operations of several families into one chain
    /// with no per-element wrapping; the typed value stays reachable by matching it. Published at FP32 and FP16.
    pub fn stockholm(precision: FloatPrecision, strength: Strength) -> Operation {
        Operation::Denoise(Self::new(DenoiseVariant::Stockholm(precision), strength))
    }

    /// Gothenburg at `precision`, applying `strength` of its output. Published at FP32 and FP16; see [`Denoise::stockholm`].
    pub fn gothenburg(precision: FloatPrecision, strength: Strength) -> Operation {
        Operation::Denoise(Self::new(DenoiseVariant::Gothenburg(precision), strength))
    }

    /// Malmo at `precision`, applying `strength` of its output. Published at FP32 and FP16; see [`Denoise::stockholm`].
    pub fn malmo(precision: FloatPrecision, strength: Strength) -> Operation {
        Operation::Denoise(Self::new(DenoiseVariant::Malmo(precision), strength))
    }

    /// Which model this runs, and at which precision.
    pub const fn variant(self) -> DenoiseVariant {
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

    /// The name a user interface shows: `Stockholm (FP32)`, without the strength.
    pub fn display_name(self) -> String {
        // The strength does not select the weights, so a name fixed at whichever value happened to load them first
        // would be wrong from the second run onwards.
        self.to_string()
    }

    /// What this run needs beyond the operation itself.
    pub const fn params(self) -> DenoiseParams {
        DenoiseParams { strength: self.strength }
    }

    /// The pipeline this operation runs — the **family's contract seam**, where a variant becomes a run.
    pub(crate) fn pipeline<B: Backend>(self) -> Shared<B> {
        // One arm, because the three variants differ in nothing that reaches `filter` but their weights, their guard
        // and their profile — not the tiling, not the normalisation, not what the strength means. No `Result`, as
        // `LightAdjustment::pipeline` has none and for the same reason: there is no state here to refuse.
        //
        // Built from what `params` publishes rather than from this type's own fields, so a pipeline cannot be
        // constructed from something the library did not say a run needs. The guard comes from the variant; see
        // `DenoiseVariant::guard`.
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
        single_artifact(Family::Denoise, self.variant.codename(), self.precision())
    }

    /// The artifacts that must be on disk before this operation can run.
    ///
    /// Always exactly one: every denoise variant is published as a single graph.
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
            Family::Denoise.prefix(),
            self.variant.codename(),
            self.variant.precision(),
            self.strength
        )
    }
}

/// What one denoise run takes, in the shape its pipeline wants it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenoiseParams {
    // A typed value rather than a map of strings to anything, for the reason `UpscaleParams` gives.
    /// How much of the model's output this run applies.
    pub strength: Strength,
}

impl std::fmt::Display for Denoise {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.variant.label(), self.variant.precision().label())
    }
}

#[cfg(test)]
mod tests {
    use super::variant::tests::every_variant;
    use super::*;
    use crate::models::precision::FloatPrecision;

    fn strength(value: f64) -> Strength {
        Strength::new(value).expect("the test supplied a strength in range")
    }

    crate::models::test_support::strength_family_tests! {
        Denoise, DenoiseVariant { Stockholm, Gothenburg, Malmo },
        params: DenoiseParams,
        every_variant: every_variant,
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
            (DenoiseVariant::Stockholm(FloatPrecision::Fp32), "dn_stockholm_fp32"),
            (DenoiseVariant::Stockholm(FloatPrecision::Fp16), "dn_stockholm_fp16"),
            (DenoiseVariant::Gothenburg(FloatPrecision::Fp32), "dn_gothenburg_fp32"),
            (DenoiseVariant::Gothenburg(FloatPrecision::Fp16), "dn_gothenburg_fp16"),
            (DenoiseVariant::Malmo(FloatPrecision::Fp32), "dn_malmo_fp32"),
            (DenoiseVariant::Malmo(FloatPrecision::Fp16), "dn_malmo_fp16"),
        ];

        for (variant, name) in expected {
            assert_eq!(Denoise::new(variant, strength(1.0)).artifact().as_str(), name);
        }
    }

    #[test]
    fn the_cache_tag_is_the_written_one_rather_than_whatever_serialization_produces() {
        // Pinned here so that renaming a field is a failing test rather than a silent invalidation of every cached
        // image on every user's disk.
        assert_eq!(
            Denoise::new(DenoiseVariant::Stockholm(FloatPrecision::Fp32), strength(0.5)).cache_tag(),
            "dn-stockholm-fp32-t0.5"
        );
        assert_eq!(
            Denoise::new(DenoiseVariant::Malmo(FloatPrecision::Fp16), strength(1.0)).cache_tag(),
            "dn-malmo-fp16-t1"
        );
    }

    #[test]
    fn a_display_name_is_the_label_and_the_precision_and_carries_no_strength() {
        for value in [0.0, 0.5, 2.5] {
            let operation = Denoise::new(DenoiseVariant::Stockholm(FloatPrecision::Fp32), strength(value));

            assert_eq!(operation.display_name(), "Stockholm (FP32)");
            assert_eq!(operation.to_string(), operation.display_name());
        }

        // The accented label, and the one denoise row whose codename and label are not one another's
        // case: what a user reads is `Malmö` while everything composed from it stays `malmo` - which
        // `every_variant_names_the_artifact_it_is_published_as` above pins from the other side.
        assert_eq!(
            Denoise::new(DenoiseVariant::Malmo(FloatPrecision::Fp16), strength(1.0)).display_name(),
            "Malmö (FP16)"
        );
    }

    #[test]
    fn the_serialized_form_names_its_variant_rather_than_positioning_it() {
        // Tagged by name so that adding a family or a variant later cannot invalidate anything already persisted.
        let json = serde_json::to_string(&Denoise::new(DenoiseVariant::Stockholm(FloatPrecision::Fp32), strength(0.5)))
            .expect("an operation serializes");

        assert_eq!(json, r#"{"variant":{"codename":"stockholm","precision":"fp32"},"strength":0.5}"#);
    }

    #[test]
    fn a_serialized_operation_pairing_a_variant_with_int8_is_refused() {
        let json = r#"{"variant":{"codename":"stockholm","precision":"int8"},"strength":0.5}"#;

        assert!(
            serde_json::from_str::<Denoise>(json).is_err(),
            "deserialization produced a denoise at a precision it is not published in"
        );
    }

    #[test]
    fn a_serialized_operation_carrying_an_out_of_range_strength_is_refused() {
        let json = r#"{"variant":{"codename":"stockholm","precision":"fp32"},"strength":4.0}"#;

        assert!(
            serde_json::from_str::<Denoise>(json).is_err(),
            "deserialization produced an operation whose strength could not have been constructed"
        );
    }

    // What only this family knows, run through its own seam against the shared fake: which of its models are guarded.
    // `filter`'s own suite proves the mechanism once; these prove the seam hands it to the right models.

    /// The pipeline for `variant` at a strength of 1, built through the family's own seam.
    fn denoising(variant: DenoiseVariant) -> Shared<crate::pipeline::test_support::Fake> {
        Denoise::new(variant, strength(1.0)).pipeline()
    }

    #[test]
    fn a_tile_stockholm_diverges_on_keeps_the_photographs_own_pixels() {
        // Fails if the seam stops handing `DenoiseVariant::guard` to the pipeline: the first tile would then be the
        // model's, with one white pixel in it.
        use crate::pipeline::test_support::{PAIR, original, session};
        use imaging::ChannelDepth;
        use imaging::tensor::Sampler;
        use imaging::test_support::photograph;

        let source = photograph(PAIR.0, PAIR.1);
        let sampler = Sampler::new(&source);

        for precision in FloatPrecision::ALL {
            let handle = session(Some((0, 4.0)));
            let produced = denoising(DenoiseVariant::Stockholm(precision))
                .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
                .expect("a denoise over a photograph");
            let produced = produced.as_rgb8().expect("eight-bit");

            for (x, y) in [(0, 0), (10, 150), (200, 30)] {
                assert_eq!(produced.get_pixel(x, y).0, original::<u8>(&sampler, x, y), "{precision:?} ({x}, {y})");
            }
        }
    }

    #[test]
    fn gothenburg_and_malmo_pass_an_exploding_tile_through() {
        // The same backend that trips Stockholm's guard, and nothing is kept: every region is the model's.
        use crate::pipeline::test_support::{PAIR, darkened, session};
        use imaging::ChannelDepth;
        use imaging::tensor::Sampler;
        use imaging::test_support::photograph;

        let source = photograph(PAIR.0, PAIR.1);
        let sampler = Sampler::new(&source);

        for variant in [DenoiseVariant::Gothenburg(FloatPrecision::Fp32), DenoiseVariant::Malmo(FloatPrecision::Fp16)] {
            let handle = session(Some((0, 4.0)));
            let produced = denoising(variant)
                .run(&source, std::slice::from_ref(&handle), ChannelDepth::Eight, None, &|| false)
                .expect("a denoise over a photograph");
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
