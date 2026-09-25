//! The AI models this application can run: naming them, validating their parameters, resolving one to the files it
//! needs, and running it.
//!
//! It holds the values a front end offers, persists and hands across a process boundary, and the inference pipeline
//! later takes as a parameter — so they describe *what* to run and own no native resource, which is what lets one
//! outlive and stay independent of any session built from it.
//!
//! All eight families are named here: [`Upscale`], [`Denoise`], [`Sharpen`], [`LightAdjustment`], [`ColorBalance`],
//! [`Colorization`], [`Detection`] and [`FaceRecovery`]. Each is its own type, so no codename can be paired with the
//! family it does not belong to.
//!
//! [`Operation`] is where that match lives. It carries an operation of any of the seven image families, one arm per
//! family holding that family's own type, and forwards the five questions they all answer; a detection run is
//! carried by [`Analysis`] instead.
//!
//! Two of them break the mould the other six share. [`Detection`] is the one family whose result is not an image: a
//! run produces a [`DetectionOutput`], the set of [`Face`]s found. [`FaceRecovery`] is the one whose per-run input is
//! not a scalar: it carries the [`Faces`] it is to restore, supplied by a detection run rather than adjusted through
//! a control, which is why it is `Clone` rather than `Copy`.
//!
//! # An operation is the request it describes
//!
//! There is no operation-id string. An operation compares and hashes as a value, so it can be a lookup key directly,
//! dispatch is an exhaustive match, and a display name is asked for rather than parsed back out of an id. The one
//! string that survives is the cache tag — [`Upscale::cache_tag`] and its counterpart on each of the other seven —
//! because the run cache persists to disk and has to outlive the process.
//!
//! **Every field is in the identity.** Two operations are equal exactly when they name the same model at the same
//! precision and carry the same per-run parameters: Upscale's [`Scale`], the [`Strength`] denoise and sharpen carry,
//! the [`Bias`] light adjustment and colour balance carry, the [`Fidelity`] Athens carries and the [`Faces`] face
//! recovery carries each distinguish two operations. That is the question a consumer holding two of them has — did
//! the user change anything — and it is not the question of which weights to load. [`Colorization`] and
//! [`Detection`] carry no per-run parameter at all, rather than an unused one defaulted to something neutral.
//!
//! **Identity does not decide what is loaded.** A resident session is filed under the artifact it was opened from
//! and the provider it was opened on — see [`crate::sessions`] — so every operation needing one graph shares it: an
//! 8x Kyoto and a 4x Kyoto both open `up_kyoto_4x_fp16`, two Osaka scales share one set of weights, and dragging a
//! slider or toggling a face never loads a second copy of weights already resident. `cache_tag` answers the third,
//! separate question: whether two runs produce the same image.
//!
//! # What is published is what can be named
//!
//! A precision travels inside the variant, in the domain that variant is actually published in — so Osaka at FP32 is
//! not a refused request but an unwritable one. A [`Scale`], a [`Strength`] and a [`Bias`] are each bounded and
//! quantized on acceptance, so a value that exists is a value in range, and two requests that would have collided in
//! the cache are one value before anything is rendered. [`fn@catalogue`] publishes both facts for all eight families,
//! built from the same rows and constants that enforce them, so a control cannot offer what resolution would refuse.
//! A [`Confidence`] is bounded the same way and for a related reason — a face that exists is a face whose confidence
//! is meaningful — and quantized too, to two decimal places, because it travels inside the [`Faces`] a face recovery
//! carries and so has to compare and hash as exactly as any other field of an identity.

