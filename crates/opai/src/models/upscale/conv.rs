//! The convolutional upscale contract: the scale table its models are built from, and what one resolves to.

// Family tier rather than model tier, because all three of Tokyo, Kyoto and Saitama are rows of `ConvVariant` and two
// of them share `FOUR_ONLY` outright. Parking either in a model's directory would make the other's row import across a
// model boundary.

use super::super::artifact::{ArtifactId, Family};
use super::super::scale::Scale;
use super::resolve::{Pass, Resolution};
use super::variant::{UpscaleModel, UpscaleVariant};

use crate::models::catalogue::{ParameterEntry, SCALE_PARAMETER};
use crate::models::precision::{FloatPrecision, Precision};
use crate::providers::profile::EpProfile;
pub(crate) use imaging::TileGeometry;
use imaging::tensor::Normalisation;

/// One entry of a convolutional variant's scale table: the largest request this entry covers, and the native passes
/// that cover it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScaleBucket {
    // In `Scale`'s quantization steps so that a request compares against it exactly, with no floating-point comparison
    // standing between a requested scale and the weights it selects.
    /// The largest requested scale this bucket covers, in quantization steps.
    max_steps: u16,
    /// The native scales to run, in order. Integers, because they name the weights: a pass is `4x` or `2x` and never
    /// anything between.
    passes: &'static [u8],
}

impl ScaleBucket {
    /// A bucket covering requests up to `max` whole times, served by `passes`.
    pub(crate) const fn new(max: u8, passes: &'static [u8]) -> Self {
        // `max` is a whole number because every bound the published weights give is one; a bucket boundary at 2.5x
        // would mean a native 2.5x model, which does not exist.
        Self { max_steps: max as u16 * 1000, passes }
    }

    /// Whether this bucket covers `scale`.
    pub(crate) const fn covers(self, scale: Scale) -> bool {
        scale.steps() <= self.max_steps
    }

    /// The native scales to run, in order.
    pub(crate) const fn passes(self) -> &'static [u8] {
        self.passes
    }

    /// The largest requested scale this bucket covers, as a number.
    #[cfg(test)]
    const fn max(self) -> f64 {
        self.max_steps as f64 / 1000.0
    }
}

/// The tensor shape a convolutional model's passes run at: the tile geometry and the range it was trained against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PassShape {
    // A pair rather than two accessors because they travel together everywhere — from the row, through the pipeline,
    // to the one `run_tiled` call — and neither is meaningful to a caller that does not have the other.
    /// The tile shape this model accepts, and how far each tile is asked to overlap the one before it.
    pub(crate) tiles: TileGeometry,
    /// The input range this model was trained against.
    pub(crate) range: Normalisation,
}

/// One convolutional upscale variant, as data.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ConvVariant {
    // Lower-case ASCII because it ends up in an artifact name, a download URL and a cache directory name, each of
    // which would escape anything else differently.
    /// The developer-facing identifier, and the only name the library itself composes anything from. Lower-case
    /// ASCII with no accents, spaces or punctuation.
    pub(crate) codename: &'static str,
    /// The display text a user sees, and which nothing is ever composed from. Free to carry accents, spaces and
    /// punctuation; for this family it differs from the codename only in capitalisation.
    pub(crate) label: &'static str,
    /// Which native passes cover which requests. Ordered by ascending bound, and the first bucket that covers a
    /// request serves it.
    pub(crate) buckets: &'static [ScaleBucket],
    // **This is the signature the reference implementation uses, and it is not the same decision.** The reference
    // needs `func(Precision) EPProfile` because a Go variant cannot carry its own precision, so the function is how the
    // precision reaches the measurement at all — and it pairs that with an `Fp16Only` wrapper to keep a measurement
    // that holds at one precision from being applied at the other. Here the precision is inside the variant, that
    // pairing is enforced by construction, and the function carries something else entirely: which model's
    // measurement this is. **No wrapper is adopted here, and none should be added.**
    /// The execution-provider tuning measured for this model, at the precision an operation carries. See the model's
    /// own directory for the measurements and the prose recording how they were arrived at.
    pub(crate) profile: fn(Precision) -> EpProfile,
    // A function per row, like `profile`: this type is the whole contract, so three models share it, and the arm is
    // the one fact that differs per model rather than per contract.
    /// Which arm of [`UpscaleVariant`] this row is, given the precision it carries. In practice every row passes the
    /// enum's own tuple constructor.
    pub(crate) variant: fn(FloatPrecision) -> UpscaleVariant,
    // On the row rather than defaulted by the pass driver: the range is the model's property (see `imaging::tensor`),
    // and the geometry is the model's for the same reason — the published weights are frozen at it. A model trained at
    // another tile size or against `[-1, 1]` is then a row that says so rather than an edit to the pass driver.
    /// The tensor shape this model's passes run at. A wrong answer here is a worse image rather than an error.
    pub(crate) shape: PassShape,
}

/// The bucket table shared by every variant publishing native 4x weights alone: one pass up to 4x, two beyond it.
pub(crate) const FOUR_ONLY: &[ScaleBucket] = &[ScaleBucket::new(4, &[4]), ScaleBucket::new(8, &[4, 4])];

