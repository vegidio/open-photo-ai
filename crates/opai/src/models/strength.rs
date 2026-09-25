//! How much of a denoise or sharpen model's effect an operation applies.

use serde::{Deserialize, Serialize};

use super::scale::RangeError;

/// How many quantization steps one whole unit of strength is divided into.
///
/// Three decimal places, the same resolution [`super::scale::Scale`] uses and finer than anything the reference
/// implementation's interface can produce — its slider steps in whole percent and its text field parses an integer
/// percent, so no reachable input is rounded.
const STEPS_PER_UNIT: f64 = 1000.0;

/// How much of a denoise or sharpen model's output an operation applies.
///
/// Bounded to [`Strength::MIN`]-[`Strength::MAX`] inclusive, and quantized to three decimal places on acceptance.
/// 1.0 is the model's own output: below it the effect is softened back towards the original image, above it the
/// effect is amplified past what the model produced.
///
/// Stored as an integer count of quantization steps rather than as an `f64`, for the reason [`super::scale::Scale`]
/// gives: the operations carrying one hash and compare to key the model registry, `f64` implements neither, and its
/// `PartialEq` is unsound as an identity anyway.
///
/// Quantizing also closes a collision the reference implementation has. It folds the value into the run cache key
/// with `%.3g`, so two strengths that round alike produce one key, collide, and the second run is served the first
/// one's image. Here the two are the same `Strength` before anything is rendered, so they are one value rather than
/// two sharing a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Strength {
    /// The quantized strength in steps of 1/[`STEPS_PER_UNIT`], always within `0..=3000`.
    steps: u16,
}

impl Strength {
    /// The smallest strength any denoise or sharpen variant accepts: the model's effect applied not at all.
    ///
    /// A property of the families rather than of any one model, and the same bound the reference's `min={0}` puts on
    /// its slider. The catalogue publishes this constant, so a control and this constructor cannot disagree about
    /// what is offerable.
    pub const MIN: f64 = 0.0;

    /// The largest strength any denoise or sharpen variant accepts: three times the model's own output, matching the
    /// reference's `max={300}`.
    pub const MAX: f64 = 3.0;

    /// The parameter's name, as [`RangeError`] and the catalogue spell it.
    pub const NAME: &'static str = "strength";

    /// A strength, refusing anything outside the permitted range.
    ///
    /// What a command-line flag wants: its input was never bounded, so an out-of-range value is a mistake to report
    /// rather than a number to correct silently.
    ///
    /// # Errors
    ///
    /// Returns [`RangeError`] naming the permitted range when `value` is below [`Strength::MIN`], above
    /// [`Strength::MAX`], or not a number.
    pub fn new(value: f64) -> Result<Self, RangeError> {
        // A containment test rather than two comparisons, so that `NaN` — which is neither below the minimum nor
        // above the maximum — is refused instead of quantized into an arbitrary integer.
        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(RangeError { parameter: Self::NAME, value, min: Self::MIN, max: Self::MAX });
        }

        Ok(Self::quantize(value))
    }

    /// A strength, bounding anything outside the permitted range to the nearest end of it.
    ///
    /// What a GUI slider wants: its input is already bounded by the control, so an error would be one the caller
    /// cannot act on. A value that is not a number bounds to [`Strength::MIN`], which changes an image least.
    pub fn clamped(value: f64) -> Self {
        if value.is_nan() {
            return Self::quantize(Self::MIN);
        }

        Self::quantize(value.clamp(Self::MIN, Self::MAX))
    }

    /// The strength as a number, at the value it was quantized to.
    pub fn get(self) -> f64 {
        f64::from(self.steps) / STEPS_PER_UNIT
    }

    /// The strength as the `f32` a pipeline's per-pixel arithmetic is done in, resolved once per run rather than per
    /// pixel.
    pub(crate) fn as_f32(self) -> f32 {
        #[expect(clippy::cast_possible_truncation, reason = "a strength is bounded to 0.0..=3.0 at construction")]
        let narrowed = self.get() as f32;

        narrowed
    }

    /// Quantizes an already-bounded value. Private because the bound is what makes the cast lossless: the largest
    /// value this is ever called with is 3.0, so the rounded product always fits a `u16`.
    fn quantize(bounded: f64) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "bounded to 0.0..=3.0 above, so the rounded product is within 0..=3000"
        )]
        Self { steps: (bounded * STEPS_PER_UNIT).round() as u16 }
    }
}

impl std::fmt::Display for Strength {
    /// Renders the strength with no trailing zeros and no decimal point where there is no fraction: `0`, `1`, `2.25`.
    ///
    /// The same shape [`super::scale::Scale`] renders, because both end up in a run cache tag and a tag whose
    /// spelling depended on which parameter wrote it would be one more thing to keep in step.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

impl TryFrom<f64> for Strength {
    type Error = RangeError;

