//! The upscale factor an operation is run at, and the error an out-of-range one produces.

use serde::{Deserialize, Serialize};

// Shared by every ranged parameter rather than written once per type: the message differs only in the parameter's
// name and its bounds, and a caller that renders one renders them all.
//
// The types that produce one — `Scale`, `Strength`, `Bias`, `Fidelity` and `Confidence` — each write out the same
// `new`/`get`/`quantize`/`TryFrom`/`From` mechanism, the first four a `clamped` besides, and that is deliberate rather
// than an oversight waiting to be folded. A `bounded_parameter!` macro generating all five was considered and rejected:
// what differs between them is not the mechanism but the argument for it — why this one is signed, why that one's NaN
// falls back to its minimum and this one's to zero, which reference formatting each rounding rule reproduces — and that
// argument lives beside each method. A macro keeps the five lines of code in step at the price of the several hundred
// words that say what they are for, which is the wrong trade in this direction.
/// A parameter that was supplied outside the range its family accepts.
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[error("{parameter} must be between {min} and {max} inclusive, but {value} was supplied")]
pub struct RangeError {
    /// The parameter's name, as a message shows it: `scale`.
    pub parameter: &'static str,
    /// The value that was refused. A non-finite value is refused like any other out-of-range one.
    pub value: f64,
    /// The lowest value the parameter accepts.
    pub min: f64,
    /// The highest value the parameter accepts.
    pub max: f64,
}

// Three decimal places, which for the 1-8 range is exactly what the four significant figures the reference
// implementation formats its ids with expresses — so every cache tag this produces is the one it produces, character
// for character.
/// How many quantization steps one whole unit of scale is divided into.
const STEPS_PER_UNIT: f64 = 1000.0;

/// The factor an upscale operation enlarges an image by.
///
/// Bounded to [`Scale::MIN`]x-[`Scale::MAX`]x inclusive, and quantized to three decimal places on acceptance. A scale
/// of 1x is meaningful rather than a no-op: the model restores detail at the image's original size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Scale {
    // Stored as an integer count of quantization steps rather than as an `f64`, which is forced rather than stylistic.
    // An operation derives `Hash` and `Eq` so it can be a lookup key, and `f64` implements neither — its
    // `PartialEq` is unsound as an identity anyway, since `NaN` compares unequal to itself and `-0.0 == 0.0` while the
    // two have different bits. The integer makes the comparison exact, the hashing sound and the rendering lossless
    // from one decision.
    //
    // Quantizing also closes a collision the reference implementation has. It renders the scale into the id with
    // `%.4g` and uses that id as the run cache key, so `1.66661x` and `1.66669x` produce one string, collide, and the
    // second request is served the first one's image. Here the two are the same `Scale` before anything is rendered,
    // so they are the same operation rather than two operations sharing a key.
    /// The quantized factor in steps of 1/[`STEPS_PER_UNIT`], always within `1000..=8000`.
    steps: u16,
}

impl Scale {
    // A property of the family rather than of any one model: a variant that disagreed would let a control offer a scale
    // its siblings refuse. The catalogue publishes this same constant, so a slider and this constructor cannot
    // disagree about what is offerable.
    /// The smallest factor any upscale variant accepts.
    pub const MIN: f64 = 1.0;

    /// The largest factor any upscale variant accepts.
    pub const MAX: f64 = 8.0;

    /// The parameter's name, as [`RangeError`] and the catalogue spell it.
    pub const NAME: &'static str = "scale";

    /// A scale, refusing anything outside the permitted range.
    ///
    /// What a command-line flag wants: its input was never bounded, so an out-of-range value is a mistake to report
    /// rather than a number to correct silently.
    ///
    /// # Errors
    ///
    /// Returns [`RangeError`] naming the permitted range when `value` is below [`Scale::MIN`], above [`Scale::MAX`],
    /// or not a number.
    pub fn new(value: f64) -> Result<Self, RangeError> {
        // Written as a containment test rather than as two comparisons so that `NaN`, which is neither below the
        // minimum nor above the maximum, is refused instead of quantized into an arbitrary integer.
        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(RangeError { parameter: Self::NAME, value, min: Self::MIN, max: Self::MAX });
        }

        Ok(Self::quantize(value))
    }

    /// A scale, bounding anything outside the permitted range to the nearest end of it.
    ///
    /// What a GUI slider wants: its input is already bounded by the control, so an error would be one the caller
    /// cannot act on. A value that is not a number bounds to [`Scale::MIN`], the factor that changes an image least.
    pub fn clamped(value: f64) -> Self {
        if value.is_nan() {
            return Self::quantize(Self::MIN);
        }

        Self::quantize(value.clamp(Self::MIN, Self::MAX))
    }

    /// The factor as a number, at the value it was quantized to.
    pub fn get(self) -> f64 {
        f64::from(self.steps) / STEPS_PER_UNIT
    }

    /// This scale applied to `width` and `height`, rounded.
    ///
    /// Rounded rather than truncated, which is a pixel on every odd dimension.
    pub(crate) fn applied_to(self, width: u32, height: u32) -> (u32, u32) {
        // The one spelling of a rule both contracts have to reach the same answer from. The diffusion side resamples
        // to this size before anything runs; the convolutional side corrects back to it after a pass sequence
        // overshoots. Which contract served a request must not be observable in the size of what comes back, so the
        // arithmetic is here — on the scale itself, beside `get` — rather than written out on each side and pinned
        // equal by a test.
        let factor = self.get();
        let axis = |length: u32| (f64::from(length) * factor).round() as u32;

        (axis(width), axis(height))
    }

    /// The factor in quantization steps, which is what a bucket table compares against.
    pub(crate) const fn steps(self) -> u16 {
        self.steps
    }

    /// Quantizes an already-bounded value.
    fn quantize(bounded: f64) -> Self {
        // Private because the bound is what makes the cast lossless.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "bounded to 1.0..=8.0 above, so the rounded product is within 1000..=8000"
        )]
        Self { steps: (bounded * STEPS_PER_UNIT).round() as u16 }
    }
}

