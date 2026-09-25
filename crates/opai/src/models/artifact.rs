//! Which enhancement family a model belongs to, and the name of one file it resolves to.
//!
//! Not to be confused with `crate::deps::artifact`, which pins the dependency *archives* the application downloads at
//! startup. This names a model graph published on HuggingFace.

use serde::{Deserialize, Serialize};

use super::precision::Precision;

/// One enhancement family, keyed by the two-letter prefix every one of its artifacts is named with.
///
/// The prefix is what an artifact name is composed from, so a family added later must not be able to reuse one
/// already taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Family {
    /// Face detection, which the families that work on faces depend on to find them.
    Detection,
    /// Noise removal.
    Denoise,
    /// Face recovery.
    FaceRecovery,
    /// Colourization of a monochrome image.
    Colorization,
    /// Light adjustment.
    LightAdjustment,
    /// Colour balance.
    ColorBalance,
    /// Sharpening.
    Sharpen,
    /// Upscaling.
    Upscale,
}

impl Family {
    /// Every family the library names, in declaration order.
    ///
    /// The one list, so a family added later is covered by everything that iterates families rather than by whichever
    /// of them its author remembered to extend. Not the order the catalogue publishes — that one is a presentation
    /// decision and is pinned where it is made.
    pub const ALL: [Self; 8] = [
        Self::Detection,
        Self::Denoise,
        Self::FaceRecovery,
        Self::Colorization,
        Self::LightAdjustment,
        Self::ColorBalance,
        Self::Sharpen,
        Self::Upscale,
    ];

    /// The two-letter prefix this family's artifact names begin with: `up`.
    ///
    /// Fixed by the files already published, so these are transcribed rather than derived from the variant names.
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Detection => "dt",
            Self::Denoise => "dn",
            Self::FaceRecovery => "fr",
            Self::LightAdjustment => "la",
            Self::ColorBalance => "cb",
            Self::Colorization => "cl",
            Self::Sharpen => "sh",
            Self::Upscale => "up",
        }
    }

    /// This family's name as a reader is shown it: `Light Adjustment`.
    ///
    /// Transcribed rather than derived from the variant's `Debug` rendering, which is what a consumer needing this
    /// would otherwise have to do. `Debug` is not an API: a renamed arm, or a family whose name is not strictly
    /// CamelCase, would silently change or mangle a name a user reads — and every consumer that wanted one would be
    /// re-deriving it, which is the duplication [`catalogue`](fn@super::catalogue) exists to prevent.
    ///
    /// The variant level already carries its own `label`; this is the same thing one level up.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Detection => "Detection",
            Self::Denoise => "Denoise",
            Self::FaceRecovery => "Face Recovery",
            Self::LightAdjustment => "Light Adjustment",
            Self::ColorBalance => "Color Balance",
            Self::Colorization => "Colorization",
            Self::Sharpen => "Sharpen",
            Self::Upscale => "Upscale",
        }
    }
}

/// The name of one model graph, as it is published: `up_kyoto_4x_fp32`.
///
/// A **stem**, carrying no file extension. One artifact is often several files — a large model keeps its weights in a
/// sibling `.onnx.data` — and which ones have external weights is discovered from the remote file listing by whoever
/// downloads them, not predicted here.
///
/// A newtype rather than a `String` because it is composed from parts in one place and consumed as an opaque name
/// everywhere else: there is no valid reason for a caller to build one by concatenation, and nothing else in this
/// vocabulary is a string it could be confused with.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArtifactId(String);

impl ArtifactId {
    /// Composes the name of the artifact serving one pass or one graph.
    ///
    /// `name` is the variant's codename, plus any suffix distinguishing one graph of a multi-stage variant from its
    /// base model — `osaka`, `osaka_vae_encoder`. `native_scale` is the scale of the weights themselves, present only
    /// where a family publishes one set per scale, and is an integer so that a whole scale renders `4x` rather than
    /// `4.0x`. It is emphatically **not** the scale that was requested: the two differ whenever a request falls
    /// between native scales, and naming an artifact from the request would name a file that was never published.
    pub(crate) fn new(family: Family, name: &str, native_scale: Option<u8>, precision: Precision) -> Self {
        let prefix = family.prefix();

        Self(match native_scale {
            Some(scale) => format!("{prefix}_{name}_{scale}x_{precision}"),
            None => format!("{prefix}_{name}_{precision}"),
        })
    }

    /// Composes the name of one graph of a multi-stage variant: `up_osaka_vae_encoder_fp16`.
    ///
    /// Separate from [`ArtifactId::new`] rather than a further argument to it, because only the diffusion variants
    /// have a suffix and `new` already carries one parameter that is `None` for every family but this one. Composing
    /// here rather than at the call site is what keeps it to a single allocation.
    pub(crate) fn suffixed(family: Family, codename: &str, suffix: &str, precision: Precision) -> Self {
        Self(format!("{}_{codename}{suffix}_{precision}", family.prefix()))
    }

    /// The name as published, for whoever downloads or opens it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_family_renders_its_documented_prefix() {
        // Transcribed from the names already published; a change to any of these renames every artifact of a family.
        assert_eq!(Family::Detection.prefix(), "dt");
        assert_eq!(Family::Denoise.prefix(), "dn");
        assert_eq!(Family::FaceRecovery.prefix(), "fr");
        assert_eq!(Family::LightAdjustment.prefix(), "la");
        assert_eq!(Family::ColorBalance.prefix(), "cb");
        assert_eq!(Family::Colorization.prefix(), "cl");
        assert_eq!(Family::Sharpen.prefix(), "sh");
        assert_eq!(Family::Upscale.prefix(), "up");
    }

    #[test]
    fn every_family_has_a_prefix_of_its_own() {
        let mut prefixes: Vec<_> = Family::ALL.iter().map(|family| family.prefix()).collect();
        prefixes.sort_unstable();
        let count = prefixes.len();
        prefixes.dedup();

        assert_eq!(prefixes.len(), count, "two families share a prefix, so their artifact names could collide");
    }

    #[test]
    fn a_scaled_artifact_name_carries_its_native_scale() {
        let id = ArtifactId::new(Family::Upscale, "kyoto", Some(4), Precision::Fp32);
        assert_eq!(id.as_str(), "up_kyoto_4x_fp32");
    }

    #[test]
    fn an_artifact_name_carries_no_decimal_part_for_a_whole_native_scale() {
        // The native scale is an integer, so `4.0x` — which names nothing — has no way to be written.
        let id = ArtifactId::new(Family::Upscale, "tokyo", Some(4), Precision::Fp16);
        assert!(id.as_str().contains("_4x_"), "{id} did not carry the native scale as a whole number");
        assert!(!id.as_str().contains("_4.0x_"));
    }

    #[test]
    fn an_unscaled_artifact_name_omits_the_scale_segment() {
        let id = ArtifactId::new(Family::Upscale, "osaka_vae_encoder", None, Precision::Fp16);
        assert_eq!(id.as_str(), "up_osaka_vae_encoder_fp16");
    }

    #[test]
    fn an_artifact_name_is_a_stem_carrying_no_file_extension() {
        let id = ArtifactId::new(Family::Upscale, "osaka", None, Precision::Int8);
        assert_eq!(id.as_str(), "up_osaka_int8");
        assert!(
            !id.as_str().contains('.'),
            "{id} carried an extension, which the download path decides rather than this"
        );
    }

    #[test]
    fn display_renders_the_published_name() {
        let id = ArtifactId::new(Family::Upscale, "kyoto", Some(2), Precision::Fp32);
        assert_eq!(id.to_string(), id.as_str());
    }
}