impl UpscaleModel for ConvVariant {
    fn codename(&self) -> &'static str {
        self.codename
    }

    fn label(&self) -> &'static str {
        self.label
    }

    fn precisions(&self) -> &'static [Precision] {
        &FloatPrecision::PRECISIONS
    }

    fn parameters(&self) -> &'static [ParameterEntry] {
        &SCALE_PARAMETER
    }

    fn profile(&self, precision: Precision) -> EpProfile {
        (self.profile)(precision)
    }

    fn pass_shape(&self) -> Option<PassShape> {
        Some(self.shape)
    }

    fn variant(&self, precision: Precision) -> Option<UpscaleVariant> {
        FloatPrecision::narrow(precision).map(self.variant)
    }

    /// The first bucket covering `scale`, as one [`Pass`] per native scale it names.
    ///
    /// Never empty: the tables are checked to reach the family's maximum with no gap, so no accepted scale falls past
    /// the last bucket.
    fn resolve(&self, precision: Precision, scale: Scale) -> Resolution {
        let passes = self
            .buckets
            .iter()
            .find(|bucket| bucket.covers(scale))
            .map(|bucket| bucket.passes())
            .unwrap_or_default()
            .iter()
            .map(|&native| Pass {
                scale: native,
                artifact: ArtifactId::new(Family::Upscale, self.codename, Some(native), precision),
            })
            .collect();

        Resolution::Passes(passes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::upscale::kyoto::KYOTO;
    use crate::models::upscale::saitama::SAITAMA;
    use crate::models::upscale::tokyo::TOKYO;

    /// Every convolutional row, for the tests that must hold across all of them.
    const ALL: [&ConvVariant; 3] = [&KYOTO, &TOKYO, &SAITAMA];

    #[test]
    fn every_bucket_table_is_ordered_by_ascending_bound() {
        // Resolution takes the first bucket that covers a request, so an unordered table would serve a small request
        // from a large bucket — an 8x pass sequence for a 1.5x request, correct in output and twice the work.
        for variant in ALL {
            let bounds: Vec<f64> = variant.buckets.iter().map(|bucket| bucket.max()).collect();
            let mut sorted = bounds.clone();
            sorted.sort_by(f64::total_cmp);
            sorted.dedup();

            assert_eq!(bounds, sorted, "{}'s buckets are not in strictly ascending order", variant.codename);
        }
    }

    #[test]
    fn every_bucket_table_reaches_the_families_maximum_with_no_gap() {
        // The table is what makes a pass sequence non-empty, so a bound short of the maximum would leave the top of
        // the accepted range resolving to nothing — which downstream is a plain resize presented as an upscale.
        for variant in ALL {
            let first = variant.buckets.first().expect("a variant with no buckets covers nothing");
            let last = variant.buckets.last().expect("a variant with no buckets covers nothing");

            assert!(
                first.max() >= Scale::MIN,
                "{}'s smallest bucket does not reach the minimum scale",
                variant.codename
            );
            assert_eq!(
                last.max(),
                Scale::MAX,
                "{}'s largest bucket stops short of the maximum scale",
                variant.codename
            );
        }
    }

    #[test]
    fn every_bucket_names_at_least_one_pass() {
        for variant in ALL {
            for bucket in variant.buckets {
                assert!(
                    !bucket.passes().is_empty(),
                    "{} has a bucket covering a request with no passes",
                    variant.codename
                );
            }
        }
    }

    #[test]
    fn kyoto_uses_its_native_two_times_weights_where_the_others_repeat_four() {
        assert_eq!(
            KYOTO.buckets.iter().map(|bucket| bucket.passes()).collect::<Vec<_>>(),
            vec![&[2][..], &[4][..], &[4, 2][..]]
        );
        assert_eq!(
            TOKYO.buckets.iter().map(|bucket| bucket.passes()).collect::<Vec<_>>(),
            vec![&[4][..], &[4, 4][..]]
        );
        assert_eq!(
            SAITAMA.buckets.iter().map(|bucket| bucket.passes()).collect::<Vec<_>>(),
            vec![&[4][..], &[4, 4][..]]
        );
    }

    #[test]
    fn every_codename_is_the_lower_case_ascii_its_filenames_need() {
        for variant in ALL {
            assert!(
                variant.codename.chars().all(|c| c.is_ascii_lowercase()),
                "{} is not spelled the way an artifact name, a URL and a directory name all accept",
                variant.codename
            );
            assert!(!variant.label.is_empty(), "{} has no display label", variant.codename);
        }
    }

    #[test]
    fn a_bucket_covers_every_request_up_to_its_bound_and_nothing_past_it() {
        let bucket = ScaleBucket::new(2, &[2]);

        assert!(bucket.covers(Scale::new(1.0).expect("in range")));
        assert!(bucket.covers(Scale::new(2.0).expect("in range")));
        assert!(!bucket.covers(Scale::new(2.001).expect("in range")));
    }
}
