//! The detection family: one variant, one graph per precision, no per-run parameter, and a result that is not an
//! image.

pub(crate) mod newyork;
pub(crate) mod variant;

use serde::{Deserialize, Serialize};

use super::artifact::{ArtifactId, Family};
use super::precision::{FloatPrecision, Precision};
use super::simple::single_artifact;

use crate::providers::profile::EpProfile;

pub(crate) use variant::ALL;
pub use variant::DetectionVariant;

/// One face-detection run: which model, and nothing else. Two operations are equal exactly when their variants are.
///
/// What makes this family unlike all seven others is what a run *produces*:
/// [`DetectionOutput`](super::face::DetectionOutput), a set of faces, rather than an image.
///
/// Nothing here owns a native resource: it is a plain `Copy` value describing *what* to run, which is what lets a
/// front end hold and persist one independently of any session built from it.
///
/// # Which of the two to reach for
///
/// [`Detection::newyork`] is the one to **call**: it takes what a detection run takes and nothing else, which is
/// what makes a wrong pairing unwritable rather than refused, and it hands back the carrier for this family's
/// path. [`Analysis`](super::operation::Analysis) — which carries this as
/// [`Analysis::Detection`](super::operation::Analysis::Detection) — is the one to **keep**: stored, serialized
/// across a process boundary, or matched on to dispatch. It is deliberately *not*
/// [`Operation`](super::operation::Operation), which carries only the families whose result is an image, so a
/// detection cannot be placed in a chain at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Detection {
    // No per-run input, expressed as the **absence of a field** rather than as something defaulted to a neutral value,
    // like `colorization::Colorization` and for the same reason.
    /// The model and its precision — the whole of this operation's identity.
    variant: DetectionVariant,
}

impl Detection {
    /// The operation running `variant`.
    pub const fn new(variant: DetectionVariant) -> Self {
        Self { variant }
    }

    /// New York, this family's model, at `precision`.
    ///
    /// The one constructor a caller reaches for. It hands back an [`Analysis`](super::operation::Analysis) rather
    /// than an [`Operation`](super::operation::Operation), because a detection run's result is a set of faces rather
    /// than an image — so the value is what [`Opai::execute`](crate::Opai::execute) takes, and there is nothing a
    /// caller could hand to the wrong path.
    ///
    /// Published at FP32 and FP16 and at no other precision, which is why the argument is a
    /// [`FloatPrecision`].
    pub const fn newyork(precision: FloatPrecision) -> super::operation::Analysis {
        super::operation::Analysis::Detection(Self::new(DetectionVariant::NewYork(precision)))
    }

    /// The detection whose faces a face recovery restores: New York at FP32, **whatever the recovery model's tier**.
    ///
    /// The one answer every caller that feeds a face recovery shares — the autopilot's face signal, the GUI's face
    /// picker and its enhance path, and the benchmark's auxiliary run — so the faces a suggestion found, the faces a
    /// user picked from and the faces a restore is handed come from the same detection, and are served by the same
    /// run-store entry rather than by a second detector build keyed a second way.
    ///
    /// A constant rather than a function of the recovery model's precision. The two builds are required to find the
    /// same faces in the same order, differing at most sub-pixel, and a [`Face`](super::face::Face) is quantized to a
    /// hundredth of a pixel on acceptance, so in the overwhelming majority of cases the two answers are the *same
    /// value*. Following the tier would therefore buy nothing and cost a second detector on disk plus a second
    /// run-store entry per photograph per framing, where one serves both. FP32 rather than FP16 because it is the build
    /// every provider runs, and the one the reference detects with.
    pub const fn for_face_recovery() -> super::operation::Analysis {
        Self::newyork(FloatPrecision::Fp32)
    }

    /// Which model this runs, and at which precision.
    pub const fn variant(self) -> DetectionVariant {
        self.variant
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

    /// The name a user interface shows: `New York (FP32)`.
    pub fn display_name(self) -> String {
        self.to_string()
    }

    /// The artifact serving this operation.
    pub fn artifact(self) -> ArtifactId {
        single_artifact(Family::Detection, self.variant.codename(), self.precision())
    }

    /// The artifacts that must be on disk before this operation can run.
    ///
    /// Always exactly one: New York is published as a single graph.
    pub fn required_artifacts(self) -> Vec<ArtifactId> {
        // A `Vec` to match `upscale::Upscale::required_artifacts`, so that whoever downloads what an operation needs
        // asks every family the same question.
        vec![self.artifact()]
    }

    /// The key the run cache stores this operation's result under.
    ///
    /// The variant and the precision alone, because there is no third thing a run could vary. Hyphen-separated, so it
    /// cannot be mistaken for an underscore-separated artifact name.
    pub fn cache_tag(self) -> String {
        // Written deliberately rather than derived from the serialized form, so that renaming a field cannot silently
        // invalidate every cached result.
        format!("{}-{}-{}", Family::Detection.prefix(), self.variant.codename(), self.variant.precision())
    }
}

impl std::fmt::Display for Detection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.variant.label(), self.variant.precision().label())
    }
}

#[cfg(test)]
mod tests {
    use super::variant::tests::every_variant;
    use super::*;
    use crate::models::face::{DetectionOutput, Faces, tests::face_at};
    use crate::models::precision::FloatPrecision;
    use crate::models::test_support::hash_of;

    #[test]
    fn every_variant_names_the_artifact_it_is_published_as() {
        // Transcribed from the reference implementation's `models/ids_test.go`, which pins these as literals because
        // they key the on-disk cache and name the downloaded files.
        let expected = [
            (DetectionVariant::NewYork(FloatPrecision::Fp32), "dt_newyork_fp32"),
            (DetectionVariant::NewYork(FloatPrecision::Fp16), "dt_newyork_fp16"),
        ];

        for (variant, name) in expected {
            assert_eq!(Detection::new(variant).artifact().as_str(), name);
        }
    }

