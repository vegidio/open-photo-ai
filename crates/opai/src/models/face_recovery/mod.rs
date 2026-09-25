//! The face recovery family: two variants, each one graph per precision, run over a set of pre-detected faces.

pub(crate) mod athens;
pub(crate) mod composite;
pub(crate) mod mask;
pub(crate) mod restore;
pub(crate) mod santorini;
pub(crate) mod transform;
pub(crate) mod variant;
pub(crate) mod warp;

use serde::{Deserialize, Serialize};

use super::artifact::{ArtifactId, Family};
use super::face::Faces;
use super::fidelity::Fidelity;
use super::operation::Operation;
use super::precision::{FloatPrecision, Precision};
use super::simple::single_artifact;

use crate::pipeline::Backend;
use crate::pipeline::Shared;
use crate::providers::profile::EpProfile;

pub(crate) use variant::ALL;
pub use variant::FaceRecoveryVariant;

// The faces are supplied rather than detected here, unlike the reference, whose `facerecovery.GetDtModel` acquires
// the detection model from inside face recovery, so the two are coupled and a face-recovery run holds two models at
// once. Here `detection::Detection` is an operation in its own right, run by whoever wanted the faces, and it reports
// its own artifact — so "what must be on disk before this can run" stays true, and no model is pinned as *the*
// detector from inside this family.
/// One face recovery run: which model, and which faces it is to restore.
///
/// The faces are **supplied by the caller** rather than detected here, so this operation never requires the detection
/// graph.
///
/// The one operation of the eight that is `Clone` rather than `Copy`. A variable-length set cannot be `Copy`, and
/// [`Faces`] is where the cost of that is paid down: it is backed by an `Arc`, so cloning an operation is a refcount
/// bump rather than a copy of every face. Nothing here owns a native resource — it is still a plain value describing
/// *what* to run, which is what lets a front end hold and persist one independently of any session built from it.
///
/// # Which of the two to reach for
///
/// This type is the one to **build**: its constructor takes what a face recovery run takes and nothing else, which is
/// what makes a wrong pairing unwritable rather than refused. [`Operation`] — which
/// carries this as [`Operation::FaceRecovery`] — is the one to **keep**:
/// stored, queued, serialized across a process boundary, or matched on to dispatch, by a holder that need not know
/// which of the eight families it has.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FaceRecovery {
    /// The model and its precision.
    variant: FaceRecoveryVariant,
    /// The faces this run is to restore, and part of the operation's identity: two selections are two requests.
    faces: Faces,
    /// How closely this run is to keep to the faces it was given, for the variant whose graph takes a second input,
    /// and `None` for the one that does not.
    ///
    /// Private, and reachable only through [`FaceRecovery::athens`]: [`FaceRecovery::santorini`] takes no such
    /// argument and sets this to `None`, so a fidelity cannot be given to the variant that has nowhere to bind it.
    ///
    /// Inside the identity and inside the cache tag: two fidelities are two requests and two images.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fidelity: Option<Fidelity>,
}

impl FaceRecovery {
    /// Athens at `precision`, restoring `faces`, keeping to them as closely as `fidelity` asks.
    ///
    /// An empty `faces` is a legitimate value rather than an error: an image with no face, or one where the user has
    /// deselected every face, is a run that recovers nothing.
    pub fn athens(precision: FloatPrecision, faces: Faces, fidelity: Fidelity) -> Operation {
        // The reference hard-codes the fidelity at `Fidelity::MAX`, and that remains the default a front end offers,
        // so an untouched control matches it exactly.
        Operation::FaceRecovery(Self {
            variant: FaceRecoveryVariant::Athens(precision),
            faces,
            fidelity: Some(fidelity),
        })
    }

    /// Santorini at `precision`, restoring `faces`.
    ///
    /// No fidelity to supply, and nowhere to put one: this variant's graph takes the image alone. See
    /// [`FaceRecovery::athens`] for the constructor that does take one, and [`FaceRecovery::fidelity`] for what an
    /// operation built here reports.
    pub fn santorini(precision: FloatPrecision, faces: Faces) -> Operation {
        Operation::FaceRecovery(Self { variant: FaceRecoveryVariant::Santorini(precision), faces, fidelity: None })
    }

