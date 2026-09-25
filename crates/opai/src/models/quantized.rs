//! The mechanism the four bounded per-run parameters — [`Scale`](super::Scale), [`Strength`](super::Strength),
//! [`Bias`](super::Bias) and [`Fidelity`](super::Fidelity) — share: quantized storage, a rejecting and a clamping
//! constructor, and the plain-number serialized form.

// Generated rather than written out four times, and only the mechanism. What differs between the four is the argument
// for each: why one is signed, which end of its range a `NaN` falls back to, which reference formatting its rendering
// reproduces. That stays beside each type — its bounds with their own docs, the `NaN` fallback's reason passed in as
// the doc of `clamped`, and its own `Display`. What is generated here is the part that was identical down to the
// comments: the containment test, the quantization, the accessor and the two serde conversions.
//
// `Confidence` is not one of them: it has no clamping constructor and a different resolution, so it keeps its own.

/// How many quantization steps one whole unit of every bounded parameter is divided into.
///
/// Three decimal places, one resolution for every per-run parameter in the library rather than each family deciding
/// its own. For the 1-8 upscale range it is exactly the four significant figures the reference implementation formats
/// its ids with, and it is finer than anything the reference's sliders and text fields can produce for the others —
/// they step in whole percent — so no reachable input is rounded.
pub(super) const STEPS_PER_UNIT: f64 = 1000.0;

/// Renders a quantized count of thousandths with no trailing zeros and no decimal point where there is no fraction:
/// `0`, `-0.5`, `2.25`, `1.667`.
///
/// The one spelling of a cache tag's number, so that a tag's spelling does not depend on which parameter wrote it.
/// The sign is written separately from the magnitude rather than falling out of signed integer division, which would
/// lose it: -500 steps divides to a whole part of 0, so `-0.5` would render as `0.5` and share a cache tag with its
/// positive counterpart. A magnitude of zero never renders a sign, so there is one spelling of zero.
pub(super) fn write_thousandths(f: &mut std::fmt::Formatter<'_>, negative: bool, magnitude: u16) -> std::fmt::Result {
    let sign = if negative && magnitude != 0 { "-" } else { "" };
    let whole = magnitude / 1000;
    let fraction = magnitude % 1000;

    if fraction == 0 {
        return write!(f, "{sign}{whole}");
    }

    // Trimmed by dividing rather than by formatting into a scratch string and trimming it: the trailing zeros are
    // decided by the value, so the divisor is too, and `Display` here is on the path every cache tag takes.
    match (fraction % 100, fraction % 10) {
        (0, _) => write!(f, "{sign}{whole}.{}", fraction / 100),
        (_, 0) => write!(f, "{sign}{whole}.{:02}", fraction / 10),
        _ => write!(f, "{sign}{whole}.{fraction:03}"),
    }
}