    #[test]
    fn an_operation_requires_exactly_one_artifact() {
        for variant in every_variant() {
            let required = Detection::new(variant).required_artifacts();

            assert_eq!(required.len(), 1, "{variant:?} required something other than one artifact");
            assert_eq!(required[0], Detection::new(variant).artifact());
        }
    }

    #[test]
    fn no_artifact_name_this_family_can_produce_is_an_int8_one() {
        // INT8 is unrepresentable rather than refused: `DetectionVariant` carries a `FloatPrecision`, so there is no
        // value to pass that would compose `dt_newyork_int8`.
        for variant in every_variant() {
            assert!(!Detection::new(variant).artifact().as_str().contains("int8"), "{variant:?}");
        }
    }

    #[test]
    fn an_operation_is_fully_described_by_its_variant_and_precision() {
        // Two operations naming the same variant at the same precision are built from the same one argument, so there
        // is nothing further that could distinguish them — the property the absent parameter field buys.
        for variant in every_variant() {
            let one = Detection::new(variant);
            let two = Detection::new(variant);

            assert_eq!(one, two, "{variant:?}");
            assert_eq!(hash_of(&one), hash_of(&two), "{variant:?} hashed two ways");
            assert_eq!(one.cache_tag(), two.cache_tag(), "{variant:?} reported two cache tags");
        }
    }

    #[test]
    fn two_precisions_are_different_operations() {
        let fp32 = Detection::new(DetectionVariant::NewYork(FloatPrecision::Fp32));
        let fp16 = Detection::new(DetectionVariant::NewYork(FloatPrecision::Fp16));

        assert_ne!(fp32, fp16, "two precisions served by different artifacts compared as one model");
        assert_ne!(fp32.cache_tag(), fp16.cache_tag());
    }

    #[test]
    fn an_operation_is_usable_as_a_lookup_key_directly() {
        let mut registry = std::collections::HashMap::new();
        registry.insert(Detection::new(DetectionVariant::NewYork(FloatPrecision::Fp16)), "loaded");

        assert_eq!(
            registry.get(&Detection::new(DetectionVariant::NewYork(FloatPrecision::Fp16))),
            Some(&"loaded"),
            "requesting the same model twice missed weights already resident"
        );
    }

    #[test]
    fn the_cache_tag_is_the_written_one_rather_than_whatever_serialization_produces() {
        // Pinned here so that renaming a field is a failing test rather than a silent invalidation of every cached
        // result on every user's disk.
        assert_eq!(Detection::new(DetectionVariant::NewYork(FloatPrecision::Fp32)).cache_tag(), "dt-newyork-fp32");
        assert_eq!(Detection::new(DetectionVariant::NewYork(FloatPrecision::Fp16)).cache_tag(), "dt-newyork-fp16");
    }

    #[test]
    fn a_cache_tag_carries_no_parameter_segment_and_can_never_be_mistaken_for_an_artifact_name() {
        for variant in every_variant() {
            let tag = Detection::new(variant).cache_tag();

            assert_eq!(tag.split('-').count(), 3, "{tag} carries more than the family, codename and precision");
            assert!(!tag.contains('_'), "{tag} is spelled the way an artifact name is");
        }
    }

    #[test]
    fn a_display_name_is_the_variant_and_the_precision() {
        let operation = Detection::new(DetectionVariant::NewYork(FloatPrecision::Fp32));

        assert_eq!(operation.display_name(), "New York (FP32)");
        assert_eq!(operation.to_string(), operation.display_name());
        assert_eq!(
            Detection::new(DetectionVariant::NewYork(FloatPrecision::Fp16)).display_name(),
            "New York (FP16)"
        );
    }

    #[test]
    fn what_a_run_produces_is_a_set_of_faces_rather_than_an_image() {
        // Nothing runs here, so what is checked is that the type exists and is the set both families speak — including
        // that an image with no face is an empty set rather than an error.
        let found: DetectionOutput = Faces::new([face_at(10.0, 20.0, 110.0, 140.0)]);
        assert_eq!(found.len(), 1);

        let none: DetectionOutput = Faces::empty();
        assert!(none.is_empty(), "an image with no face produced something other than an empty set");
    }

    #[test]
    fn an_operation_round_trips_through_its_serialized_form() {
        for variant in every_variant() {
            let operation = Detection::new(variant);
            let json = serde_json::to_string(&operation).expect("an operation serializes");
            let back: Detection = serde_json::from_str(&json).expect("an operation deserializes");

            assert_eq!(back, operation, "{json} did not round-trip to an equal operation");
            assert_eq!(back.variant(), operation.variant(), "{json} lost its variant or precision");
            assert_eq!(back.cache_tag(), operation.cache_tag(), "{json} round-tripped to a different result");
        }
    }

    #[test]
    fn the_serialized_form_names_its_variant_rather_than_positioning_it() {
        // Tagged by name so that adding a family or a variant later cannot invalidate anything already persisted.
        let json = serde_json::to_string(&Detection::new(DetectionVariant::NewYork(FloatPrecision::Fp32)))
            .expect("an operation serializes");

        assert_eq!(json, r#"{"variant":{"codename":"newyork","precision":"fp32"}}"#);
    }

    #[test]
    fn a_serialized_operation_pairing_new_york_with_int8_is_refused() {
        let json = r#"{"variant":{"codename":"newyork","precision":"int8"}}"#;

        assert!(
            serde_json::from_str::<Detection>(json).is_err(),
            "deserialization produced a detection at a precision it is not published in"
        );
    }
}