    /// This run over `faces` instead, with the model, the precision and the fidelity left exactly as they were.
    ///
    /// For a caller that decides *what* to run before it can know *which faces*: a front end resolving a request, or
    /// a benchmark selecting its rows, builds the operation first — over an empty selection — and hands it the faces
    /// once a detection run has found them. Neither then has to match on the variant to rebuild it through the right
    /// constructor, which is how a model added later would be rebuilt as the wrong one, or its fidelity dropped.
    #[must_use]
    pub fn with_faces(&self, faces: Faces) -> Self {
        Self { faces, ..self.clone() }
    }

    /// Which model this runs, and at which precision.
    pub const fn variant(&self) -> FaceRecoveryVariant {
        self.variant
    }

    /// The faces this run is to restore, in the order they were given and unchanged.
    pub const fn faces(&self) -> &Faces {
        &self.faces
    }

    /// How closely this run is to keep to the faces it was given, or `None` where the model takes no such value.
    ///
    /// `Some` for an operation built by [`FaceRecovery::athens`] and `None` for one built by
    /// [`FaceRecovery::santorini`]. A front end reads this to decide whether it has a control to draw, and
    /// [`fn@crate::catalogue`] publishes the same fact ahead of building anything.
    pub const fn fidelity(&self) -> Option<Fidelity> {
        self.fidelity
    }

    /// The precision this runs at, widened to the common currency.
    pub fn precision(&self) -> Precision {
        self.variant.precision()
    }

    /// The execution-provider tuning measured for the model this runs, at the precision it runs at.
    pub(crate) fn profile(&self) -> EpProfile {
        // Asked of the variant, because the variant is what names the graph a measurement was made against — and the
        // precision it was measured at travels inside it.
        self.variant.profile()
    }

    /// The name a user interface shows: `Athens (FP32)`. The faces are not in it.
    pub fn display_name(&self) -> String {
        // The faces do not select the weights, so a name fixed at whichever selection happened to load them would be
        // wrong from the second run onwards — the same reason a strength is not in `denoise::Denoise::display_name`.
        self.to_string()
    }

    /// What one run of this operation takes beyond naming the model.
    pub fn params(&self) -> FaceRecoveryParams {
        FaceRecoveryParams { faces: self.faces.clone(), fidelity: self.fidelity }
    }

    /// The artifact serving this operation.
    pub fn artifact(&self) -> ArtifactId {
        single_artifact(Family::FaceRecovery, self.variant.codename(), self.precision())
    }

    /// The artifacts that must be on disk before this operation can run.
    ///
    /// Exactly one, and never a detection graph: this operation never opens one. Whoever detected the faces asked
    /// [`super::detection::Detection`], which reported that artifact itself.
    pub fn required_artifacts(&self) -> Vec<ArtifactId> {
        vec![self.artifact()]
    }

    /// The pipeline this operation runs — the **family's contract seam**, where a variant becomes a run.
    pub(crate) fn pipeline<B: Backend>(&self) -> Shared<B> {
        // **One arm over both variants**, because both restore through the same contract: `restore::pipeline` aligns
        // each face onto the same template, runs one graph, and feathers the result back through the same mask. The
        // variants differ at exactly one point — whether a weight is bound to the graph — and that is resolved ahead of
        // this call by `FaceRecoveryVariant::weight`, which is where a per-model property belongs and why the option
        // it answers is not the one `params` carries.
        //
        // Built from what `params` publishes rather than from this type's own fields: a pipeline cannot be constructed
        // from something the library did not say a run needs.
        //
        // No `Result`: there is no variant it can refuse and no resolution that can fail, so a `Result` here would be
        // a failure case no caller could reach and no test could construct. Deliberately **not** symmetric with
        // `Upscale::pipeline`, which keeps its `Result` because it has a real guard: a scale that resolves to no pass
        // sequence. Uniformity between the two seams would be bought by making one of them lie, and the arm in
        // `Operation::pipeline` that adapts the two shapes is one line.
        restore::pipeline::<B>(
            self.display_name(),
            self.artifact(),
            self.profile(),
            self.params(),
            self.variant.weight(self.fidelity),
        )
    }

