//! What can be asked for in the color balance family: the rows and the enum that names one of them at a precision.

use serde::{Deserialize, Serialize};

use super::super::catalogue::BIAS_PARAMETER;
use super::super::precision::{FloatPrecision, Precision};
use super::super::simple::SimpleVariant;

use super::{rio, saopaulo};

use crate::providers::profile::EpProfile;

/// Rio, the colour balance model a front end offers first.
pub(crate) const RIO: SimpleVariant = SimpleVariant { codename: "rio", label: "Rio", parameters: &BIAS_PARAMETER };

/// São Paulo — the one variant in the library whose label is not its codename capitalised. The accent and the space
/// belong to the label alone; see `SimpleVariant::codename` for why.
pub(crate) const SAO_PAULO: SimpleVariant =
    SimpleVariant { codename: "saopaulo", label: "São Paulo", parameters: &BIAS_PARAMETER };

/// Every colour balance row, in the order a chooser should offer them. Read by the catalogue.
pub(crate) const ALL: [SimpleVariant; 2] = [RIO, SAO_PAULO];

/// Which colour balance model an operation runs, and at which precision.
///
/// The precision travels **inside** the variant, in the domain these models are published in, rather than beside it
/// as one shared enum plus a check. Both ship FP32 and FP16 and nothing else, so `cb_rio_int8` is not a request this
/// refuses — it does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "codename", content = "precision", rename_all = "lowercase")]
pub enum ColorBalanceVariant {
    /// Rio.
    Rio(FloatPrecision),
    /// São Paulo.
    SaoPaulo(FloatPrecision),
}

impl ColorBalanceVariant {
    /// The precision this variant was asked for, widened to the common currency artifact names are composed from.
    pub fn precision(self) -> Precision {
        match self {
            Self::Rio(precision) | Self::SaoPaulo(precision) => precision.into(),
        }
    }

    /// The execution-provider tuning measured for this variant.
    ///
    /// Each arm's figures, and the conditions they were taken under, are in [`rio::profile`] and
    /// [`saopaulo::profile`].
    pub(crate) fn profile(self) -> EpProfile {
        // A match reaching the model's own file, which is where a measured profile and the prose behind it live: the
        // measurement is of a graph, and the graph is the model's.
        //
        // **Neither variant's answer is evidence for the other's**, which is the thing this seam is most likely to be
        // tidied into forgetting; `saopaulo::profile` says why. Rio's compute-unit answer is in turn the *opposite* of
        // light adjustment's on the same hardware, which is what makes carrying any of them sideways a measurement
        // nobody took.
        match self {
            Self::Rio(precision) => rio::profile(precision.into()),
            // Reaches a file rather than answering `EpProfile::default` inline, and the value being identical is
            // exactly why: a model nobody measured and a model measured to want nothing are the same configuration
            // and different facts, and only the second one is findable.
            Self::SaoPaulo(precision) => saopaulo::profile(precision.into()),
        }
    }

    /// The fixed square this variant's graph accepts, and the only one it accepts.
    pub(crate) const fn canvas(self) -> u32 {
        // A match for the reason `profile` is one: the square is a property of an **export**, so a model re-exported
        // at a different size is presented at its own without the contract moving. It is also deliberately not a
        // field on `SimpleVariant` — that row type is shared across five families and four of them have no canvas at
        // all.
        //
        // **Both variants ship at 656 and the two numbers are not one number**, which is the thing worth knowing
        // before either is changed, and is why both arms reach a model's own file rather than sharing a constant.
        // Rio's is tradeable and São Paulo's is not; the reasoning is at `rio::CANVAS` and `saopaulo::CANVAS`.
        match self {
            Self::Rio(_) => rio::CANVAS,
            Self::SaoPaulo(_) => saopaulo::CANVAS,
        }
    }

    /// The variant this family publishes under `codename` at `precision`, or `None` where it publishes none.
    ///
    /// The inverse of the codename each variant reports, and the reason it is public: a front end that filled a
    /// chooser from [`fn@crate::catalogue`] hands the codename and the precision it was given straight back and gets
    /// the typed variant, restating none of this vocabulary in code of its own.
    ///
    /// A precision outside this contract answers `None` rather than being narrowed to something published — an
    /// INT8 row would name `cb_rio_int8`, which does not exist.
    pub fn from_codename(codename: &str, precision: Precision) -> Option<Self> {
        let precision = FloatPrecision::narrow(precision)?;

        [Self::Rio(precision), Self::SaoPaulo(precision)]
            .into_iter()
            .find(|variant| variant.codename() == codename)
    }

