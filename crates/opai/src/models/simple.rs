//! The shape the seven single-artifact families share: what one of their variants is, and the name it resolves to.
//!
//! Upscale needs a row per contract because its variants differ in how many artifacts they resolve to and in what
//! selects their weights. These sixteen differ in nothing but a codename, a label and what they take, so one row
//! type and one composition serve all seven families — and what stays per-family is the variant enum, which is the part that would
//! actually diverge if a family later gained a second graph or a third precision.

use super::artifact::{ArtifactId, Family};
use super::catalogue::ParameterEntry;
use super::precision::Precision;

/// One variant of a family published as a single graph per precision.
///
/// The same split [`ConvVariant`](super::upscale::conv::ConvVariant) makes between the two names, for the same reason: only one
/// of them is ever composed into anything.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SimpleVariant {
    /// The developer-facing identifier, and the only name the library composes anything from. Lower-case ASCII with
    /// no accents, spaces or punctuation, because it ends up in an artifact name, a download URL and a cache
    /// directory name, each of which would escape anything else differently — which is why São Paulo's row pairs the
    /// codename `saopaulo` with an accented label.
    pub(crate) codename: &'static str,
    /// The display text a user sees, and which nothing is ever composed from. Free to carry accents, spaces and
    /// punctuation.
    pub(crate) label: &'static str,
    /// What this variant takes, as the catalogue publishes it.
    ///
    /// **On the row rather than on the family**, because the two are not the same question: six families happen to
    /// give every variant the same answer, and face recovery does not — Athens takes a fidelity its sibling has
    /// nowhere to put. Writing it here for all seven is what keeps that difference from being a branch in the
    /// catalogue, and what makes a new variant state what it takes or fail to compile.
    pub(crate) parameters: &'static [ParameterEntry],
}

/// The artifact serving one variant of a single-graph family at one precision: `dn_stockholm_fp32`.
///
/// No scale segment, because none of these families publishes one set of weights per scale — the segment exists for
/// the convolutional upscalers alone, and passing `None` here is what says so.
pub(crate) fn single_artifact(family: Family, codename: &str, precision: Precision) -> ArtifactId {
    ArtifactId::new(family, codename, None, precision)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_variant_of_each_family_composes_the_name_its_artifact_is_published_under() {
        // One per family, transcribed from the reference implementation's `models/ids_test.go`, which pins these as
        // literals because they key the on-disk cache and name the downloaded files. Each family module pins its own
        // full set; what is checked here is the composition every one of them goes through.
        let composed = [
            (Family::Denoise, "stockholm", Precision::Fp32, "dn_stockholm_fp32"),
            (Family::Sharpen, "moscow", Precision::Fp16, "sh_moscow_fp16"),
            (Family::LightAdjustment, "paris", Precision::Fp32, "la_paris_fp32"),
            (Family::ColorBalance, "saopaulo", Precision::Fp16, "cb_saopaulo_fp16"),
            (Family::Colorization, "delhi", Precision::Fp32, "cl_delhi_fp32"),
        ];

        for (family, codename, precision, expected) in composed {
            assert_eq!(single_artifact(family, codename, precision).as_str(), expected);
        }
    }

    #[test]
    fn a_composed_name_carries_no_scale_segment() {
        // A scale segment here would name a file that was never published: these families export one set of weights,
        // not one per factor.
        let id = single_artifact(Family::Denoise, "gothenburg", Precision::Fp16);

        assert_eq!(id.as_str(), "dn_gothenburg_fp16");
        assert!(!id.as_str().contains('x'), "{id} carried a scale segment");
    }
}