impl std::fmt::Display for Scale {
    /// Renders the factor with no trailing zeros and no decimal point where there is no fraction: `2`, `2.5`,
    /// `1.667`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The same text the reference implementation's `%.4g` produces for every value in range.
        let whole = self.steps / 1000;
        let fraction = self.steps % 1000;

        if fraction == 0 {
            return write!(f, "{whole}");
        }

        // Trimmed by dividing rather than by formatting into a scratch string and trimming it: the trailing zeros
        // are decided by the value, so the divisor is too, and `Display` here is on the path every cache tag takes.
        match (fraction % 100, fraction % 10) {
            (0, _) => write!(f, "{whole}.{}", fraction / 100),
            (_, 0) => write!(f, "{whole}.{:02}", fraction / 10),
            _ => write!(f, "{whole}.{fraction:03}"),
        }
    }
}

impl TryFrom<f64> for Scale {
    type Error = RangeError;

    /// The path a deserialized scale takes, so a persisted or `invoke`d value is validated exactly as a directly
    /// constructed one is — the rejecting way, because a payload is not a slider.
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Scale> for f64 {
    /// The serialized form: a plain number, so a front end reads the scale it set rather than this type's storage.
    fn from(scale: Scale) -> Self {
        scale.get()
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::hash_of;
    use super::*;

    #[test]
    fn a_scale_inside_the_range_is_taken_as_given() {
        for value in [1.0, 1.65, 2.0, 2.5, 8.0] {
            let scale = Scale::new(value).expect("a scale in range is accepted");
            assert_eq!(scale.get(), value, "{value} was not carried at the value supplied");
        }
    }

    #[test]
    fn an_out_of_range_scale_is_refused_when_rejection_was_asked_for() {
        for value in [0.5, 12.0] {
            let error = Scale::new(value).expect_err("an out-of-range scale was accepted");
            assert_eq!(error.value, value);
            assert_eq!((error.min, error.max), (Scale::MIN, Scale::MAX), "the error did not name the permitted range");
            assert!(error.to_string().contains("between 1 and 8"), "the message did not state the range: {error}");
        }
    }

    #[test]
    fn a_scale_that_is_not_a_number_is_refused_rather_than_quantized() {
        assert!(Scale::new(f64::NAN).is_err());
        assert!(Scale::new(f64::INFINITY).is_err());
    }

    #[test]
    fn an_out_of_range_scale_is_bounded_when_clamping_was_asked_for() {
        assert_eq!(Scale::clamped(0.5).get(), Scale::MIN);
        assert_eq!(Scale::clamped(12.0).get(), Scale::MAX);
    }

    #[test]
    fn clamping_a_value_that_is_not_a_number_still_produces_a_scale_in_range() {
        // Infallible means infallible: there is no value of the argument that yields something out of range, which is
        // what lets everything downstream stop re-checking.
        assert_eq!(Scale::clamped(f64::NAN).get(), Scale::MIN);
    }

    #[test]
    fn a_scale_finer_than_three_decimals_is_quantized_on_acceptance() {
        assert_eq!(Scale::new(1.666_61).expect("in range").get(), 1.667);
    }

    #[test]
    fn two_scales_that_quantize_alike_are_one_value() {
        let low = Scale::new(1.666_61).expect("in range");
        let high = Scale::new(1.666_69).expect("in range");

        assert_eq!(low, high, "two scales that quantize alike stayed distinguishable");

        assert_eq!(hash_of(&low), hash_of(&high));
    }

    #[test]
    fn every_accepted_scale_compares_equal_to_itself() {
        // The property `f64` fails and the reason the storage is an integer: a scale that did not compare equal to
        // itself could not identify anything, so nothing could be keyed on an operation carrying one.
        let mut value = Scale::MIN;
        while value <= Scale::MAX {
            let scale = Scale::new(value).expect("in range");
            assert_eq!(scale, scale, "{value} did not compare equal to itself");
            value += 0.001;
        }
    }

    #[test]
    fn a_whole_scale_renders_with_no_decimal_part() {
        assert_eq!(Scale::new(4.0).expect("in range").to_string(), "4");
        assert_eq!(Scale::new(1.0).expect("in range").to_string(), "1");
    }

    #[test]
    fn a_fractional_scale_renders_without_trailing_zeros() {
        assert_eq!(Scale::new(2.5).expect("in range").to_string(), "2.5");
        assert_eq!(Scale::new(3.75).expect("in range").to_string(), "3.75");
        assert_eq!(Scale::new(1.666_61).expect("in range").to_string(), "1.667");
    }

    #[test]
    fn a_scale_round_trips_through_its_serialized_form_as_a_plain_number() {
        let scale = Scale::new(2.5).expect("in range");
        let json = serde_json::to_string(&scale).expect("a scale serializes");

        assert_eq!(json, "2.5");
        assert_eq!(serde_json::from_str::<Scale>(&json).expect("a scale deserializes"), scale);
    }

    #[test]
    fn a_serialized_scale_outside_the_range_is_refused() {
        assert!(
            serde_json::from_str::<Scale>("12").is_err(),
            "deserialization produced a scale that could not have been constructed"
        );
    }
}
