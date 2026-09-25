//! What can be asked for in the sharpen family: the rows and the enum that names one of them at a precision.

use serde::{Deserialize, Serialize};

use super::super::catalogue::STRENGTH_PARAMETER;
use super::super::precision::{FloatPrecision, Precision};
use super::super::simple::SimpleVariant;

use super::{moscow, novgorod, petersburg};

use crate::providers::profile::EpProfile;

/// Moscow, the sharpener a front end offers first.
pub(crate) const MOSCOW: SimpleVariant =
    SimpleVariant { codename: "moscow", label: "Moscow", parameters: &STRENGTH_PARAMETER };

// The label the reference application shows, rather than the "St. Petersburg" the city is usually called.
// Transcribed from its variant table, not expanded.
pub(crate) const PETERSBURG: SimpleVariant =
    SimpleVariant { codename: "petersburg", label: "Petersburg", parameters: &STRENGTH_PARAMETER };

pub(crate) const NOVGOROD: SimpleVariant =
    SimpleVariant { codename: "novgorod", label: "Novgorod", parameters: &STRENGTH_PARAMETER };

/// Every sharpen row, in the order a chooser should offer them. Read by the catalogue.
pub(crate) const ALL: [SimpleVariant; 3] = [MOSCOW, PETERSBURG, NOVGOROD];

/// Which sharpen model an operation runs, and at which precision.
///
/// The precision travels **inside** the variant, in the domain these models are published in. All three ship FP32
/// and FP16 and nothing else, so `sh_moscow_int8` is not a request this refuses — it does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "codename", content = "precision", rename_all = "lowercase")]
pub enum SharpenVariant {
    /// Moscow.
    Moscow(FloatPrecision),
    /// Petersburg.
    Petersburg(FloatPrecision),
    /// Novgorod.
    Novgorod(FloatPrecision),
}

impl SharpenVariant {
    /// The precision this variant was asked for, widened to the common currency artifact names are composed from.
    pub fn precision(self) -> Precision {
        match self {
            Self::Moscow(precision) | Self::Petersburg(precision) | Self::Novgorod(precision) => precision.into(),
        }
    }

    /// The execution-provider tuning measured for this variant: Moscow's is [`moscow::profile`], Petersburg's
    /// [`petersburg::profile`] and Novgorod's [`novgorod::profile`].
    pub(crate) fn profile(self) -> EpProfile {
        // A match reaching the model's own file, which is where `models`' tiers put a measured profile and the prose
        // behind it: the measurement is of a graph, and the graph is the model's. Moscow and Novgorod reach the same
        // answer as one another, measured separately; Petersburg is a different architecture and is measured on its
        // own.
        match self {
            Self::Moscow(precision) => moscow::profile(precision.into()),
            Self::Petersburg(precision) => petersburg::profile(precision.into()),
            Self::Novgorod(precision) => novgorod::profile(precision.into()),
        }
    }

    /// The magnitude past which a tile's raw output is discarded in favour of its input, or `None` for a model that
    /// is not guarded: Petersburg's is [`petersburg::GUARD`], and Moscow and Novgorod have none.
    pub(crate) const fn guard(self) -> Option<f32> {
        // Deliberately **not** a field on `SimpleVariant`: that row type is shared across five families, and a
        // threshold there would be a value every other row has to supply and nobody reads. The two Restormer models
        // run unguarded, as they do in the reference.
        match self {
            Self::Petersburg(_) => Some(petersburg::GUARD),
            Self::Moscow(_) | Self::Novgorod(_) => None,
        }
    }

    /// The variant this family publishes under `codename` at `precision`, or `None` where it publishes none.
    ///
    /// The inverse of the codename each variant reports: a front end that filled a chooser from [`fn@crate::catalogue`]
    /// hands the codename and the precision it was given straight back and gets the typed variant. A precision this
    /// family does not publish answers `None` rather than being narrowed to one it does.
    pub fn from_codename(codename: &str, precision: Precision) -> Option<Self> {
        let precision = FloatPrecision::narrow(precision)?;

        [Self::Moscow(precision), Self::Petersburg(precision), Self::Novgorod(precision)]
            .into_iter()
            .find(|variant| variant.codename() == codename)
    }

    /// The row backing this variant.
    pub(crate) const fn row(self) -> SimpleVariant {
        // An exhaustive match rather than a lookup, so a variant added later cannot compile without one.
        match self {
            Self::Moscow(_) => MOSCOW,
            Self::Petersburg(_) => PETERSBURG,
            Self::Novgorod(_) => NOVGOROD,
        }
    }

