//! The colorization family: three variants, each one graph per precision, and no per-run parameter at all.
//!
//! # The Lab contract
//!
//! The family's graphs predict chroma only. Each sees the whole photograph stretched to its own square and returns
//! the a/b of CIELab at that size. The detail comes from the photograph itself: its own L is kept at full resolution,
//! the predicted a/b is put onto it, and the result is converted back to sRGB.
//!
//! What a run is built from, each at the tier its callers put it:
//!
//! - `process`: the pipeline every variant runs. The photograph is stretched to the model's square, the graph is run
//!   once, and the predicted chroma is put onto the photograph's own lightness. It also holds `Contract`, the two
//!   contracts as data on the variant.
//! - `lab`: the round trip through CIELab, and the fast paths that make it affordable twice per photograph pixel.
//! - `stretch`: the photograph stretched to a graph's square, with any alpha composited against black first.
//! - `ab`: the Ab contract Delhi and Mumbai share. A gray rendering goes in at 512, and the a/b planes come out.
//! - `jaipur::rgb`: the Rgb contract, Jaipur's alone. ITU-601 luma goes in at 560, and an RGB rendering comes out,
//!   whose a/b is extracted.
//! - `compose`: the predicted a/b put onto the photograph's own full-resolution L, in one fused pass at the run's
//!   depth.
//!
//! Each model's directory, `delhi`, `mumbai` and `jaipur`, holds its execution-provider profile and the prose behind
//! it.

mod ab;
mod compose;
mod delhi;
mod jaipur;
mod lab;
#[cfg(test)]
mod live;
mod mumbai;
mod process;
mod stretch;
pub(crate) mod variant;

pub(crate) use lab::srgb_to_linear;

use serde::{Deserialize, Serialize};

use super::artifact::{ArtifactId, Family};
use super::operation::Operation;
use super::precision::{FloatPrecision, Precision};
use super::simple::single_artifact;

use crate::pipeline::Backend;
use crate::pipeline::Shared;
use crate::providers::profile::EpProfile;

pub(crate) use variant::ALL;
pub use variant::ColorizationVariant;

/// One colorization run: which model, and nothing else.
///
/// One of the two families that takes no per-run input, alongside [`super::detection::Detection`]: the variant and its
/// precision are the whole of what a run is.
///
/// Nothing here owns a native resource: it is a plain `Copy` value describing *what* to run, which is what lets a
/// front end hold and persist one independently of any session built from it.
///
/// Build this; keep it as an [`Operation::Colorization`] — see
/// [`Operation`] for which of the two to reach for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Colorization {
    // No per-run input is expressed as the **absence of a field** rather than as an amount defaulted to something
    // neutral, so nothing unused has to be carried or ignored.
    /// The model and its precision — the whole of this operation's identity.
    variant: ColorizationVariant,
}

impl Colorization {
    /// The operation running `variant`.
    pub const fn new(variant: ColorizationVariant) -> Self {
        Self { variant }
    }

    /// Delhi at `precision`.
    ///
    /// Returns the carrier rather than `Self`, so a caller collects operations of several families into one chain
    /// with no per-element wrapping; the typed value stays reachable by matching it. Published at FP32 and FP16.
    pub fn delhi(precision: FloatPrecision) -> Operation {
        Operation::Colorization(Self::new(ColorizationVariant::Delhi(precision)))
    }

    /// Mumbai at `precision`. Published at FP32 and FP16; see [`Colorization::delhi`].
    pub fn mumbai(precision: FloatPrecision) -> Operation {
        Operation::Colorization(Self::new(ColorizationVariant::Mumbai(precision)))
    }

    /// Jaipur at `precision`. Published at FP32 and FP16; see [`Colorization::delhi`].
    pub fn jaipur(precision: FloatPrecision) -> Operation {
        Operation::Colorization(Self::new(ColorizationVariant::Jaipur(precision)))
    }

    /// Which model this runs, and at which precision.
    pub const fn variant(self) -> ColorizationVariant {
        self.variant
    }

    /// The precision this runs at, widened to the common currency.
    pub fn precision(self) -> Precision {
        self.variant.precision()
    }

    /// The execution-provider tuning measured for the model this runs, at the precision it runs at.
    pub(crate) fn profile(self) -> EpProfile {
        self.variant.profile()
    }

