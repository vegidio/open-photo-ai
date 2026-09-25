//! Which direction, and how far, a light adjustment or colour balance model shifts an image.

use serde::{Deserialize, Serialize};

use super::scale::RangeError;

/// How many quantization steps one whole unit of bias is divided into.
///
/// Three decimal places, matching [`super::strength::Strength`] and [`super::scale::Scale`], and finer than anything
/// the reference implementation's interface can produce.
const STEPS_PER_UNIT: f64 = 1000.0;

/// Which direction, and how far, a light adjustment or colour balance operation shifts an image.
///
/// Bounded to [`Bias::MIN`]-[`Bias::MAX`] inclusive, and quantized to three decimal places on acceptance. 0.0 leaves
/// the image as it was, 1.0 is the model's own output, and a negative value applies the adjustment in the opposite
/// direction — which is why this is a signed type and [`super::strength::Strength`] is not.
///
/// A separate type from `Strength` rather than one shared "intensity", although the reference implementation calls
/// both by that name and passes both under one map key. They are not one parameter: 0-3 centred on 1 says how much of
/// an effect, and -1..1 centred on 0 says in which direction and how far. One type would have to carry its bounds per
/// family, and its neutral value would depend on who was holding it.
///
/// Stored as an integer count of quantization steps for the reason [`super::scale::Scale`] gives: the operations
/// carrying one hash and compare to key the model registry, and `f64` implements neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Bias {
    /// The quantized bias in steps of 1/[`STEPS_PER_UNIT`], always within `-1000..=1000`.
    steps: i16,
}

impl Bias {
    /// The most negative bias any light adjustment or colour balance variant accepts: the model's adjustment applied
    /// in full, in the opposite direction. The same bound the reference's slider defaults to at `min={-100}`.
    pub const MIN: f64 = -1.0;

    /// The largest bias any light adjustment or colour balance variant accepts: the model's own output, matching the
    /// reference's `max={100}`.
    pub const MAX: f64 = 1.0;

    /// The parameter's name, as [`RangeError`] and the catalogue spell it.
    pub const NAME: &'static str = "bias";

    /// A bias, refusing anything outside the permitted range.
    ///
    /// What a command-line flag wants: its input was never bounded, so an out-of-range value is a mistake to report
    /// rather than a number to correct silently.
    ///
    /// # Errors
    ///
    /// Returns [`RangeError`] naming the permitted range when `value` is below [`Bias::MIN`], above [`Bias::MAX`], or
    /// not a number.
    pub fn new(value: f64) -> Result<Self, RangeError> {
        // A containment test rather than two comparisons, so that `NaN` — which is neither below the minimum nor
        // above the maximum — is refused instead of quantized into an arbitrary integer.
        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(RangeError { parameter: Self::NAME, value, min: Self::MIN, max: Self::MAX });
        }

        Ok(Self::quantize(value))
    }

    /// A bias, bounding anything outside the permitted range to the nearest end of it.
    ///
    /// What a GUI slider wants: its input is already bounded by the control, so an error would be one the caller
    /// cannot act on. A value that is not a number bounds to 0, the bias that leaves an image alone — the neutral
    /// value here, where [`super::strength::Strength`]'s neutral fallback is its minimum.
    pub fn clamped(value: f64) -> Self {
        if value.is_nan() {
            return Self::quantize(0.0);
        }

        Self::quantize(value.clamp(Self::MIN, Self::MAX))
    }

    /// The bias as a number, at the value it was quantized to.
    pub fn get(self) -> f64 {
        f64::from(self.steps) / STEPS_PER_UNIT
    }

    /// The bias as the `f32` a pipeline's per-pixel arithmetic is done in, resolved once per run rather than per
    /// pixel.
    pub(crate) fn as_f32(self) -> f32 {
        #[expect(clippy::cast_possible_truncation, reason = "a bias is bounded to -1.0..=1.0 at construction")]
        let narrowed = self.get() as f32;

        narrowed
    }

    /// Quantizes an already-bounded value. Private because the bound is what makes the cast lossless: the largest
    /// magnitude this is ever called with is 1.0, so the rounded product always fits an `i16`.
    fn quantize(bounded: f64) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "bounded to -1.0..=1.0 above, so the rounded product is within -1000..=1000"
        )]
        Self { steps: (bounded * STEPS_PER_UNIT).round() as i16 }
    }
}

