//! What can be asked for in the light adjustment family: the rows and the enum that names one of them at a precision.

use serde::{Deserialize, Serialize};

use super::super::catalogue::BIAS_PARAMETER;
use super::super::precision::{FloatPrecision, Precision};
use super::super::simple::SimpleVariant;

use super::{lyon, paris};

use crate::providers::profile::EpProfile;

/// Paris, the light adjustment model a front end offers first.
pub(crate) const PARIS: SimpleVariant =
    SimpleVariant { codename: "paris", label: "Paris", parameters: &BIAS_PARAMETER };

pub(crate) const LYON: SimpleVariant = SimpleVariant { codename: "lyon", label: "Lyon", parameters: &BIAS_PARAMETER };

/// Every light adjustment row, in the order a chooser should offer them. Read by the catalogue.
pub(crate) const ALL: [SimpleVariant; 2] = [PARIS, LYON];

/// Which light adjustment model an operation runs, and at which precision.
///
/// The precision travels **inside** the variant, in the domain these models are published in. Both ship FP32 and
/// FP16 and nothing else, so `la_paris_int8` is not a request this refuses — it does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "codename", content = "precision", rename_all = "lowercase")]
pub enum LightAdjustmentVariant {
    /// Paris.
    Paris(FloatPrecision),
    /// Lyon.
    Lyon(FloatPrecision),
}

impl LightAdjustmentVariant {
    /// The precision this variant was asked for, widened to the common currency artifact names are composed from.
    pub fn precision(self) -> Precision {
        match self {
            Self::Paris(precision) | Self::Lyon(precision) => precision.into(),
        }
    }

    /// The execution-provider tuning measured for this variant: Paris' is [`paris::profile`] and Lyon's
    /// [`lyon::profile`].
    pub(crate) fn profile(self) -> EpProfile {
        // A match reaching the model's own file, which is where `models`' tiers put a measured profile and the prose
        // behind it: the measurement is of a graph, and the graph is the model's.
        //
        // The two rows agree on the compute units — CoreML off the Neural Engine, at FP16 only — and disagree on the
        // execution mode, sequential for Paris (also at FP16 only) and none for Lyon; both are measured rather than
        // accidental. Two graphs of completely different architectures reached the same compute-unit answer on the
        // same hardware, separately, and neither is evidence for the other; on the execution mode they reached
        // opposite answers, because Lyon compiles into a single fused CoreML operation and has nothing for an inter-op
        // pool to schedule where Paris has 78 nodes. Carrying either model's answer over to the other on the strength
        // of both adjusting light is how a profile ends up pessimising a graph it was never measured against.
        match self {
            Self::Paris(precision) => paris::profile(precision.into()),
            Self::Lyon(precision) => lyon::profile(precision.into()),
        }
    }

    /// The fixed square this variant's graph accepts, and the only one it accepts.
    pub(crate) const fn canvas(self) -> u32 {
        // A match reaching the model's own file, for the reason `profile` is one: the square is a property of an
        // **export**, so a model re-exported at a different size is presented at its own without the contract
        // moving. Both variants ship at 1024 and arrived there by different arguments — Paris' is about where its
        // global branch's answer sits, Lyon's about what a smaller square does to a window-attention transformer —
        // which is exactly why the value is not a constant in `process`.
        //
        // Deliberately **not** a field on `SimpleVariant`. That row type is shared by sixteen variants across seven
        // families and five of those families have no canvas at all, so a field there would be a value those five
        // must supply and nobody reads.
        match self {
            Self::Paris(_) => paris::CANVAS,
            Self::Lyon(_) => lyon::CANVAS,
        }
    }

    /// The variant this family publishes under `codename` at `precision`, or `None` where it publishes none.
    ///
    /// The inverse of the codename each variant reports: a front end that filled a chooser from [`fn@crate::catalogue`]
    /// hands the codename and the precision it was given straight back and gets the typed variant. A precision this
    /// family does not publish answers `None` rather than being narrowed to one it does.
    pub fn from_codename(codename: &str, precision: Precision) -> Option<Self> {
        let precision = FloatPrecision::narrow(precision)?;

        [Self::Paris(precision), Self::Lyon(precision)]
            .into_iter()
            .find(|variant| variant.codename() == codename)
    }

    /// The row backing this variant.
    pub(crate) const fn row(self) -> SimpleVariant {
        // An exhaustive match rather than a lookup, so a variant added later cannot compile without one.
        match self {
            Self::Paris(_) => PARIS,
            Self::Lyon(_) => LYON,
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
    pub(crate) fn every_variant() -> Vec<LightAdjustmentVariant> {
        let mut variants = Vec::new();

        for precision in FloatPrecision::ALL {
            variants.extend([LightAdjustmentVariant::Paris(precision), LightAdjustmentVariant::Lyon(precision)]);
        }

        variants
    }

    #[test]
    fn every_variant_reports_its_label_codename_and_precision() {
        assert_eq!(LightAdjustmentVariant::Paris(FloatPrecision::Fp32).label(), "Paris");
        assert_eq!(LightAdjustmentVariant::Lyon(FloatPrecision::Fp32).label(), "Lyon");

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
                let variant = LightAdjustmentVariant::from_codename(row.codename, precision.into())
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
                LightAdjustmentVariant::from_codename(variant.codename(), variant.precision()),
                Some(variant),
                "{variant:?} did not round-trip through the codename and precision it reports"
            );
        }
    }

    #[test]
    fn every_variant_reports_a_canvas() {
        // Driven from the traversal rather than from a list written here, so a variant added to the family and left
        // out of the match fails this. Which settings each model declares is pinned in its own directory, and the
        // profile seam's own traversal is the test below.
        for variant in every_variant() {
            assert_eq!(variant.canvas(), 1024, "{variant:?} runs at a square nothing exported it at");
        }
    }

    #[test]
    fn only_the_fp16_arms_declare_a_profile() {
        // Both halves of the precision split, across the whole family. Every figure behind either declaration is an
        // FP16 measurement — an FP32 MLProgram cannot reach the Neural Engine in the first place, so there is
        // nothing at FP32 for a compute-unit restriction to buy — and a profile applied to a precision it was not
        // measured at is a slower session with nothing to report it.
        for variant in every_variant() {
            let declared = variant.profile().tuning() != EpProfile::default();

            assert_eq!(
                declared,
                variant.precision() == Precision::Fp16,
                "{variant:?} declared a profile at a precision nothing measured it at"
            );
        }
    }

    #[test]
    fn a_pairing_this_family_does_not_publish_names_nothing() {
        assert_eq!(
            LightAdjustmentVariant::from_codename("rio", Precision::Fp32),
            None,
            "a colour balance codename named a light adjustment variant"
        );
        assert_eq!(LightAdjustmentVariant::from_codename("", Precision::Fp32), None);
        assert_eq!(
            LightAdjustmentVariant::from_codename(&ALL[0].codename.to_uppercase(), Precision::Fp32),
            None,
            "a codename was matched by something other than the spelling that is published"
        );

        for row in ALL {
            assert_eq!(
                LightAdjustmentVariant::from_codename(row.codename, Precision::Int8),
                None,
                "{} named an INT8 build the family does not publish",
                row.codename
            );
        }
    }
}