    /// The key the run cache stores this operation's result under.
    ///
    /// Carries the faces, which the tag has to spell itself because it is written out by hand rather than derived
    /// from the identity: one set of weights serves every selection, but two selections produce two images, so a tag
    /// that left them out would serve the second run the first one's picture.
    /// The signature spells each bounding box out at two decimals, in order, semicolon-terminated — the reference's
    /// `FacesCacheKey` verbatim.
    ///
    /// A selection carrying no faces reports `…-f0`, which no non-empty selection can produce: a real box at the
    /// origin renders `0.00,…`.
    ///
    /// Hyphen-separated, so it cannot be mistaken for an underscore-separated artifact name.
    pub fn cache_tag(&self) -> String {
        // Written deliberately rather than derived from the serialized form, so that renaming a field cannot silently
        // invalidate every cached image.
        let mut tag =
            format!("{}-{}-{}", Family::FaceRecovery.prefix(), self.variant.codename(), self.variant.precision());

        // Written only where there is one: a variant that carries no fidelity cannot be made to miss the cache by a
        // value it does not have. `w` rather than `f`, which the faces already use.
        if let Some(fidelity) = self.fidelity {
            use std::fmt::Write as _;

            // Writing into a `String` cannot fail, so there is no error here to propagate to a caller.
            let _ = write!(tag, "-w{fidelity}");
        }

        tag.push_str("-f");

        // The reference returns `""` for an empty selection and drops the contribution entirely, which is correct but
        // leaves an empty selection's tag indistinguishable in shape from a family that takes no parameter at all. An
        // explicit `f0` says "this operation carries faces, and there were none".
        if self.faces.is_empty() {
            tag.push('0');
        } else {
            self.faces.write_cache_signature(&mut tag);
        }

        tag
    }
}

// A typed value rather than a map of strings to anything. The reference passes the faces under the stringly-typed key
// `"faces"` and substitutes nothing on a miss, so a typo there is a run that silently restores no face; here a
// parameter that is not carried cannot be read.
/// What one face recovery run takes, in the shape its pipeline wants it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FaceRecoveryParams {
    /// The faces this run is to restore.
    pub faces: Faces,
    /// How closely this run is to keep to them, or `None` where the model takes no such value — see
    /// [`FaceRecovery::fidelity`].
    pub fidelity: Option<Fidelity>,
}

impl std::fmt::Display for FaceRecovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.variant.label(), self.variant.precision().label())
    }
}

#[cfg(test)]
mod tests {
    use super::variant::tests::every_variant;
    use super::*;
    use crate::models::face::tests::face_at;
    use crate::models::precision::FloatPrecision;
    use crate::models::test_support::hash_of;

    #[test]
    fn with_faces_swaps_the_faces_alone() {
        let faces = Faces::new([face_at(10.0, 20.0, 110.0, 140.0)]);
        let fidelity = Fidelity::new(0.25).expect("in range");

        for (empty, full) in [
            (
                FaceRecovery::athens(FloatPrecision::Fp16, Faces::empty(), fidelity),
                FaceRecovery::athens(FloatPrecision::Fp16, faces.clone(), fidelity),
            ),
            (
                FaceRecovery::santorini(FloatPrecision::Fp32, Faces::empty()),
                FaceRecovery::santorini(FloatPrecision::Fp32, faces.clone()),
            ),
        ] {
            let (Operation::FaceRecovery(empty), Operation::FaceRecovery(full)) = (empty, full) else {
                panic!("a face recovery constructor built another family");
            };

            assert_eq!(empty.with_faces(faces.clone()), full, "{:?}", empty.variant());
        }
    }