/// Generates a bounded parameter's constructors, accessor, `NAME` and serde conversions.
///
/// The type is a struct with one field, `steps`, of the integer type given, and an inherent `MIN` and `MAX` of its
/// own. The doc comment before `not_a_number` is appended to `clamped`'s, and should say why a `NaN` bounds to the
/// value it names.
macro_rules! quantized_param {
    (
        $ty:ident($steps:ty),
        name: $name:literal,
        $(#[$fallback_doc:meta])*
        not_a_number: $fallback:expr $(,)?
    ) => {
        // The bound is what makes `quantize`'s cast lossless, so it is checked where the cast is written rather than
        // trusted.
        const _: () = assert!(
            <$steps>::MIN as f64 <= $ty::MIN * $crate::models::quantized::STEPS_PER_UNIT
                && $ty::MAX * $crate::models::quantized::STEPS_PER_UNIT <= <$steps>::MAX as f64,
            "the permitted range does not fit the storage"
        );

        impl $ty {
            /// The parameter's name, as [`RangeError`](crate::RangeError) and the catalogue spell it.
            pub const NAME: &'static str = $name;

            #[doc = concat!("A ", $name, ", refusing anything outside the permitted range.")]
            ///
            /// What a command-line flag wants: its input was never bounded, so an out-of-range value is a mistake to
            /// report rather than a number to correct silently.
            ///
            /// # Errors
            ///
            #[doc = concat!(
                "Returns [`RangeError`](crate::RangeError) naming the permitted range when `value` is below [`",
                stringify!($ty), "::MIN`], above [`", stringify!($ty), "::MAX`], or not a number."
            )]
            pub fn new(value: f64) -> Result<Self, $crate::RangeError> {
                // A containment test rather than two comparisons, so that `NaN` — which is neither below the minimum
                // nor above the maximum — is refused instead of quantized into an arbitrary integer.
                if !(Self::MIN..=Self::MAX).contains(&value) {
                    return Err($crate::RangeError { parameter: Self::NAME, value, min: Self::MIN, max: Self::MAX });
                }

                Ok(Self::quantize(value))
            }

            #[doc = concat!("A ", $name, ", bounding anything outside the permitted range to the nearest end of it.")]
            ///
            /// What a GUI slider wants: its input is already bounded by the control, so an error would be one the
            /// caller cannot act on.
            ///
            $(#[$fallback_doc])*
            pub fn clamped(value: f64) -> Self {
                if value.is_nan() {
                    return Self::quantize($fallback);
                }

                Self::quantize(value.clamp(Self::MIN, Self::MAX))
            }

            #[doc = concat!("The ", $name, " as a number, at the value it was quantized to.")]
            pub fn get(self) -> f64 {
                f64::from(self.steps) / $crate::models::quantized::STEPS_PER_UNIT
            }

            /// Quantizes an already-bounded value. Private because the bound is what makes the cast lossless, which
            /// the assertion beside this impl checks against the storage.
            fn quantize(bounded: f64) -> Self {
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "bounded to MIN..=MAX before this is called, and that range fits the storage"
                )]
                Self { steps: (bounded * $crate::models::quantized::STEPS_PER_UNIT).round() as $steps }
            }
        }

        impl TryFrom<f64> for $ty {
            type Error = $crate::RangeError;

            #[doc = concat!(
                "The path a deserialized ", $name, " takes, so a persisted or `invoke`d value is validated exactly as \
                 a directly constructed one is — the rejecting way, because a payload is not a slider."
            )]
            fn try_from(value: f64) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$ty> for f64 {
            #[doc = concat!(
                "The serialized form: a plain number, so a front end reads the ", $name, " it set rather than this \
                 type's storage."
            )]
            fn from(value: $ty) -> Self {
                value.get()
            }
        }
    };
}

pub(super) use quantized_param;

