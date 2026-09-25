//! What can be asked for in the colorization family: the rows and the enum that names one of them at a precision.

use serde::{Deserialize, Serialize};

use super::super::precision::{FloatPrecision, Precision};
use super::super::simple::SimpleVariant;
use super::process::Contract;
use super::{delhi, jaipur, mumbai};

use crate::providers::profile::EpProfile;

/// Delhi, the colorization model a front end offers first.
pub(crate) const DELHI: SimpleVariant = SimpleVariant { codename: "delhi", label: "Delhi", parameters: &[] };

pub(crate) const MUMBAI: SimpleVariant = SimpleVariant { codename: "mumbai", label: "Mumbai", parameters: &[] };

pub(crate) const JAIPUR: SimpleVariant = SimpleVariant { codename: "jaipur", label: "Jaipur", parameters: &[] };

/// Every colorization row, in the order a chooser should offer them. Read by the catalogue.
pub(crate) const ALL: [SimpleVariant; 3] = [DELHI, MUMBAI, JAIPUR];

/// Which colorization model an operation runs, and at which precision.
///
/// The precision travels **inside** the variant, in the domain these models are published in. All three ship FP32
/// and FP16 and nothing else, so `cl_delhi_int8` is not a request this refuses — it does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "codename", content = "precision", rename_all = "lowercase")]
pub enum ColorizationVariant {
    /// Delhi.
    Delhi(FloatPrecision),
    /// Mumbai.
    Mumbai(FloatPrecision),
    /// Jaipur.
    Jaipur(FloatPrecision),
}

impl ColorizationVariant {
    /// The precision this variant was asked for, widened to the common currency artifact names are composed from.
    pub fn precision(self) -> Precision {
        match self {
            Self::Delhi(precision) | Self::Mumbai(precision) | Self::Jaipur(precision) => precision.into(),
        }
    }

    /// The execution-provider tuning measured for this variant: Delhi's is [`delhi::profile`], Mumbai's
    /// [`mumbai::profile`] and Jaipur's [`jaipur::profile`].
    pub(crate) fn profile(self) -> EpProfile {
        // A match reaching the model's own file, which is where `models`' tiers put a measured profile and the prose
        // behind it: the measurement is of a graph, and the graph is the model's.
        //
        // Three files carrying the same settings rather than one shared constant, because they reached them three
        // different ways. Mumbai's were measured on Mumbai's graph. Jaipur's were measured separately on a DeOldify
        // graph, a different architecture that arrived at the same answer. Delhi's were not measured at all: the
        // reference declares Mumbai's beside it. A shared constant would erase exactly that difference, and carrying
        // one model's answer to another because both do the same job is how a profile ends up pessimising a graph it
        // was never measured against.
        match self {
            Self::Delhi(precision) => delhi::profile(precision.into()),
            Self::Mumbai(precision) => mumbai::profile(precision.into()),
            Self::Jaipur(precision) => jaipur::profile(precision.into()),
        }
    }

    /// Which of the family's two graph contracts this variant's model speaks.
    pub(super) const fn contract(self) -> Contract {
        // An exhaustive match rather than a field on the row, for the reason `row` is one, and because the contract is
        // a property of an export: a model re-exported to the other contract moves arms without the pipeline moving.
        match self {
            Self::Delhi(_) | Self::Mumbai(_) => Contract::Ab,
            Self::Jaipur(_) => Contract::Rgb,
        }
    }

    /// The variant this family publishes under `codename` at `precision`, or `None` where it publishes none.
    ///
    /// The inverse of the codename each variant reports: a front end that filled a chooser from [`fn@crate::catalogue`]
    /// hands the codename and the precision it was given straight back and gets the typed variant. A precision this
    /// family does not publish answers `None` rather than being narrowed to one it does.
    pub fn from_codename(codename: &str, precision: Precision) -> Option<Self> {
        let precision = FloatPrecision::narrow(precision)?;

        [Self::Delhi(precision), Self::Mumbai(precision), Self::Jaipur(precision)]
            .into_iter()
            .find(|variant| variant.codename() == codename)
    }

    /// The row backing this variant.
    pub(crate) const fn row(self) -> SimpleVariant {
        // An exhaustive match rather than a lookup, so a variant added later cannot compile without one.
        match self {
            Self::Delhi(_) => DELHI,
            Self::Mumbai(_) => MUMBAI,
            Self::Jaipur(_) => JAIPUR,
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
    pub(crate) fn every_variant() -> Vec<ColorizationVariant> {
        let mut variants = Vec::new();

        for precision in FloatPrecision::ALL {
            variants.extend([
                ColorizationVariant::Delhi(precision),
                ColorizationVariant::Mumbai(precision),
                ColorizationVariant::Jaipur(precision),
            ]);
        }

        variants
    }

    #[test]
    fn every_variant_reports_its_label_codename_and_precision() {
        assert_eq!(ColorizationVariant::Delhi(FloatPrecision::Fp32).label(), "Delhi");
        assert_eq!(ColorizationVariant::Mumbai(FloatPrecision::Fp32).label(), "Mumbai");
        assert_eq!(ColorizationVariant::Jaipur(FloatPrecision::Fp32).label(), "Jaipur");

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
                let variant = ColorizationVariant::from_codename(row.codename, precision.into())
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
                ColorizationVariant::from_codename(variant.codename(), variant.precision()),
                Some(variant),
                "{variant:?} did not round-trip through the codename and precision it reports"
            );
        }
    }

    #[test]
    fn a_pairing_this_family_does_not_publish_names_nothing() {
        assert_eq!(
            ColorizationVariant::from_codename("stockholm", Precision::Fp32),
            None,
            "a denoise codename named a colorization variant"
        );
        assert_eq!(ColorizationVariant::from_codename("", Precision::Fp32), None);
        assert_eq!(
            ColorizationVariant::from_codename(&ALL[0].codename.to_uppercase(), Precision::Fp32),
            None,
            "a codename was matched by something other than the spelling that is published"
        );

        for row in ALL {
            assert_eq!(
                ColorizationVariant::from_codename(row.codename, Precision::Int8),
                None,
                "{} named an INT8 build the family does not publish",
                row.codename
            );
        }
    }
}