    /// The pipeline this operation runs — the **family's contract seam**, where a variant becomes a run.
    pub(crate) fn pipeline<B: Backend>(self) -> Shared<B> {
        // One arm, and no `Result`, as `LightAdjustment::pipeline` has none and for the same reason: every variant this
        // family publishes has a graph, a contract and a profile, so there is no state here to refuse. The two
        // contracts differ in data the variant answers, not in which pipeline runs; see `ColorizationVariant::graph`.
        process::pipeline::<B>(self.display_name(), self.artifact(), self.profile(), self.variant.contract())
    }

    /// The name a user interface shows: `Delhi (FP32)`.
    pub fn display_name(self) -> String {
        self.to_string()
    }

    /// The artifact serving this operation.
    pub fn artifact(self) -> ArtifactId {
        single_artifact(Family::Colorization, self.variant.codename(), self.precision())
    }

    /// The artifacts that must be on disk before this operation can run.
    ///
    /// Always exactly one: every colorization variant is published as a single graph.
    pub fn required_artifacts(self) -> Vec<ArtifactId> {
        // A `Vec` to match `Upscale::required_artifacts`, so that whoever downloads what an operation needs asks every
        // family the same question.
        vec![self.artifact()]
    }

    /// The key the run cache stores this operation's result under.
    ///
    /// The variant and the precision alone, because they are the whole of what changes the resulting image.
    /// Hyphen-separated, so it cannot be mistaken for an underscore-separated artifact name.
    pub fn cache_tag(self) -> String {
        // Written deliberately rather than derived from the serialized form, so that renaming a field cannot silently
        // invalidate every cached image.
        format!("{}-{}-{}", Family::Colorization.prefix(), self.variant.codename(), self.variant.precision())
    }
}

impl std::fmt::Display for Colorization {
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

    #[test]
    fn every_variant_names_the_artifact_it_is_published_as() {
        // Transcribed from the reference implementation's `models/ids_test.go`, which pins these as literals because
        // they key the on-disk cache and name the downloaded files.
        let expected = [
            (ColorizationVariant::Delhi(FloatPrecision::Fp32), "cl_delhi_fp32"),
            (ColorizationVariant::Delhi(FloatPrecision::Fp16), "cl_delhi_fp16"),
            (ColorizationVariant::Mumbai(FloatPrecision::Fp32), "cl_mumbai_fp32"),
            (ColorizationVariant::Mumbai(FloatPrecision::Fp16), "cl_mumbai_fp16"),
            (ColorizationVariant::Jaipur(FloatPrecision::Fp32), "cl_jaipur_fp32"),
            (ColorizationVariant::Jaipur(FloatPrecision::Fp16), "cl_jaipur_fp16"),
        ];

        for (variant, name) in expected {
            assert_eq!(Colorization::new(variant).artifact().as_str(), name);
        }
    }

    #[test]
    fn an_operation_requires_exactly_one_artifact() {
        for variant in every_variant() {
            let required = Colorization::new(variant).required_artifacts();

            assert_eq!(required.len(), 1, "{variant:?} required something other than one artifact");
            assert_eq!(required[0], Colorization::new(variant).artifact());
        }
    }

    #[test]
    fn an_operation_is_fully_described_by_its_variant_and_precision() {
        // Two operations naming the same variant at the same precision are built from the same one argument, so there
        // is nothing further that could distinguish them — the property the absent parameter field buys.
        for variant in every_variant() {
            let one = Colorization::new(variant);
            let two = Colorization::new(variant);

            assert_eq!(one, two, "{variant:?}");
            assert_eq!(hash_of(&one), hash_of(&two), "{variant:?} hashed two ways");
            assert_eq!(one.cache_tag(), two.cache_tag(), "{variant:?} reported two cache tags");
        }
    }

    #[test]
    fn the_identity_and_the_cache_tag_agree_across_the_whole_family() {
        // For the parameterized families this agreement rests on each tag spelling its per-run parameter itself,
        // because the tag is written out by hand rather than derived. Here it rests on the variant alone: with no
        // per-run input, two equal operations always produce the same image and two unequal ones never produce the
        // same one.
        for one in every_variant() {
            for two in every_variant() {
                assert_eq!(
                    Colorization::new(one) == Colorization::new(two),
                    Colorization::new(one).cache_tag() == Colorization::new(two).cache_tag(),
                    "{one:?} and {two:?} disagree about whether they are one operation and one image"
                );
            }
        }
    }

    #[test]
    fn two_precisions_are_different_operations() {
        let fp32 = Colorization::new(ColorizationVariant::Delhi(FloatPrecision::Fp32));
        let fp16 = Colorization::new(ColorizationVariant::Delhi(FloatPrecision::Fp16));

        assert_ne!(fp32, fp16, "two precisions served by different artifacts compared as one model");
    }