    /// The fidelity the tests below use where the value is not what they are about.
    ///
    /// [`Fidelity::MAX`], which is the reference implementation's hard-coded value and the default a front end
    /// offers — so a tag pinned against it is the tag a user who touched no control produces.
    fn default_fidelity() -> Fidelity {
        Fidelity::MAXIMUM
    }

    /// The typed operation `variant` names over `faces`, built through the constructor a caller would call.
    ///
    /// There is no other way in: the family publishes `athens` and `santorini` and nothing that takes a variant, so
    /// a test driving every variant has to make the same choice a caller does — Athens is given a fidelity and
    /// Santorini has nowhere to put one. Unwrapped back to the typed value, because what these tests check is what
    /// the typed value reports.
    fn built(variant: FaceRecoveryVariant, faces: Faces, fidelity: Fidelity) -> FaceRecovery {
        let carried = match variant {
            FaceRecoveryVariant::Athens(precision) => FaceRecovery::athens(precision, faces, fidelity),
            FaceRecoveryVariant::Santorini(precision) => FaceRecovery::santorini(precision, faces),
        };

        match carried {
            Operation::FaceRecovery(operation) => operation,
            other => panic!("a face recovery constructor built {other:?}"),
        }
    }

    /// The same, at the fidelity the tests that are not about the fidelity use.
    fn typical(variant: FaceRecoveryVariant, faces: Faces) -> FaceRecovery {
        built(variant, faces, default_fidelity())
    }

    /// One face, two faces, and the same two reversed — the spread every test below picks from.
    fn one() -> Faces {
        Faces::new([face_at(12.34, 56.78, 90.12, 34.56)])
    }

    fn two() -> Faces {
        Faces::new([face_at(12.34, 56.78, 90.12, 34.56), face_at(200.0, 100.5, 260.25, 170.75)])
    }

    fn reversed() -> Faces {
        Faces::new([face_at(200.0, 100.5, 260.25, 170.75), face_at(12.34, 56.78, 90.12, 34.56)])
    }

    #[test]
    fn every_variant_names_the_artifact_it_is_published_as() {
        // Transcribed from the reference implementation's `models/ids_test.go`, which pins these as literals because
        // they key the on-disk cache and name the downloaded files.
        let expected = [
            (FaceRecoveryVariant::Athens(FloatPrecision::Fp32), "fr_athens_fp32"),
            (FaceRecoveryVariant::Athens(FloatPrecision::Fp16), "fr_athens_fp16"),
            (FaceRecoveryVariant::Santorini(FloatPrecision::Fp32), "fr_santorini_fp32"),
            (FaceRecoveryVariant::Santorini(FloatPrecision::Fp16), "fr_santorini_fp16"),
        ];

        for (variant, name) in expected {
            assert_eq!(typical(variant, two()).artifact().as_str(), name);
        }
    }

    #[test]
    fn an_operation_requires_exactly_one_artifact_and_never_the_detection_graph() {
        // The divergence from the reference that this family is built around: the faces arrive already detected, so
        // a detection graph named here would be one this operation never opens.
        for variant in every_variant() {
            for faces in [Faces::empty(), one(), two()] {
                let operation = typical(variant, faces);
                let required = operation.required_artifacts();

                assert_eq!(required.len(), 1, "{variant:?} required something other than one artifact");
                assert_eq!(required[0], operation.artifact());
                assert!(
                    !required.iter().any(|id| id.as_str().starts_with("dt_")),
                    "{variant:?} reported a detection artifact it never opens"
                );
            }
        }
    }

    #[test]
    fn no_artifact_name_this_family_can_produce_is_an_int8_one() {
        // INT8 is unrepresentable rather than refused: `FaceRecoveryVariant` carries a `FloatPrecision`, so there is
        // no value to pass that would compose `fr_athens_int8`.
        for variant in every_variant() {
            assert!(!typical(variant, two()).artifact().as_str().contains("int8"), "{variant:?}");
        }
    }

    #[test]
    fn an_operation_carries_the_faces_it_was_given_in_the_order_it_was_given_them() {
        let faces = two();
        let operation = typical(FaceRecoveryVariant::Athens(FloatPrecision::Fp32), faces.clone());

        assert_eq!(operation.faces(), &faces);
        assert_eq!(operation.faces().as_slice(), faces.as_slice());
        assert_eq!(operation.params().faces, faces);
    }