// Where a file belongs: three tiers, and the test for which one a file is at is who else could use it.
//
// - **One model** — `models/<family>/<model>/`, as `upscale/tokyo/`, `upscale/kyoto/`, `upscale/saitama/` and
//   `upscale/osaka/` are. Its row, the execution-provider profile measured for it and the prose behind it, its
//   pipeline, the vocabulary only its own contract needs, and its tests. Deleting a model is deleting its directory
//   plus the arms that name it, and nothing else.
// - **One family** — a file directly under `models/<family>/`, shared by two or more of that family's models and by
//   nothing outside it: `upscale/passes.rs`, which all three convolutional models run, and `upscale/conv.rs`, which
//   all three are rows of.
// - **Every family** (cross-family tier) — shared by two or more families, and by nothing that is not a model. Four
//   homes, by what the thing is:
//   - this module, for the vocabulary all eight speak — the carriers, `Subject`, the parameters, the catalogue;
//   - a file directly under `models/`, for model code two families run — `filter.rs`, the tiled filter contract
//     denoise and sharpen both run;
//   - `crate::pipeline`, for the contract every model is written against — the pipeline traits, `Backend` and the
//     session seam;
//   - the `imaging` crate, for the pixel primitives no model owns — the tile grid, tensor conversion, padding,
//     blending, the fixed-canvas presentation (`imaging::present`) and the blend by a run's control
//     (`imaging::mix`), which light adjustment and colour balance both run.
//
// **Nothing under `models/` names `crate::inference`**, in code or in prose. `inference` is the orchestrator: it plans
// and runs what `models` describes and implements, so it depends on `models` and never the other way round. A model
// that needs something `inference` holds has found an item at the wrong tier — move the item down to one of the four
// homes above; do not import upwards.
//
// A tier is decided by a file's callers and re-decided every time one is added: a second model of the family needing
// a model-tier file promotes it to family tier, and a caller from a second family promotes it to cross-family tier,
// in both cases before the new caller is written. A `use` reaching from one model's directory into another's is a
// promotion that has not happened yet, and copying is the alternative that ends in two versions disagreeing.
//
// Parked at model tier, and expected to be promoted. Each of these has exactly one caller today, which is why it is
// where it is, and each is written so that gaining a second is a **file move and an import change** rather than a
// rewrite: none of them names a variant, and each takes the sizes and the inputs its caller's geometry would differ
// in as parameters. The list is here so that whoever adds the second caller knows what to move before writing it.
//
// - `upscale/osaka/colorfix.rs` — the wavelet colour fix for one-step diffusion drift.
// - `upscale/osaka/graph.rs` — role-addressed multi-graph resolution, which is a contract-level idea.
// - `upscale/osaka/latent.rs` — VAE stride, patchify and the one-step scheduler.
// - `upscale/osaka/noise.rs` — deterministic seeded per-region noise.
// - `upscale/osaka/region.rs` — the three-graph region loop.
//
// The second diffusion model in any family promotes Osaka's five, and does not reimplement them.
//
// A family exposes its models through exactly two seams, and neither ever reaches the chain driver: one match from
// the variant enum to that family's model, one arm per model; and one match from the resolved contract to a
// pipeline, one arm per contract. The upscale family is where both stand today — `UpscaleVariant::model` is the
// first and `Upscale::pipeline` the second. A new model is a directory and one arm in the first; a new contract
// within a family is one arm in the second.
//
// A pipeline underneath this module does not contradict its values owning no native resource: it is reached only
// through `crate::pipeline::ImagePipeline`, and no value named here holds one.
//
// Each family is its own type rather than one shared shape tagged with a family, so dispatch is a match the compiler
// checks.

pub(crate) mod artifact;
pub(crate) mod bias;
pub(crate) mod build;
pub(crate) mod catalogue;
pub(crate) mod color_balance;
pub(crate) mod colorization;
pub(crate) mod denoise;
pub(crate) mod detection;
pub(crate) mod face;
pub(crate) mod face_recovery;
pub(crate) mod fidelity;
pub(crate) mod filter;
pub(crate) mod light_adjustment;
pub(crate) mod operation;
pub(crate) mod precision;
mod quantized;
pub(crate) mod scale;
pub(crate) mod sharpen;
pub(crate) mod simple;
pub(crate) mod strength;
pub(crate) mod subject;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod upscale;

pub use artifact::{ArtifactId, Family};
pub use bias::Bias;
pub use build::{BuildError, ParameterValues};
pub use catalogue::{FamilyEntry, ParameterEntry, ParameterKind, VariantEntry, catalogue};
pub use color_balance::{ColorBalance, ColorBalanceParams, ColorBalanceVariant};
pub use colorization::{Colorization, ColorizationVariant};
pub use denoise::{Denoise, DenoiseParams, DenoiseVariant};
pub use detection::{Detection, DetectionVariant};
pub use face::{Confidence, DetectionOutput, Face, Faces, Point, Rect};
pub use face_recovery::{FaceRecovery, FaceRecoveryParams, FaceRecoveryVariant};
pub use fidelity::Fidelity;
pub use light_adjustment::{LightAdjustment, LightAdjustmentParams, LightAdjustmentVariant};
pub use operation::{Analysis, DataOperation, Operation};
pub use precision::{FloatPrecision, Precision};
pub use scale::{RangeError, Scale};
pub use sharpen::{Sharpen, SharpenParams, SharpenVariant};
pub use strength::Strength;
pub use subject::Subject;
pub(crate) use upscale::osaka::graph::ResolvedGraph;
pub use upscale::osaka::graph::{GraphRole, GraphSet, UnknownRole};
pub use upscale::osaka::precision::OsakaPrecision;
pub use upscale::resolve::{Pass, Resolution};
pub use upscale::{Upscale, UpscaleParams, UpscaleVariant};

#[cfg(test)]
mod tests {
    use super::test_support::{every_artifact_name, every_cache_tag, every_carried_family};
    use super::*;

    #[test]
    fn the_cross_family_set_carries_every_family_the_library_names() {
        // The premise the two tests below rest on: a collision between two families is only visible in a set that
        // holds both. Asked of the operations themselves rather than counted, so a family left out of the spread
        // fails here by name instead of weakening the checks silently.
        //
        // Across **both** carriers, which is what makes "every family" true: the seven whose result is an image and the
        // one whose result is not.
        let carried: std::collections::HashSet<Family> = every_carried_family().into_iter().collect();

        for family in Family::ALL {
            assert!(carried.contains(&family), "{family:?} is missing from the cross-family set");
        }
    }

