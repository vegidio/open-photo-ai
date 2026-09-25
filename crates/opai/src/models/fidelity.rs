//! How closely a face recovery model is asked to keep to the face it was given.

use serde::{Deserialize, Serialize};

use super::scale::RangeError;

/// How many quantization steps one whole unit of fidelity is divided into.
///
/// Three decimal places, the same resolution [`super::scale::Scale`], [`super::strength::Strength`] and
/// [`super::bias::Bias`] use — so every per-run parameter in the library is accepted at one resolution rather than
/// each family deciding its own.
const STEPS_PER_UNIT: f64 = 1000.0;

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
    /// The quantized fidelity in steps of 1/[`STEPS_PER_UNIT`], always within `0..=1000`.
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

    /// The parameter's name, as [`RangeError`] and the catalogue spell it.
    pub const NAME: &'static str = "fidelity";

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

    /// A fidelity, refusing anything outside the permitted range.
    ///
    /// What a command-line flag wants: its input was never bounded, so an out-of-range value is a mistake to report
    /// rather than a number to correct silently.
    ///
    /// # Errors
    ///
    /// Returns [`RangeError`] naming the permitted range when `value` is below [`Fidelity::MIN`], above
    /// [`Fidelity::MAX`], or not a number.
    pub fn new(value: f64) -> Result<Self, RangeError> {
        // A containment test rather than two comparisons, so that `NaN` — which is neither below the minimum nor
        // above the maximum — is refused instead of quantized into an arbitrary integer.
        if !(Self::MIN..=Self::MAX).contains(&value) {
            return Err(RangeError { parameter: Self::NAME, value, min: Self::MIN, max: Self::MAX });
        }

        Ok(Self::quantize(value))
    }

    /// A fidelity, bounding anything outside the permitted range to the nearest end of it.
    ///
    /// What a GUI slider wants: its input is already bounded by the control, so an error would be one the caller
    /// cannot act on. A value that is not a number bounds to [`Fidelity::MAX`], which changes a face least — the
    /// same rule [`super::strength::Strength::clamped`] follows, at this parameter's own end of its range.
    pub fn clamped(value: f64) -> Self {
        if value.is_nan() {
            return Self::quantize(Self::MAX);
        }

        Self::quantize(value.clamp(Self::MIN, Self::MAX))
    }

    /// The fidelity as a number, at the value it was quantized to.
    pub fn get(self) -> f64 {
        f64::from(self.steps) / STEPS_PER_UNIT
    }

    /// Quantizes an already-bounded value. Private because the bound is what makes the cast lossless: the largest
    /// value this is ever called with is 1.0, so the rounded product always fits a `u16`.
    fn quantize(bounded: f64) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "bounded to 0.0..=1.0 above, so the rounded product is within 0..=1000"
        )]
        Self { steps: (bounded * STEPS_PER_UNIT).round() as u16 }
    }
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

impl TryFrom<f64> for Fidelity {
    type Error = RangeError;