    #[test]
    fn an_empty_selection_is_accepted_rather_than_refused() {
        let operation = typical(FaceRecoveryVariant::Santorini(FloatPrecision::Fp16), Faces::empty());

        assert!(operation.faces().is_empty());
        assert!(operation.params().faces.is_empty());
    }

    #[test]
    fn two_selections_are_two_operations() {
        // A user toggling a face has changed the request, and a consumer holding the two operations is asking
        // exactly that. What toggling a face must not do is reload the weights, and it does not: a resident session
        // is filed under the artifact it was opened from and the provider it was opened on — see `crate::sessions` —
        // and every selection of one variant names the same single artifact.
        for variant in every_variant() {
            let few = typical(variant, one());
            let many = typical(variant, two());
            let none = typical(variant, Faces::empty());

            assert_ne!(few, many, "{variant:?} read two selections as one request");
            assert_ne!(few, none, "{variant:?} read an empty selection as the same request");
            assert_ne!(hash_of(&few), hash_of(&many), "{variant:?} hashed two selections alike");

            assert_eq!(
                few.required_artifacts(),
                many.required_artifacts(),
                "{variant:?} would load different weights for two selections"
            );
        }
    }

    #[test]
    fn one_selection_built_twice_is_one_operation() {
        // The other half of the property, and the one a reordered selection has to fail: equality is over the faces
        // in the order they were given.
        for variant in every_variant() {
            assert_eq!(typical(variant, two()), typical(variant, two()));
            assert_eq!(hash_of(&typical(variant, two())), hash_of(&typical(variant, two())));

            assert_ne!(
                typical(variant, two()),
                typical(variant, reversed()),
                "{variant:?} read a reordered selection as the same request"
            );
        }
    }

    #[test]
    fn an_operation_is_usable_as_a_lookup_key_across_a_differing_selection() {
        // Keyed on the whole request, selection included, so a toggled face is a different key. The weights are
        // unaffected, which is what the artifact assertion says.
        let athens = FaceRecoveryVariant::Athens(FloatPrecision::Fp16);
        let mut seen = std::collections::HashMap::new();
        seen.insert(typical(athens, one()), "requested");

        assert_eq!(seen.get(&typical(athens, one())), Some(&"requested"), "one request did not find itself");
        assert_eq!(seen.get(&typical(athens, two())), None, "a different selection found another request's entry");
        assert_eq!(
            typical(athens, one()).required_artifacts(),
            typical(athens, two()).required_artifacts(),
            "toggling a face would load different weights"
        );
    }

    #[test]
    fn two_precisions_and_two_variants_are_different_operations() {
        let fp32 = typical(FaceRecoveryVariant::Athens(FloatPrecision::Fp32), two());
        let fp16 = typical(FaceRecoveryVariant::Athens(FloatPrecision::Fp16), two());
        let santorini = typical(FaceRecoveryVariant::Santorini(FloatPrecision::Fp32), two());

        assert_ne!(fp32, fp16, "two precisions served by different artifacts compared as one model");
        assert_ne!(fp32, santorini);
    }

    #[test]
    fn a_display_name_is_the_variant_and_the_precision_whatever_the_selection() {
        let operation = typical(FaceRecoveryVariant::Athens(FloatPrecision::Fp16), two());

        assert_eq!(operation.display_name(), "Athens (FP16)");
        assert_eq!(operation.to_string(), operation.display_name());

        for faces in [Faces::empty(), one(), two(), reversed()] {
            let over = typical(FaceRecoveryVariant::Athens(FloatPrecision::Fp16), faces);

            assert_eq!(over.display_name(), "Athens (FP16)", "the selection reached the display name");
        }

        assert_eq!(
            typical(FaceRecoveryVariant::Santorini(FloatPrecision::Fp32), one()).display_name(),
            "Santorini (FP32)"
        );
    }

