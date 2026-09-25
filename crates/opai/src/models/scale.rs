//! The upscale factor an operation is run at, and the error an out-of-range one produces.

use serde::{Deserialize, Serialize};

use super::quantized::{quantized_param, write_thousandths};

// Shared by every ranged parameter rather than written once per type: the message differs only in the parameter's
// name and its bounds, and a caller that renders one renders them all.
//
// The types that produce one are `Scale`, `Strength`, `Bias`, `Fidelity` and `Confidence`. The first four share
// their mechanism through `quantized::quantized_param!`; what differs between them — why one is signed, which end a
// `NaN` falls back to, which reference formatting each rendering reproduces — stays beside each type.
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
    /// The quantized factor in steps of 1/[`STEPS_PER_UNIT`](super::quantized::STEPS_PER_UNIT), always within
    /// `1000..=8000`.
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
}

quantized_param! {
    Scale(u16),
    name: "scale",
    /// A value that is not a number bounds to [`Scale::MIN`], the factor that changes an image least.
    not_a_number: Scale::MIN,
}

impl std::fmt::Display for Scale {
    /// Renders the factor with no trailing zeros and no decimal point where there is no fraction: `2`, `2.5`,
    /// `1.667`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The same text the reference implementation's `%.4g` produces for every value in range.
        write_thousandths(f, false, self.steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    super::super::quantized::quantized_param_tests! {
        Scale {
            accepted: [1.0, 1.65, 2.0, 2.5, 8.0],
            refused: [0.5, 12.0],
            range: "between 1 and 8",
            not_a_number: Scale::MIN,
            quantized: 1.666_61 => 1.667,
            alike: [1.666_61, 1.666_69],
            serialized: 2.5 => "2.5",
            refused_serialized: ["12", "0.5"],
        }
    }

    #[test]
    fn a_scale_renders_as_the_references_four_significant_figures_do() {
        assert_eq!(Scale::new(1.0).expect("in range").to_string(), "1");
        assert_eq!(Scale::new(4.0).expect("in range").to_string(), "4");
        assert_eq!(Scale::new(3.75).expect("in range").to_string(), "3.75");
        assert_eq!(Scale::new(1.666_61).expect("in range").to_string(), "1.667");
    }
}
