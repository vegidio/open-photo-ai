//! Which direction, and how far, a light adjustment or colour balance model shifts an image.

use serde::{Deserialize, Serialize};

use super::quantized::{quantized_param, write_thousandths};

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
    /// The quantized bias in steps of 1/[`STEPS_PER_UNIT`](super::quantized::STEPS_PER_UNIT), always within
    /// `-1000..=1000`.
    steps: i16,
}

impl Bias {
    /// The most negative bias any light adjustment or colour balance variant accepts: the model's adjustment applied
    /// in full, in the opposite direction. The same bound the reference's slider defaults to at `min={-100}`.
    pub const MIN: f64 = -1.0;

    /// The largest bias any light adjustment or colour balance variant accepts: the model's own output, matching the
    /// reference's `max={100}`.
    pub const MAX: f64 = 1.0;

    /// The bias as the `f32` a pipeline's per-pixel arithmetic is done in, resolved once per run rather than per
    /// pixel.
    pub(crate) fn as_f32(self) -> f32 {
        #[expect(clippy::cast_possible_truncation, reason = "a bias is bounded to -1.0..=1.0 at construction")]
        let narrowed = self.get() as f32;

        narrowed
    }
}

quantized_param! {
    Bias(i16),
    name: "bias",
    /// A value that is not a number bounds to 0, the bias that leaves an image alone — the neutral value here, where
    /// [`super::strength::Strength`]'s neutral fallback is its minimum.
    not_a_number: 0.0,
}

impl std::fmt::Display for Bias {
    /// Renders the bias with no trailing zeros and no decimal point where there is no fraction: `0`, `-0.5`, `1`.
    ///
    /// The sign is written separately from the magnitude rather than falling out of integer division, which would
    /// lose it: -500 steps divides to a whole part of 0, so `-0.5` would render as `0.5` and a negative bias would
    /// share a cache tag with its positive counterpart.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_thousandths(f, self.steps < 0, self.steps.unsigned_abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    super::super::quantized::quantized_param_tests! {
        Bias {
            accepted: [-1.0, -0.35, 0.0, 0.5, 1.0],
            refused: [-2.0, 1.5],
            range: "between -1 and 1",
            not_a_number: 0.0,
            quantized: 0.123_41 => 0.123,
            alike: [0.123_39, 0.123_41],
            serialized: -0.5 => "-0.5",
            refused_serialized: ["2", "-2"],
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
    fn a_negative_bias_is_quantized_as_symmetrically_as_a_positive_one() {
        assert_eq!(Bias::new(0.123_41).expect("in range").get(), 0.123);
        assert_eq!(Bias::new(-0.123_41).expect("in range").get(), -0.123);
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
}