    #[test]
    fn the_cache_tag_is_the_written_one_rather_than_whatever_serialization_produces() {
        // Pinned here so that renaming a field, or changing how a box or a fidelity is spelled, is a failing test
        // rather than a silent invalidation of every cached image on every user's disk.
        //
        // Athens carries a `-w` segment at a fixed three decimals, ahead of the `-f` the faces write. Santorini
        // carries none, which is the whole point of the `Option`: two Santorini runs cannot be made to miss by a
        // value neither of them has.
        let athens = FaceRecoveryVariant::Athens(FloatPrecision::Fp32);
        let santorini = FaceRecoveryVariant::Santorini(FloatPrecision::Fp16);
        let half = Fidelity::new(0.5).expect("0.5 is in range");

        assert_eq!(typical(athens, Faces::empty()).cache_tag(), "fr-athens-fp32-w1.000-f0");
        assert_eq!(typical(athens, one()).cache_tag(), "fr-athens-fp32-w1.000-f12.34,56.78,90.12,34.56;");
        assert_eq!(built(athens, one(), half).cache_tag(), "fr-athens-fp32-w0.500-f12.34,56.78,90.12,34.56;");
        assert_eq!(
            typical(athens, two()).cache_tag(),
            "fr-athens-fp32-w1.000-f12.34,56.78,90.12,34.56;200.00,100.50,260.25,170.75;"
        );
        assert_eq!(typical(santorini, one()).cache_tag(), "fr-santorini-fp16-f12.34,56.78,90.12,34.56;");
    }

    #[test]
    fn two_fidelities_do_not_collide_in_the_cache() {
        // One set of weights serves every fidelity and two of them produce two images, so the tag has to tell them
        // apart: the same image at 1.0 is served from the cache, and at 0.5 is recomputed.
        let athens = FaceRecoveryVariant::Athens(FloatPrecision::Fp32);
        let mut seen = std::collections::HashSet::new();

        for value in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let fidelity = Fidelity::new(value).expect("in range");
            let tag = built(athens, two(), fidelity).cache_tag();

            assert!(seen.insert(tag.clone()), "two fidelities reported the tag {tag}");
        }