    /// The display text a user sees for this variant, with no precision appended.
    pub(crate) const fn label(self) -> &'static str {
        self.row().label
    }

    /// The developer-facing identifier this variant's artifact name is composed from.
    pub(crate) const fn codename(self) -> &'static str {
        self.row().codename
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Every variant and precision pairing this family publishes.
    pub(crate) fn every_variant() -> Vec<SharpenVariant> {
        let mut variants = Vec::new();

        for precision in FloatPrecision::ALL {
            variants.extend([
                SharpenVariant::Moscow(precision),
                SharpenVariant::Petersburg(precision),
                SharpenVariant::Novgorod(precision),
            ]);
        }

        variants
    }

    #[test]
    fn every_variant_reports_its_label_codename_and_precision() {
        assert_eq!(SharpenVariant::Moscow(FloatPrecision::Fp32).label(), "Moscow");
        assert_eq!(SharpenVariant::Petersburg(FloatPrecision::Fp32).label(), "Petersburg");
        assert_eq!(SharpenVariant::Novgorod(FloatPrecision::Fp32).label(), "Novgorod");

        for variant in every_variant() {
            assert_eq!(variant.codename(), variant.label().to_lowercase(), "{variant:?}");
            assert!(matches!(variant.precision(), Precision::Fp32 | Precision::Fp16), "{variant:?}");
        }
    }

    #[test]
    fn every_published_row_is_reachable_from_its_codename() {
        // Driven from `ALL` — the rows the catalogue publishes — rather than from a list written here, so a variant
        // added to the family and left out of `from_codename` fails this rather than going unnoticed.
        for row in ALL {
            for precision in FloatPrecision::ALL {
                let variant = SharpenVariant::from_codename(row.codename, precision.into())
                    .unwrap_or_else(|| panic!("{} is published but its codename names no variant", row.codename));

                assert_eq!(variant.codename(), row.codename);
                assert_eq!(variant.precision(), Precision::from(precision));
            }
        }

        // The other direction, and what makes a row removed from the published table fail rather than simply
        // shrink every traversal that reads it: a variant the enum can still name but no row publishes is one the
        // catalogue offers no way to choose.
        for variant in every_variant() {
            assert!(
                ALL.iter().any(|row| row.codename == variant.codename()),
                "{variant:?} can be named but no published row offers it"
            );
        }

        for variant in every_variant() {
            assert_eq!(
                SharpenVariant::from_codename(variant.codename(), variant.precision()),
                Some(variant),
                "{variant:?} did not round-trip through the codename and precision it reports"
            );
        }
    }

    #[test]
    fn only_petersburg_is_guarded() {
        // Driven from the traversal, so a variant added to the family and left out of the match fails this.
        for variant in every_variant() {
            let guarded = matches!(variant, SharpenVariant::Petersburg(_));

            assert_eq!(variant.guard().is_some(), guarded, "{variant:?}");
        }
    }

    #[test]
    fn every_variant_reaches_its_own_models_profile() {
        // The seam's traversal. Which settings each model declares, and why, is pinned in its own directory; what is
        // pinned here is that each arm reaches it, and that the precision split holds across the family.
        for variant in every_variant() {
            match variant {
                SharpenVariant::Petersburg(precision) => {
                    assert_eq!(variant.profile(), petersburg::profile(precision.into()), "{variant:?}");
                }
                SharpenVariant::Moscow(FloatPrecision::Fp16) | SharpenVariant::Novgorod(FloatPrecision::Fp16) => {
                    assert_ne!(variant.profile(), EpProfile::default(), "{variant:?} lost its measured profile");
                }
                SharpenVariant::Moscow(FloatPrecision::Fp32) | SharpenVariant::Novgorod(FloatPrecision::Fp32) => {
                    assert_eq!(variant.profile(), EpProfile::default(), "{variant:?} declared a setting at FP32");
                }
            }
        }
    }

    #[test]
    fn a_pairing_this_family_does_not_publish_names_nothing() {
        assert_eq!(
            SharpenVariant::from_codename("stockholm", Precision::Fp32),
            None,
            "a denoise codename named a sharpen variant"
        );
        assert_eq!(SharpenVariant::from_codename("", Precision::Fp32), None);
        assert_eq!(
            SharpenVariant::from_codename(&ALL[0].codename.to_uppercase(), Precision::Fp32),
            None,
            "a codename was matched by something other than the spelling that is published"
        );

        for row in ALL {
            assert_eq!(
                SharpenVariant::from_codename(row.codename, Precision::Int8),
                None,
                "{} named an INT8 build the family does not publish",
                row.codename
            );
        }
    }
}