/// The tests every bounded parameter answers alike, generated into a `quantized` module beside the type's own.
///
/// Each argument is a value on the type's own range: `accepted` are carried exactly, `refused` is one value below
/// the range and one above it, `range` is how the error message states the bounds, `not_a_number` is what clamping a
/// `NaN` produces, `quantized` is a value finer than three decimals and what it quantizes to, `alike` are two values
/// that quantize to one, `serialized` is a value and its JSON, and `refused_serialized` are JSON numbers outside the
/// range.
#[cfg(test)]
macro_rules! quantized_param_tests {
    (
        $ty:ident {
            accepted: [$($accepted:expr),+ $(,)?],
            refused: [$below:expr, $above:expr],
            range: $range:literal,
            not_a_number: $fallback:expr,
            quantized: $finer:expr => $quantized:expr,
            alike: [$low:expr, $high:expr],
            serialized: $value:expr => $json:literal,
            refused_serialized: [$($refused_json:literal),+ $(,)?] $(,)?
        }
    ) => {
        mod quantized {
            use super::*;
            use $crate::models::test_support::hash_of;

            #[test]
            fn a_value_inside_the_range_is_taken_as_given() {
                for value in [$($accepted),+] {
                    let accepted = $ty::new(value).expect("a value in range is accepted");
                    assert_eq!(accepted.get(), value, "{value} was not carried at the value supplied");
                }
            }

            #[test]
            fn an_out_of_range_value_is_refused_when_rejection_was_asked_for() {
                for value in [$below, $above] {
                    let error = $ty::new(value).expect_err("an out-of-range value was accepted");

                    assert_eq!(error.value, value);
                    assert_eq!((error.min, error.max), ($ty::MIN, $ty::MAX), "the error did not name the range");
                    assert!(error.to_string().contains($range), "the message did not state the range: {error}");
                    assert!(
                        error.to_string().starts_with($ty::NAME),
                        "the message did not name the parameter: {error}"
                    );
                }
            }

            #[test]
            fn a_value_that_is_not_a_number_is_refused_rather_than_quantized() {
                assert!($ty::new(f64::NAN).is_err());
                assert!($ty::new(f64::INFINITY).is_err());
                assert!($ty::new(f64::NEG_INFINITY).is_err());
            }

            #[test]
            fn an_out_of_range_value_is_bounded_when_clamping_was_asked_for() {
                assert_eq!($ty::clamped($below).get(), $ty::MIN);
                assert_eq!($ty::clamped($above).get(), $ty::MAX);
            }

            #[test]
            fn clamping_a_value_that_is_not_a_number_still_produces_a_value_in_range() {
                // Infallible means infallible: no argument yields something out of range, which is what lets
                // everything downstream stop re-checking.
                assert_eq!($ty::clamped(f64::NAN).get(), $fallback);
            }

            #[test]
            fn the_published_bounds_are_the_ones_the_constructor_enforces() {
                assert!($ty::new($ty::MIN).is_ok());
                assert!($ty::new($ty::MAX).is_ok());
                assert!($ty::new($ty::MIN - 0.001).is_err());
                assert!($ty::new($ty::MAX + 0.001).is_err());
            }

            #[test]
            fn a_value_finer_than_three_decimals_is_quantized_on_acceptance() {
                assert_eq!($ty::new($finer).expect("in range").get(), $quantized);
            }

            #[test]
            fn two_values_that_quantize_alike_are_one_value() {
                let low = $ty::new($low).expect("in range");
                let high = $ty::new($high).expect("in range");

                assert_eq!(low, high, "two values that quantize alike stayed distinguishable");
                assert_eq!(hash_of(&low), hash_of(&high));
            }

            #[test]
            fn every_accepted_value_compares_equal_to_itself() {
                // The property `f64` fails and the reason the storage is an integer: a value that did not compare
                // equal to itself could not identify anything, so nothing carrying one could be keyed on.
                let mut value = $ty::MIN;
                while value <= $ty::MAX {
                    let accepted = $ty::new(value).expect("in range");
                    assert_eq!(accepted, accepted, "{value} did not compare equal to itself");
                    value += 0.001;
                }
            }

            #[test]
            fn a_value_round_trips_through_its_serialized_form_as_a_plain_number() {
                let accepted = $ty::new($value).expect("in range");
                let json = serde_json::to_string(&accepted).expect("a bounded parameter serializes");

                assert_eq!(json, $json);
                assert_eq!(serde_json::from_str::<$ty>(&json).expect("a bounded parameter deserializes"), accepted);
            }

            #[test]
            fn a_serialized_value_outside_the_range_is_refused() {
                for json in [$($refused_json),+] {
                    assert!(
                        serde_json::from_str::<$ty>(json).is_err(),
                        "deserializing {json} produced a value that could not have been constructed"
                    );
                }
            }
        }
    };
}

#[cfg(test)]
pub(super) use quantized_param_tests;

#[cfg(test)]
mod tests {
    use super::write_thousandths;

    /// `write_thousandths` as a string, through a `Display` wrapper.
    fn rendered(negative: bool, magnitude: u16) -> String {
        struct Thousandths(bool, u16);

        impl std::fmt::Display for Thousandths {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write_thousandths(f, self.0, self.1)
            }
        }

        Thousandths(negative, magnitude).to_string()
    }

    #[test]
    fn a_whole_value_renders_with_no_decimal_part() {
        assert_eq!(rendered(false, 0), "0");
        assert_eq!(rendered(false, 1000), "1");
        assert_eq!(rendered(false, 8000), "8");
        assert_eq!(rendered(true, 1000), "-1");
    }

    #[test]
    fn a_fractional_value_renders_without_trailing_zeros() {
        assert_eq!(rendered(false, 500), "0.5");
        assert_eq!(rendered(false, 2250), "2.25");
        assert_eq!(rendered(false, 3750), "3.75");
        assert_eq!(rendered(false, 1667), "1.667");
        assert_eq!(rendered(false, 2050), "2.05");
        assert_eq!(rendered(false, 1), "0.001");
    }

    #[test]
    fn a_negative_value_keeps_its_sign_even_with_no_whole_part() {
        assert_eq!(rendered(true, 500), "-0.5");
        assert_eq!(rendered(true, 350), "-0.35");
        assert_eq!(rendered(true, 1), "-0.001");
    }

    #[test]
    fn zero_has_one_spelling_whatever_its_sign() {
        assert_eq!(rendered(true, 0), "0");
    }
}