        // And the weights are not what differs, which is what makes this the tag's job rather than the artifact's.
        assert_eq!(
            built(athens, two(), Fidelity::new(0.0).expect("in range")).required_artifacts(),
            built(athens, two(), Fidelity::new(1.0).expect("in range")).required_artifacts()
        );
    }

    #[test]
    fn a_repeated_fidelity_reports_one_tag() {
        let athens = FaceRecoveryVariant::Athens(FloatPrecision::Fp32);
        let fidelity = Fidelity::new(1.0).expect("in range");

        assert_eq!(built(athens, two(), fidelity).cache_tag(), built(athens, two(), fidelity).cache_tag());

        // Two fidelities that quantize alike are one value, so they are one tag as well — the collision the
        // reference implementation has, closed before anything is rendered.
        let low = Fidelity::new(0.499_99).expect("in range");
        let high = Fidelity::new(0.500_01).expect("in range");
        assert_eq!(built(athens, two(), low).cache_tag(), built(athens, two(), high).cache_tag());
    }

    #[test]
    fn no_santorini_tag_carries_a_fidelity_segment() {
        // A fidelity cannot make a Santorini run miss, because there is none to vary.
        for precision in [FloatPrecision::Fp32, FloatPrecision::Fp16] {
            let santorini = FaceRecoveryVariant::Santorini(precision);

            for faces in [Faces::empty(), one(), two(), reversed()] {
                let tag = typical(santorini, faces).cache_tag();

                assert!(!tag.contains("-w"), "{tag} carries a fidelity segment Santorini has no value for");
            }
        }

        assert_eq!(
            typical(FaceRecoveryVariant::Santorini(FloatPrecision::Fp32), Faces::empty()).cache_tag(),
            "fr-santorini-fp32-f0"
        );
    }

    #[test]
    fn athens_reports_its_fidelity_and_santorini_reports_none() {
        let half = Fidelity::new(0.5).expect("in range");

        assert_eq!(built(FaceRecoveryVariant::Athens(FloatPrecision::Fp32), two(), half).fidelity(), Some(half));
        assert_eq!(typical(FaceRecoveryVariant::Santorini(FloatPrecision::Fp32), two()).fidelity(), None);

        // And it travels beside the faces in what one run takes, which is the seam a pipeline is built from.
        let params = built(FaceRecoveryVariant::Athens(FloatPrecision::Fp16), two(), half).params();
        assert_eq!(params.fidelity, Some(half));
        assert_eq!(params.faces, two());
    }

    #[test]
    fn two_fidelities_of_one_variant_are_two_operations() {
        // A fidelity is part of the request, so two of them are two requests — and one request built twice is one
        // operation.
        let athens = FaceRecoveryVariant::Athens(FloatPrecision::Fp32);
        let low = Fidelity::new(0.0).expect("in range");
        let high = Fidelity::new(1.0).expect("in range");

        assert_ne!(built(athens, two(), low), built(athens, two(), high));
        assert_ne!(hash_of(&built(athens, two(), low)), hash_of(&built(athens, two(), high)));
        assert_eq!(built(athens, two(), low), built(athens, two(), low));
        assert_eq!(hash_of(&built(athens, two(), low)), hash_of(&built(athens, two(), low)));
    }

    #[test]
    fn two_selections_do_not_collide_in_the_cache() {
        // The other half of the identity split: one model, but two images, so the two runs cannot be served one
        // another's picture.
        for variant in every_variant() {
            let tags = [Faces::empty(), one(), two(), reversed()].map(|faces| typical(variant, faces).cache_tag());
            let mut seen = std::collections::HashSet::new();

            for tag in tags {
                assert!(seen.insert(tag.clone()), "{variant:?} reported the tag {tag} for two distinct selections");
            }
        }
    }

    #[test]
    fn a_reordered_selection_is_a_different_cache_entry() {
        let athens = FaceRecoveryVariant::Athens(FloatPrecision::Fp32);

        assert_ne!(
            typical(athens, two()).cache_tag(),
            typical(athens, reversed()).cache_tag(),
            "two orderings of one selection shared an image"
        );
    }

    #[test]
    fn a_repeated_selection_reports_the_same_tag() {
        // Detection is deterministic for a given image, crop and precision, so a rebuilt selection has to hit the
        // cache rather than re-render.
        for variant in every_variant() {
            assert_eq!(
                typical(variant, two()).cache_tag(),
                typical(variant, two()).cache_tag(),
                "{variant:?} reported two tags for one selection"
            );
        }
    }

    #[test]
    fn a_difference_below_the_second_decimal_is_one_selection() {
        // What the two decimals are for: a re-detection that moved a box by a hundredth of a pixel must not discard
        // the cached image.
        //
        // The two selections are one **value** as well as one tag, because a coordinate is quantized when the face is
        // accepted rather than when the tag is written — so the tag agreeing is a consequence rather than a
        // coincidence the formatter arranged.
        let athens = FaceRecoveryVariant::Athens(FloatPrecision::Fp32);
        let detected = Faces::new([face_at(12.34, 56.78, 90.12, 34.56)]);
        let again = Faces::new([face_at(12.340_1, 56.780_2, 90.120_3, 34.560_4)]);

        assert_eq!(detected, again, "a sub-hundredth difference read as two selections");
        assert_eq!(
            typical(athens, detected).cache_tag(),
            typical(athens, again).cache_tag(),
            "a sub-hundredth difference discarded the cached image"
        );
    }

    #[test]
    fn the_empty_tag_is_unreachable_from_any_selection_that_carries_a_face() {
        // `f0` has to mean "no faces" and nothing else, which is what makes it worth spelling at all. A box at the
        // origin is the selection that would produce it if the spelling were not fixed-width.
        let athens = FaceRecoveryVariant::Athens(FloatPrecision::Fp32);
        let empty = typical(athens, Faces::empty()).cache_tag();
        let origin = Faces::new([face_at(0.0, 0.0, 0.0, 0.0)]);

        assert_eq!(typical(athens, origin.clone()).cache_tag(), "fr-athens-fp32-w1.000-f0.00,0.00,0.00,0.00;");

        for faces in [origin, one(), two(), reversed()] {
            assert_ne!(typical(athens, faces).cache_tag(), empty, "a selection reported the empty tag");
        }
    }

    #[test]
    fn no_cache_tag_can_be_mistaken_for_an_artifact_name() {
        for variant in every_variant() {
            for faces in [Faces::empty(), one(), two(), reversed()] {
                let tag = typical(variant, faces).cache_tag();

                assert!(!tag.contains('_'), "{tag} is spelled the way an artifact name is");
            }
        }
    }

    #[test]
    fn an_operation_round_trips_through_its_serialized_form() {
        for variant in every_variant() {
            for faces in [Faces::empty(), one(), two(), reversed()] {
                let operation = typical(variant, faces);
                let json = serde_json::to_string(&operation).expect("an operation serializes");
                let back: FaceRecovery = serde_json::from_str(&json).expect("an operation deserializes");

                assert_eq!(back, operation, "{json} did not round-trip to an equal operation");
                assert_eq!(back.faces(), operation.faces(), "{json} lost or reordered the selection");
                assert_eq!(back.cache_tag(), operation.cache_tag(), "{json} round-tripped to a different image");
            }
        }
    }

    #[test]
    fn an_empty_selection_round_trips_as_an_empty_selection() {
        // Rather than failing, or acquiring a face on the way back in — a front end persists a user's per-face
        // selection between sessions, and "none of them" is one of the things it has to persist.
        let operation = typical(FaceRecoveryVariant::Athens(FloatPrecision::Fp32), Faces::empty());
        let json = serde_json::to_string(&operation).expect("an operation serializes");
        let back: FaceRecovery = serde_json::from_str(&json).expect("an operation deserializes");

        assert!(back.faces().is_empty(), "{json} round-tripped an empty selection into something else");
        assert_eq!(back.cache_tag(), "fr-athens-fp32-w1.000-f0");
    }

    #[test]
    fn the_serialized_form_names_its_variant_rather_than_positioning_it() {
        // Tagged by name so that adding a family or a variant later cannot invalidate anything already persisted.
        let operation = typical(FaceRecoveryVariant::Athens(FloatPrecision::Fp32), Faces::empty());
        let json = serde_json::to_string(&operation).expect("an operation serializes");

        assert_eq!(json, r#"{"variant":{"codename":"athens","precision":"fp32"},"faces":[],"fidelity":1.0}"#);

        // Santorini's form gains nothing, because the field is skipped where there is nothing in it — so a
        // Santorini operation persisted before this parameter existed deserializes unchanged.
        assert_eq!(
            serde_json::to_string(&typical(FaceRecoveryVariant::Santorini(FloatPrecision::Fp32), Faces::empty()))
                .expect("an operation serializes"),
            r#"{"variant":{"codename":"santorini","precision":"fp32"},"faces":[]}"#
        );
    }

    #[test]
    fn a_serialized_operation_pairing_a_variant_with_int8_is_refused() {
        let json = r#"{"variant":{"codename":"athens","precision":"int8"},"faces":[]}"#;

        assert!(
            serde_json::from_str::<FaceRecovery>(json).is_err(),
            "deserialization produced a face recovery at a precision it is not published in"
        );
    }

    #[test]
    fn a_serialized_operation_carrying_a_face_that_could_not_have_been_constructed_is_refused() {
        let operation = typical(FaceRecoveryVariant::Athens(FloatPrecision::Fp32), one());
        let json = serde_json::to_string(&operation).expect("an operation serializes");
        let out_of_range = json.replace("\"confidence\":0.9", "\"confidence\":1.5");

        assert_ne!(out_of_range, json, "the test did not rewrite the confidence it meant to");
        assert!(
            serde_json::from_str::<FaceRecovery>(&out_of_range).is_err(),
            "deserialization produced an operation carrying a confidence outside 0.0 to 1.0"
        );
    }
}