    /// The path a deserialized fidelity takes, so a persisted or `invoke`d value is validated exactly as a directly
    /// constructed one is — the rejecting way, because a payload is not a slider.
    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Fidelity> for f64 {
    /// The serialized form: a plain number, so a front end reads the fidelity it set rather than this type's storage.
    fn from(fidelity: Fidelity) -> Self {
        fidelity.get()
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::hash_of;
    use super::*;

    #[test]
    fn a_fidelity_inside_the_range_is_taken_as_given() {
        for value in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let fidelity = Fidelity::new(value).expect("a fidelity in range is accepted");
            assert_eq!(fidelity.get(), value, "{value} was not carried at the value supplied");
        }
    }

    #[test]
    fn an_out_of_range_fidelity_is_refused_when_rejection_was_asked_for() {
        for value in [-0.1, 1.5] {
            let error = Fidelity::new(value).expect_err("an out-of-range fidelity was accepted");

            assert_eq!(error.value, value);
            assert_eq!((error.min, error.max), (Fidelity::MIN, Fidelity::MAX), "the error did not name the range");
            assert!(error.to_string().contains("between 0 and 1"), "the message did not state the range: {error}");
            assert!(error.to_string().starts_with("fidelity"), "the message did not name the parameter: {error}");
        }
    }

    #[test]
    fn a_fidelity_that_is_not_a_number_is_refused_rather_than_quantized() {
        assert!(Fidelity::new(f64::NAN).is_err());
        assert!(Fidelity::new(f64::INFINITY).is_err());
        assert!(Fidelity::new(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn an_out_of_range_fidelity_is_bounded_when_clamping_was_asked_for() {
        assert_eq!(Fidelity::clamped(-0.1).get(), Fidelity::MIN);
        assert_eq!(Fidelity::clamped(1.5).get(), Fidelity::MAX);
    }

    #[test]
    fn clamping_a_value_that_is_not_a_number_still_produces_a_fidelity_in_range() {
        // Infallible means infallible: no argument yields something out of range, which is what lets everything
        // downstream stop re-checking. The fallback is the end that changes a face least.
        assert_eq!(Fidelity::clamped(f64::NAN).get(), Fidelity::MAX);
    }

    #[test]
    fn the_published_constants_are_the_ones_the_constructor_enforces() {
        assert!(Fidelity::new(Fidelity::MIN).is_ok());
        assert!(Fidelity::new(Fidelity::MAX).is_ok());

        // `MAXIMUM` spells its `steps` as a literal, which is only correct while it agrees with what the
        // constructor produces for the same value. Pinned here so a change to `STEPS_PER_UNIT` or to `MAX` fails
        // this rather than silently moving the default every face-recovery run without a control uses.
        assert_eq!(Fidelity::MAXIMUM, Fidelity::new(Fidelity::MAX).expect("the maximum is in range"));
        assert!((Fidelity::MAXIMUM.get() - Fidelity::MAX).abs() < f64::EPSILON);
        assert!(Fidelity::new(Fidelity::MIN - 0.001).is_err());
        assert!(Fidelity::new(Fidelity::MAX + 0.001).is_err());
    }

    #[test]
    fn a_fidelity_finer_than_three_decimals_is_quantized_on_acceptance() {
        assert_eq!(Fidelity::new(0.123_456).expect("in range").get(), 0.123);
    }

    #[test]
    fn two_fidelities_that_quantize_alike_are_one_value() {
        let low = Fidelity::new(0.499_99).expect("in range");
        let high = Fidelity::new(0.500_01).expect("in range");

        assert_eq!(low.get(), 0.5, "the test did not supply two values that quantize to one");
        assert_eq!(low, high, "two fidelities that quantize alike stayed distinguishable");
        assert_eq!(hash_of(&low), hash_of(&high));
    }

    #[test]
    fn every_accepted_fidelity_compares_equal_to_itself() {
        // The property `f64` fails and the reason the storage is an integer: a fidelity that did not compare equal
        // to itself could not identify anything, so nothing carrying one could be keyed on.
        let mut value = Fidelity::MIN;
        while value <= Fidelity::MAX {
            let fidelity = Fidelity::new(value).expect("in range");
            assert_eq!(fidelity, fidelity, "{value} did not compare equal to itself");
            value += 0.001;
        }
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

    #[test]
    fn a_fidelity_round_trips_through_its_serialized_form_as_a_plain_number() {
        let fidelity = Fidelity::new(0.5).expect("in range");
        let json = serde_json::to_string(&fidelity).expect("a fidelity serializes");

        assert_eq!(json, "0.5");
        assert_eq!(serde_json::from_str::<Fidelity>(&json).expect("a fidelity deserializes"), fidelity);
    }

    #[test]
    fn a_serialized_fidelity_outside_the_range_is_refused() {
        assert!(
            serde_json::from_str::<Fidelity>("1.5").is_err(),
            "deserialization produced a fidelity that could not have been constructed"
        );
        assert!(serde_json::from_str::<Fidelity>("-0.1").is_err());
    }
}
