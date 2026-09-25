//! How closely a face recovery model is asked to keep to the face it was given.

use serde::{Deserialize, Serialize};

use super::quantized::quantized_param;

/// How closely a face recovery run is asked to keep to the face it was given.
///
/// Bounded to [`Fidelity::MIN`]-[`Fidelity::MAX`] inclusive, and quantized to three decimal places on acceptance.
/// [`Fidelity::MAX`] is **maximum fidelity to the original face**: the restoration stays as close to what was
/// photographed as the model can, and is the value the reference implementation hard-codes. Lower values let the
/// model depart further from the face in front of it, restoring more and resembling less.
///
/// Carried by Athens alone. Santorini's graph takes the image and nothing beside it, so a Santorini operation has
/// nowhere to put one — see [`super::face_recovery::FaceRecovery`], where the absence is an `Option` rather than the
/// reference's `Fidelity: -1` sentinel.
///
/// Stored as an integer count of quantization steps rather than as an `f64`, for the reason
/// [`super::scale::Scale`] gives: an operation carrying one compares and hashes as a value, `f64` implements neither
/// of those traits, and its `PartialEq` is unsound as an identity anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Fidelity {
    /// The quantized fidelity in steps of 1/[`STEPS_PER_UNIT`](super::quantized::STEPS_PER_UNIT), always within
    /// `0..=1000`.
    steps: u16,
}

impl Fidelity {
    /// The lowest fidelity a face recovery variant accepts: the model departs from the photographed face as far as
    /// it will.
    ///
    /// A property of the family rather than of any one model. The catalogue publishes this constant, so a control
    /// and this constructor cannot disagree about what is offerable.
    pub const MIN: f64 = 0.0;

    /// The highest fidelity a face recovery variant accepts: the restoration keeps as close to the photographed face
    /// as the model can, which is the value the reference implementation's `athens.go` fixes.
    pub const MAX: f64 = 1.0;

    /// The highest fidelity, as a value rather than as a bound to construct one from.
    ///
    /// **The default every caller that does not offer the control reaches for** — the reference implementation's
    /// `athens.go` fixes it, so a run with no fidelity beside it runs at this one. Spelled as a constant because
    /// `Fidelity::new(Fidelity::MAX)` cannot fail and the four callers that wrote it were each carrying an
    /// `.expect` and the same sentence explaining why it would not fire.
    ///
    /// The `steps` literal is the quantization of [`MAX`](Self::MAX) — `1.0 * 1000.0` — which the test below pins
    /// against the constructor so the two cannot drift.
    pub const MAXIMUM: Self = Self { steps: 1000 };
}

quantized_param! {
    Fidelity(u16),
    name: "fidelity",
    /// A value that is not a number bounds to [`Fidelity::MAX`], which changes a face least — the same rule
    /// [`super::strength::Strength::clamped`] follows, at this parameter's own end of its range.
    not_a_number: Fidelity::MAX,
}

impl std::fmt::Display for Fidelity {
    /// Renders the fidelity at a fixed three decimals: `0.000`, `0.500`, `1.000`.
    ///
    /// The one place this parameter's rendering departs from [`super::strength::Strength`] and
    /// [`super::scale::Scale`], which trim trailing zeros. Where this ends up is the `-w` segment of
    /// [`FaceRecovery::cache_tag`](super::face_recovery::FaceRecovery::cache_tag), beside a faces signature that is
    /// itself fixed-width at two decimals — so the tag reads as one spelling rather than two, and every fidelity's
    /// segment has the same shape. Nothing is invented by the width: the value is exact at three decimals, which is
    /// what it was quantized to.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.3}", self.get())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    super::super::quantized::quantized_param_tests! {
        Fidelity {
            accepted: [0.0, 0.25, 0.5, 0.75, 1.0],
            refused: [-0.1, 1.5],
            range: "between 0 and 1",
            not_a_number: Fidelity::MAX,
            quantized: 0.123_456 => 0.123,
            alike: [0.499_99, 0.500_01],
            serialized: 0.5 => "0.5",
            refused_serialized: ["1.5", "-0.1"],
        }
    }

    #[test]
    fn the_maximum_is_the_value_the_constructor_produces_for_the_upper_bound() {
        // `MAXIMUM` spells its `steps` as a literal, which is only correct while it agrees with what the
        // constructor produces for the same value. Pinned here so a change to `STEPS_PER_UNIT` or to `MAX` fails
        // this rather than silently moving the default every face-recovery run without a control uses.
        assert_eq!(Fidelity::MAXIMUM, Fidelity::new(Fidelity::MAX).expect("the maximum is in range"));
        assert!((Fidelity::MAXIMUM.get() - Fidelity::MAX).abs() < f64::EPSILON);
    }

    #[test]
    fn a_fidelity_renders_at_a_fixed_three_decimals() {
        // Pinned as literals: this is the `-w` segment of every cached Athens image's key, so a change to the
        // spelling invalidates all of them.
        assert_eq!(Fidelity::new(0.0).expect("in range").to_string(), "0.000");
        assert_eq!(Fidelity::new(0.5).expect("in range").to_string(), "0.500");
        assert_eq!(Fidelity::new(1.0).expect("in range").to_string(), "1.000");
        assert_eq!(Fidelity::new(0.123_456).expect("in range").to_string(), "0.123");
    }
}