    #[test]
    fn two_variants_are_never_the_same_operation() {
        let delhi = Colorization::new(ColorizationVariant::Delhi(FloatPrecision::Fp32));
        let jaipur = Colorization::new(ColorizationVariant::Jaipur(FloatPrecision::Fp32));

        assert_ne!(delhi, jaipur);
    }

    #[test]
    fn an_operation_is_usable_as_a_lookup_key_directly() {
        let mut registry = std::collections::HashMap::new();
        registry.insert(Colorization::new(ColorizationVariant::Mumbai(FloatPrecision::Fp16)), "loaded");

        assert_eq!(
            registry.get(&Colorization::new(ColorizationVariant::Mumbai(FloatPrecision::Fp16))),
            Some(&"loaded"),
            "requesting the same model twice missed weights already resident"
        );
    }

    #[test]
    fn the_cache_tag_is_the_written_one_rather_than_whatever_serialization_produces() {
        // Pinned here so that renaming a field is a failing test rather than a silent invalidation of every cached
        // image on every user's disk.
        assert_eq!(Colorization::new(ColorizationVariant::Delhi(FloatPrecision::Fp32)).cache_tag(), "cl-delhi-fp32");
        assert_eq!(
            Colorization::new(ColorizationVariant::Jaipur(FloatPrecision::Fp16)).cache_tag(),
            "cl-jaipur-fp16"
        );
    }

    #[test]
    fn a_cache_tag_carries_no_parameter_segment() {
        // The shape a family that acquired a neutral default would grow, and the one thing that would make two runs
        // of one colorization look like two images.
        for variant in every_variant() {
            let tag = Colorization::new(variant).cache_tag();

            assert_eq!(tag.split('-').count(), 3, "{tag} carries more than the family, codename and precision");
            assert!(!tag.contains("-t"), "{tag} carries a strength segment");
            assert!(!tag.contains("-b"), "{tag} carries a bias segment");
        }
    }

    #[test]
    fn no_two_operations_producing_different_images_share_one_tag() {
        let mut seen = std::collections::HashMap::new();

        for variant in every_variant() {
            let operation = Colorization::new(variant);

            if let Some(previous) = seen.insert(operation.cache_tag(), operation) {
                panic!("{previous:?} and {operation:?} share the cache tag {}", operation.cache_tag());
            }
        }
    }

    #[test]
    fn a_cache_tag_can_never_be_mistaken_for_an_artifact_name() {
        for variant in every_variant() {
            let tag = Colorization::new(variant).cache_tag();

            assert!(!tag.contains('_'), "{tag} is spelled the way an artifact name is");
        }
    }

    #[test]
    fn a_display_name_is_the_variant_and_the_precision() {
        let operation = Colorization::new(ColorizationVariant::Delhi(FloatPrecision::Fp32));

        assert_eq!(operation.display_name(), "Delhi (FP32)");
        assert_eq!(operation.to_string(), operation.display_name());
        assert_eq!(
            Colorization::new(ColorizationVariant::Mumbai(FloatPrecision::Fp16)).display_name(),
            "Mumbai (FP16)"
        );
    }

    #[test]
    fn an_operation_round_trips_through_its_serialized_form() {
        for variant in every_variant() {
            let operation = Colorization::new(variant);
            let json = serde_json::to_string(&operation).expect("an operation serializes");
            let back: Colorization = serde_json::from_str(&json).expect("an operation deserializes");

            assert_eq!(back, operation, "{json} did not round-trip to an equal operation");
            assert_eq!(back.variant(), operation.variant(), "{json} lost its variant or precision");
            assert_eq!(back.cache_tag(), operation.cache_tag(), "{json} round-tripped to a different image");
        }
    }

    #[test]
    fn the_serialized_form_names_its_variant_rather_than_positioning_it() {
        // Tagged by name so that adding a family or a variant later cannot invalidate anything already persisted.
        let json = serde_json::to_string(&Colorization::new(ColorizationVariant::Delhi(FloatPrecision::Fp32)))
            .expect("an operation serializes");

        assert_eq!(json, r#"{"variant":{"codename":"delhi","precision":"fp32"}}"#);
    }

    #[test]
    fn a_serialized_operation_pairing_a_variant_with_int8_is_refused() {
        let json = r#"{"variant":{"codename":"delhi","precision":"int8"}}"#;

        assert!(
            serde_json::from_str::<Colorization>(json).is_err(),
            "deserialization produced a colorization at a precision it is not published in"
        );
    }
}