impl std::fmt::Display for Bias {
    /// Renders the bias with no trailing zeros and no decimal point where there is no fraction: `0`, `-0.5`, `1`.
    ///
    /// The sign is written separately from the magnitude rather than falling out of integer division, which would
    /// lose it: -500 steps divides to a whole part of 0, so `-0.5` would render as `0.5` and a negative bias would
    /// share a cache tag with its positive counterpart.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sign = if self.steps < 0 { "-" } else { "" };
        let magnitude = self.steps.unsigned_abs();
        let whole = magnitude / 1000;
        let fraction = magnitude % 1000;

        if fraction == 0 {
            return write!(f, "{sign}{whole}");
        }

        // Trimmed by dividing rather than by formatting into a scratch string and trimming it: the trailing zeros
        // are decided by the value, so the divisor is too, and `Display` here is on the path every cache tag takes.
        match (fraction % 100, fraction % 10) {
            (0, _) => write!(f, "{sign}{whole}.{}", fraction / 100),
            (_, 0) => write!(f, "{sign}{whole}.{:02}", fraction / 10),
            _ => write!(f, "{sign}{whole}.{fraction:03}"),
        }
    }
}

impl TryFrom<f64> for Bias {
    type Error = RangeError;

    /// The path a deserialized bias takes, so a persisted or `invoke`d value is validated exactly as a directly
    /// constructed one is — the rejecting way, because a payload is not a slider.
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Bias> for f64 {
    /// The serialized form: a plain number, so a front end reads the bias it set rather than this type's storage.
    fn from(bias: Bias) -> Self {
        bias.get()
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::hash_of;
    use super::*;

    #[test]
    fn a_bias_inside_the_range_is_taken_as_given() {
        for value in [-1.0, -0.35, 0.0, 0.5, 1.0] {
            let bias = Bias::new(value).expect("a bias in range is accepted");
            assert_eq!(bias.get(), value, "{value} was not carried at the value supplied");
        }
    }

    #[test]
    fn the_negative_half_of_the_range_is_carried_as_precisely_as_the_positive_one() {
        // The half a `u16` storage would have made unrepresentable, and the half the reference's `%.3g` formatting
        // renders with a sign it never validates.
        for value in [-1.0, -0.999, -0.5, -0.123, -0.001] {
            let bias = Bias::new(value).expect("a negative bias in range is accepted");

            assert_eq!(bias.get(), value, "{value} was not carried at the value supplied");
            assert!(bias.get() < 0.0, "{value} lost its sign");
            assert_ne!(bias, Bias::new(-value).expect("in range"), "{value} compared equal to its opposite");
        }
    }

    #[test]
    fn an_out_of_range_bias_is_refused_when_rejection_was_asked_for() {
        for value in [-2.0, 1.5] {
            let error = Bias::new(value).expect_err("an out-of-range bias was accepted");

            assert_eq!(error.value, value);
            assert_eq!((error.min, error.max), (Bias::MIN, Bias::MAX), "the error did not name the range");
            assert!(error.to_string().contains("between -1 and 1"), "the message did not state the range: {error}");
            assert!(error.to_string().starts_with("bias"), "the message did not name the parameter: {error}");
        }
    }

    #[test]
    fn a_bias_that_is_not_a_number_is_refused_rather_than_quantized() {
        assert!(Bias::new(f64::NAN).is_err());
        assert!(Bias::new(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn an_out_of_range_bias_is_bounded_when_clamping_was_asked_for() {
        assert_eq!(Bias::clamped(-2.0).get(), Bias::MIN);
        assert_eq!(Bias::clamped(1.5).get(), Bias::MAX);
    }

    #[test]
    fn clamping_a_value_that_is_not_a_number_produces_the_bias_that_leaves_an_image_alone() {
        assert_eq!(Bias::clamped(f64::NAN).get(), 0.0);
    }

    #[test]
    fn a_bias_finer_than_three_decimals_is_quantized_on_acceptance() {
        assert_eq!(Bias::new(0.123_41).expect("in range").get(), 0.123);
        assert_eq!(Bias::new(-0.123_41).expect("in range").get(), -0.123);
    }

    #[test]
    fn two_biases_that_quantize_alike_are_one_value() {
        let low = Bias::new(0.123_39).expect("in range");
        let high = Bias::new(0.123_41).expect("in range");

        assert_eq!(low, high, "two biases that quantize alike stayed distinguishable");

        assert_eq!(hash_of(&low), hash_of(&high));
    }

    #[test]
    fn every_accepted_bias_compares_equal_to_itself() {
        let mut value = Bias::MIN;
        while value <= Bias::MAX {
            let bias = Bias::new(value).expect("in range");
            assert_eq!(bias, bias, "{value} did not compare equal to itself");
            value += 0.001;
        }
    }

    #[test]
    fn a_bias_of_exactly_zero_round_trips_and_renders_without_a_sign() {
        // The value a neutral operation carries, and the one a signed storage could render as `-0`: two spellings of
        // one bias would be two cache tags for one image.
        let zero = Bias::new(0.0).expect("in range");

        assert_eq!(zero.to_string(), "0");
        assert_eq!(zero.get(), 0.0);
        assert_eq!(Bias::new(-0.0).expect("in range"), zero, "negative zero was a second value");
        assert_eq!(Bias::new(-0.0).expect("in range").to_string(), "0");

        let json = serde_json::to_string(&zero).expect("a bias serializes");
        assert_eq!(serde_json::from_str::<Bias>(&json).expect("a bias deserializes"), zero);
        assert_eq!(serde_json::from_str::<Bias>(&json).expect("in range").to_string(), "0");
    }

    #[test]
    fn a_whole_bias_renders_with_no_decimal_part() {
        assert_eq!(Bias::new(1.0).expect("in range").to_string(), "1");
        assert_eq!(Bias::new(-1.0).expect("in range").to_string(), "-1");
    }

    #[test]
    fn a_fractional_bias_renders_without_trailing_zeros_and_keeps_its_sign() {
        assert_eq!(Bias::new(0.5).expect("in range").to_string(), "0.5");
        assert_eq!(Bias::new(-0.5).expect("in range").to_string(), "-0.5");
        assert_eq!(Bias::new(-0.35).expect("in range").to_string(), "-0.35");
        assert_eq!(Bias::new(0.123_41).expect("in range").to_string(), "0.123");
    }

    #[test]
    fn a_negative_bias_never_renders_the_same_text_as_its_positive_counterpart() {
        // What keeps the two out of one run cache entry, since the rendered text is what a cache tag carries.
        let mut value = 0.001;
        while value <= Bias::MAX {
            let positive = Bias::new(value).expect("in range").to_string();
            let negative = Bias::new(-value).expect("in range").to_string();

            assert_ne!(positive, negative, "{value} and its opposite render alike");
            value += 0.001;
        }
    }

    #[test]
    fn a_bias_round_trips_through_its_serialized_form_as_a_plain_number() {
        let bias = Bias::new(-0.5).expect("in range");
        let json = serde_json::to_string(&bias).expect("a bias serializes");

        assert_eq!(json, "-0.5");
        assert_eq!(serde_json::from_str::<Bias>(&json).expect("a bias deserializes"), bias);
    }

    #[test]
    fn a_serialized_bias_outside_the_range_is_refused() {
        assert!(
            serde_json::from_str::<Bias>("2").is_err(),
            "deserialization produced a bias that could not have been constructed"
        );
        assert!(serde_json::from_str::<Bias>("-2").is_err());
    }
}
