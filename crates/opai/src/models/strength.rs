//! How much of a denoise or sharpen model's effect an operation applies.

use serde::{Deserialize, Serialize};

use super::quantized::{quantized_param, write_thousandths};

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
    /// The quantized strength in steps of 1/[`STEPS_PER_UNIT`](super::quantized::STEPS_PER_UNIT), always within
    /// `0..=3000`.
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

    /// The strength as the `f32` a pipeline's per-pixel arithmetic is done in, resolved once per run rather than per
    /// pixel.
    pub(crate) fn as_f32(self) -> f32 {
        #[expect(clippy::cast_possible_truncation, reason = "a strength is bounded to 0.0..=3.0 at construction")]
        let narrowed = self.get() as f32;

        narrowed
    }
}

quantized_param! {
    Strength(u16),
    name: "strength",
    /// A value that is not a number bounds to [`Strength::MIN`], which changes an image least.
    not_a_number: Strength::MIN,
}

impl std::fmt::Display for Strength {
    /// Renders the strength with no trailing zeros and no decimal point where there is no fraction: `0`, `1`, `2.25`.
    ///
    /// The same shape [`super::scale::Scale`] renders, because both end up in a run cache tag and a tag whose
    /// spelling depended on which parameter wrote it would be one more thing to keep in step.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write_thousandths(f, false, self.steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    super::super::quantized::quantized_param_tests! {
        Strength {
            accepted: [0.0, 0.5, 1.0, 2.25, 3.0],
            refused: [-0.5, 4.0],
            range: "between 0 and 3",
            not_a_number: Strength::MIN,
            quantized: 1.234_56 => 1.235,
            alike: [1.234_51, 1.234_56],
            serialized: 0.5 => "0.5",
            refused_serialized: ["4", "-0.5"],
        }
    }

    #[test]
    fn a_strength_renders_without_trailing_zeros_or_a_decimal_point_it_does_not_need() {
        assert_eq!(Strength::new(0.0).expect("in range").to_string(), "0");
        assert_eq!(Strength::new(3.0).expect("in range").to_string(), "3");
        assert_eq!(Strength::new(2.25).expect("in range").to_string(), "2.25");
        assert_eq!(Strength::new(1.234_56).expect("in range").to_string(), "1.235");
    }
}