    /// The path a deserialized strength takes, so a persisted or `invoke`d value is validated exactly as a directly
    /// constructed one is — the rejecting way, because a payload is not a slider.
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Strength> for f64 {
    /// The serialized form: a plain number, so a front end reads the strength it set rather than this type's storage.
    fn from(strength: Strength) -> Self {
        strength.get()
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::hash_of;
    use super::*;

    #[test]
    fn a_strength_inside_the_range_is_taken_as_given() {
        for value in [0.0, 0.5, 1.0, 2.25, 3.0] {
            let strength = Strength::new(value).expect("a strength in range is accepted");
            assert_eq!(strength.get(), value, "{value} was not carried at the value supplied");
        }
    }

    #[test]
    fn an_out_of_range_strength_is_refused_when_rejection_was_asked_for() {
        for value in [-0.5, 4.0] {
            let error = Strength::new(value).expect_err("an out-of-range strength was accepted");

            assert_eq!(error.value, value);
            assert_eq!((error.min, error.max), (Strength::MIN, Strength::MAX), "the error did not name the range");
            assert!(error.to_string().contains("between 0 and 3"), "the message did not state the range: {error}");
            assert!(error.to_string().starts_with("strength"), "the message did not name the parameter: {error}");
        }
    }

    #[test]
    fn a_strength_that_is_not_a_number_is_refused_rather_than_quantized() {
        assert!(Strength::new(f64::NAN).is_err());
        assert!(Strength::new(f64::INFINITY).is_err());
    }

    #[test]
    fn an_out_of_range_strength_is_bounded_when_clamping_was_asked_for() {
        assert_eq!(Strength::clamped(-0.5).get(), Strength::MIN);
        assert_eq!(Strength::clamped(4.0).get(), Strength::MAX);
    }

    #[test]
    fn clamping_a_value_that_is_not_a_number_still_produces_a_strength_in_range() {
        // Infallible means infallible: no argument yields something out of range, which is what lets everything
        // downstream stop re-checking.
        assert_eq!(Strength::clamped(f64::NAN).get(), Strength::MIN);
    }

    #[test]
    fn a_strength_finer_than_three_decimals_is_quantized_on_acceptance() {
        assert_eq!(Strength::new(1.234_56).expect("in range").get(), 1.235);
    }

    #[test]
    fn two_strengths_that_quantize_alike_are_one_value() {
        let low = Strength::new(1.234_51).expect("in range");
        let high = Strength::new(1.234_56).expect("in range");

        assert_eq!(low, high, "two strengths that quantize alike stayed distinguishable");

        assert_eq!(hash_of(&low), hash_of(&high));
    }

    #[test]
    fn every_accepted_strength_compares_equal_to_itself() {
        // The property `f64` fails and the reason the storage is an integer: a strength that did not compare equal to
        // itself could not identify anything, so nothing carrying one could be keyed on.
        let mut value = Strength::MIN;
        while value <= Strength::MAX {
            let strength = Strength::new(value).expect("in range");
            assert_eq!(strength, strength, "{value} did not compare equal to itself");
            value += 0.001;
        }
    }

    #[test]
    fn a_whole_strength_renders_with_no_decimal_part() {
        assert_eq!(Strength::new(0.0).expect("in range").to_string(), "0");
        assert_eq!(Strength::new(1.0).expect("in range").to_string(), "1");
        assert_eq!(Strength::new(3.0).expect("in range").to_string(), "3");
    }

    #[test]
    fn a_fractional_strength_renders_without_trailing_zeros() {
        assert_eq!(Strength::new(0.5).expect("in range").to_string(), "0.5");
        assert_eq!(Strength::new(2.25).expect("in range").to_string(), "2.25");
        assert_eq!(Strength::new(1.234_56).expect("in range").to_string(), "1.235");
    }

    #[test]
    fn a_strength_round_trips_through_its_serialized_form_as_a_plain_number() {
        let strength = Strength::new(0.5).expect("in range");
        let json = serde_json::to_string(&strength).expect("a strength serializes");

        assert_eq!(json, "0.5");
        assert_eq!(serde_json::from_str::<Strength>(&json).expect("a strength deserializes"), strength);
    }

    #[test]
    fn a_serialized_strength_outside_the_range_is_refused() {
        assert!(
            serde_json::from_str::<Strength>("4").is_err(),
            "deserialization produced a strength that could not have been constructed"
        );
        assert!(serde_json::from_str::<Strength>("-0.5").is_err());
    }
}