    #[test]
    fn no_two_operations_across_the_eight_families_share_a_cache_tag() {
        // The run cache is one namespace shared by every family, so distinctness within a family is not enough: two
        // families whose prefixes or parameter letters collided would serve one another's images, and the failure
        // would be a wrong picture rather than an error. One namespace means one set, across both carriers.
        let mut seen = std::collections::HashSet::new();

        for tag in every_cache_tag() {
            assert!(seen.insert(tag.clone()), "two operations share the cache tag {tag}");
        }
    }

    #[test]
    fn no_cache_tag_is_spelled_the_way_any_published_artifact_name_is() {
        // Two namespaces that must not meet: artifact names are underscore-separated and name files on disk and URLs
        // on the remote, while cache tags are hyphen-separated and key cached images. A tag that could be read as an
        // artifact name is what would let a cache entry be mistaken for a model, or the reverse.
        let names = every_artifact_name();

        assert!(!names.is_empty(), "no artifact names were collected, so this test would pass vacuously");

        for tag in every_cache_tag() {
            assert!(!tag.contains('_'), "the cache tag {tag} is spelled the way an artifact name is");
            assert!(!names.contains(&tag), "the cache tag {tag} is also a published artifact name");
        }
    }

    #[test]
    fn the_quantized_parameters_keep_their_rendered_and_serialized_spellings() {
        // Pinned as literals across all four bounded parameters: the rendering is what every cache tag carrying one
        // spells, and the serialized number is what a front end and a persisted setting read back, so a change to
        // either invalidates cached images or saved settings rather than failing loudly.
        fn json(value: &impl serde::Serialize) -> String {
            serde_json::to_string(value).expect("a bounded parameter serializes")
        }

        let pins = |display: String, json: String, expected: (&str, &str)| {
            assert_eq!((display.as_str(), json.as_str()), expected);
        };

        for (value, expected) in [
            (1.0, ("1", "1.0")),
            (1.5, ("1.5", "1.5")),
            (1.25, ("1.25", "1.25")),
            (1.666_61, ("1.667", "1.667")),
            (2.05, ("2.05", "2.05")),
            (8.0, ("8", "8.0")),
        ] {
            let scale = Scale::new(value).expect("in range");
            pins(scale.to_string(), json(&scale), expected);
        }

        for (value, expected) in [
            (0.0, ("0", "0.0")),
            (0.001, ("0.001", "0.001")),
            (0.5, ("0.5", "0.5")),
            (1.0, ("1", "1.0")),
            (2.25, ("2.25", "2.25")),
            (1.234_56, ("1.235", "1.235")),
            (3.0, ("3", "3.0")),
        ] {
            let strength = Strength::new(value).expect("in range");
            pins(strength.to_string(), json(&strength), expected);
        }

        for (value, expected) in [
            (-1.0, ("-1", "-1.0")),
            (-0.5, ("-0.5", "-0.5")),
            (-0.35, ("-0.35", "-0.35")),
            (-0.001, ("-0.001", "-0.001")),
            (-0.0, ("0", "0.0")),
            (0.0, ("0", "0.0")),
            (0.123_41, ("0.123", "0.123")),
            (0.1, ("0.1", "0.1")),
            (1.0, ("1", "1.0")),
        ] {
            let bias = Bias::new(value).expect("in range");
            pins(bias.to_string(), json(&bias), expected);
        }

        for (value, expected) in [
            (0.0, ("0.000", "0.0")),
            (0.05, ("0.050", "0.05")),
            (0.5, ("0.500", "0.5")),
            (0.123_456, ("0.123", "0.123")),
            (1.0, ("1.000", "1.0")),
        ] {
            let fidelity = Fidelity::new(value).expect("in range");
            pins(fidelity.to_string(), json(&fidelity), expected);
        }

        // And as the cache tags they end up in, one per parameter letter.
        let strength = Strength::new(0.25).expect("in range");
        let bias = Bias::new(-0.35).expect("in range");
        let tags = [
            Upscale::tokyo(FloatPrecision::Fp32, Scale::new(2.5).expect("in range")).cache_tag(),
            Denoise::stockholm(FloatPrecision::Fp16, strength).cache_tag(),
            Sharpen::moscow(FloatPrecision::Fp32, strength).cache_tag(),
            LightAdjustment::paris(FloatPrecision::Fp32, bias).cache_tag(),
            ColorBalance::rio(FloatPrecision::Fp16, bias).cache_tag(),
            FaceRecovery::athens(FloatPrecision::Fp32, Faces::empty(), Fidelity::new(0.05).expect("in range"))
                .cache_tag(),
        ];

        assert_eq!(
            tags,
            [
                "up-tokyo-fp32-s2.5",
                "dn-stockholm-fp16-t0.25",
                "sh-moscow-fp32-t0.25",
                "la-paris-fp32-b-0.35",
                "cb-rio-fp16-b-0.35",
                "fr-athens-fp32-w0.050-f0",
            ]
        );
    }
}