    /// The row backing this variant.
    pub(crate) const fn row(self) -> SimpleVariant {
        // An exhaustive match rather than a lookup, so a variant added later cannot compile without one.
        match self {
            Self::Rio(_) => RIO,
            Self::SaoPaulo(_) => SAO_PAULO,
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
    pub(crate) fn every_variant() -> Vec<ColorBalanceVariant> {
        let mut variants = Vec::new();

        for precision in FloatPrecision::ALL {
            variants.extend([ColorBalanceVariant::Rio(precision), ColorBalanceVariant::SaoPaulo(precision)]);
        }

        variants
    }

    #[test]
    fn every_variant_reports_its_label_codename_and_precision() {
        assert_eq!(ColorBalanceVariant::Rio(FloatPrecision::Fp32).label(), "Rio");
        assert_eq!(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp32).label(), "São Paulo");
        assert_eq!(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp32).codename(), "saopaulo");

        for variant in every_variant() {
            assert!(
                variant.codename().chars().all(|c| c.is_ascii_lowercase()),
                "{variant:?} is not spelled the way an artifact name, a URL and a directory name all accept"
            );
            assert!(matches!(variant.precision(), Precision::Fp32 | Precision::Fp16), "{variant:?}");
        }
    }

    #[test]
    fn every_published_row_is_reachable_from_its_codename() {
        // Driven from `ALL` — the rows the catalogue publishes — rather than from a list written here, so a variant
        // added to the family and left out of `from_codename` fails this rather than going unnoticed.
        for row in ALL {
            for precision in FloatPrecision::ALL {
                let variant = ColorBalanceVariant::from_codename(row.codename, precision.into())
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
                ColorBalanceVariant::from_codename(variant.codename(), variant.precision()),
                Some(variant),
                "{variant:?} did not round-trip through the codename and precision it reports"
            );
        }
    }

    #[test]
    fn every_variant_reports_the_square_its_graph_accepts() {
        // Driven from every published pairing rather than from Rio alone, because a fixed-shape graph has no other
        // size to fall back to: a variant reaching the presentation with a zero or a stale canvas is a run that
        // feeds the graph a tensor it will not take, or one it will take and answer nonsense from.
        for variant in every_variant() {
            assert_eq!(variant.canvas(), 656, "{variant:?} does not report the square its graph was exported at");
        }

        // And each arm reaches its **own** model's constant rather than a shared literal, which is the half a
        // tidy-up breaks: the two numbers agree today and are not one number, so folding them would make a
        // re-export of either silently move the other.
        for precision in FloatPrecision::ALL {
            assert_eq!(ColorBalanceVariant::Rio(precision).canvas(), rio::CANVAS, "{precision:?}");
            assert_eq!(ColorBalanceVariant::SaoPaulo(precision).canvas(), saopaulo::CANVAS, "{precision:?}");
        }
    }

    #[test]
    fn only_rios_fp16_arm_declares_a_profile() {
        // The half of this seam that is silent when it is wrong, in both directions. A profile left at the default
        // still loads Rio and still corrects the right photograph, 3% slower; a profile carried across to São
        // Paulo, whose own sweep could not separate that setting from its run-to-run spread, is how a model gets
        // pessimised on the strength of a neighbour's numbers — and this family's neighbour disagrees with light
        // adjustment about the Neural Engine, so there is no house answer to fall back on.
        //
        // **São Paulo's defaults are a measurement rather than an absence**, which is why this asserts the *shape*
        // of the seam rather than that one arm is unwritten: both arms reach a model's own file, and only one of
        // the two files answers with anything but the provider's defaults.
        for variant in every_variant() {
            let declared = variant.profile().tuning();

            let measured = matches!(variant, ColorBalanceVariant::Rio(FloatPrecision::Fp16));
            assert_eq!(
                declared != EpProfile::default(),
                measured,
                "{variant:?} declared {declared:?}, which is not what was measured for it"
            );
        }

        // Asserted beside it, where a tidy-up unifying the two arms would have to walk past it: São Paulo's
        // options are the provider defaults at **both** precisions, with nothing added and nothing removed, and
        // Rio's FP16 arm still declares the specialization its own graph earned.
        for precision in FloatPrecision::ALL {
            assert_eq!(
                ColorBalanceVariant::SaoPaulo(precision).profile().tuning(),
                EpProfile::default(),
                "São Paulo at {precision:?} carries a setting its own sweep could not separate from noise"
            );
        }

        assert_eq!(
            ColorBalanceVariant::Rio(FloatPrecision::Fp16).profile().coreml_specialization,
            crate::providers::profile::CoreMlSpecialization::FastPrediction,
            "Rio's FP16 arm stopped declaring the fast-prediction specialization measured for it"
        );
    }

    #[test]
    fn a_pairing_this_family_does_not_publish_names_nothing() {
        assert_eq!(
            ColorBalanceVariant::from_codename("paris", Precision::Fp32),
            None,
            "a light adjustment codename named a colour balance variant"
        );
        assert_eq!(ColorBalanceVariant::from_codename("", Precision::Fp32), None);
        assert_eq!(
            ColorBalanceVariant::from_codename(&ALL[0].codename.to_uppercase(), Precision::Fp32),
            None,
            "a codename was matched by something other than the spelling that is published"
        );

        for row in ALL {
            assert_eq!(
                ColorBalanceVariant::from_codename(row.codename, Precision::Int8),
                None,
                "{} named an INT8 build the family does not publish",
                row.codename
            );
        }
    }
}
