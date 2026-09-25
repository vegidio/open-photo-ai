//! What can be asked for in the face recovery family: the rows and the enum that names one of them at a precision.

use serde::{Deserialize, Serialize};

use super::super::catalogue::{FACES_AND_FIDELITY_PARAMETERS, FACES_PARAMETER};
use super::super::fidelity::Fidelity;
use super::super::precision::{FloatPrecision, Precision};
use super::super::simple::SimpleVariant;

use super::{athens, santorini};

use crate::providers::profile::EpProfile;

/// Athens, the CodeFormer-style restorer a front end offers first.
pub(crate) const ATHENS: SimpleVariant =
    SimpleVariant { codename: "athens", label: "Athens", parameters: &FACES_AND_FIDELITY_PARAMETERS };

/// Santorini.
pub(crate) const SANTORINI: SimpleVariant =
    SimpleVariant { codename: "santorini", label: "Santorini", parameters: &FACES_PARAMETER };

/// Every face recovery row, in the order a chooser should offer them. Read by the catalogue.
pub(crate) const ALL: [SimpleVariant; 2] = [ATHENS, SANTORINI];

// The precision travels inside the variant rather than beside it as one shared enum plus a check.
/// Which face recovery model an operation runs, and at which precision.
///
/// The precision travels **inside** the variant, in the domain these models are published in. Both ship FP32 and FP16
/// and nothing else, so `fr_athens_int8` is not a request this refuses — it does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "codename", content = "precision", rename_all = "lowercase")]
pub enum FaceRecoveryVariant {
    /// Athens.
    Athens(FloatPrecision),
    /// Santorini.
    Santorini(FloatPrecision),
}

impl FaceRecoveryVariant {
    /// The precision this variant was asked for, widened to the common currency artifact names are composed from.
    pub fn precision(self) -> Precision {
        match self {
            Self::Athens(precision) | Self::Santorini(precision) => precision.into(),
        }
    }

    /// The execution-provider tuning measured for this variant.
    pub(crate) fn profile(self) -> EpProfile {
        // A match reaching the model's own file, which is where `models`' tiers put a measured profile and the prose
        // behind it: the measurement is of a graph, and the graph is the model's. Detection's sits in its family's
        // `variant.rs` only because that family has one model.
        //
        // The two rows disagree on the compute units and on the NHWC layout, and both disagreements are measured
        // rather than accidental. Carrying either model's answer over to the other on the strength of the two being
        // the same kind of model is how a profile ends up pessimising a graph it was never measured against — the
        // difference is in the exports, which is stated at each of the two arms.
        match self {
            Self::Athens(precision) => athens::profile(precision.into()),
            Self::Santorini(precision) => santorini::profile(precision.into()),
        }
    }

    /// The weight this variant binds to its graph, given the fidelity the operation `carried`: Athens binds what it
    /// carries, or [`Fidelity::MAX`] where it carries none, and Santorini binds nothing.
    ///
    /// [`restore`](super::restore) matches on what this answers: a value takes the weighted session call, and `None`
    /// takes the single-input one.
    pub(crate) fn weight(self, carried: Option<Fidelity>) -> Option<f32> {
        // The reference's `facerecovery.Variant.Fidelity` — `1.0` on Athens' row, `-1` on Santorini's — put where the
        // reference puts it and without the sentinel.
        //
        // **It cannot be read off `FaceRecoveryParams::fidelity` alone**, which is the whole reason it is asked of the
        // variant. That field is `None` for two unrelated reasons: Santorini has nowhere to bind a weight, and a
        // *deserialized* Athens operation can reach a pipeline with nothing in it, because the field is
        // `#[serde(default)]`. Choosing the call from the option would send such an Athens operation down the
        // single-input path, where the runtime refuses it for a missing required input — a stored selection failing
        // after an unrelated change, for a reason the error does not name. The variant knows which graph it is; the
        // operation only knows what it was given.
        //
        // Athens carrying none restores at `Fidelity::MAX`, the value the reference hard-codes for every one of its
        // runs. The alternatives are refusing it — a new public refusal for a state a caller cannot see the cause of —
        // and binding zero, which is the *most* aggressive restoration and the worst possible default for a value that
        // went missing.
        match self {
            Self::Athens(_) => Some(carried.map_or(Fidelity::MAX, Fidelity::get) as f32),
            Self::Santorini(_) => None,
        }
    }

    /// The variant this family publishes under `codename` at `precision`, or `None` where it publishes none.
    ///
    /// The inverse of the codename each variant reports: a front end that filled a chooser from [`fn@crate::catalogue`]
    /// hands the codename and the precision it was given straight back and gets the typed variant, restating none of
    /// this vocabulary in code of its own.
    ///
    /// A precision outside this contract answers `None` rather than being narrowed to something published — an
    /// INT8 row would name `fr_athens_int8`, which does not exist.
    pub fn from_codename(codename: &str, precision: Precision) -> Option<Self> {
        let precision = FloatPrecision::narrow(precision)?;

        [Self::Athens(precision), Self::Santorini(precision)]
            .into_iter()
            .find(|variant| variant.codename() == codename)
    }

    /// The row backing this variant.
    pub(crate) const fn row(self) -> SimpleVariant {
        // An exhaustive match rather than a lookup, so a variant added later cannot compile without one.
        match self {
            Self::Athens(_) => ATHENS,
            Self::Santorini(_) => SANTORINI,
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
    pub(crate) fn every_variant() -> Vec<FaceRecoveryVariant> {
        let mut variants = Vec::new();

        for precision in FloatPrecision::ALL {
            variants.extend([FaceRecoveryVariant::Athens(precision), FaceRecoveryVariant::Santorini(precision)]);
        }

        variants
    }

    #[test]
    fn every_variant_reports_its_label_codename_and_precision() {
        assert_eq!(FaceRecoveryVariant::Athens(FloatPrecision::Fp32).label(), "Athens");
        assert_eq!(FaceRecoveryVariant::Santorini(FloatPrecision::Fp32).label(), "Santorini");

        for variant in every_variant() {
            assert_eq!(variant.codename(), variant.label().to_lowercase(), "{variant:?}");
            assert!(matches!(variant.precision(), Precision::Fp32 | Precision::Fp16), "{variant:?}");
        }
    }

    #[test]
    fn the_family_publishes_two_variants_at_both_float_precisions_and_at_no_other() {
        assert_eq!(ALL.len(), 2);
        assert_eq!(every_variant().len(), 4);

        for variant in every_variant() {
            assert_ne!(variant.precision(), Precision::Int8, "{variant:?} reached a precision it is not published in");
        }
    }

    #[test]
    fn every_published_row_is_reachable_from_its_codename() {
        // Driven from `ALL` — the rows the catalogue publishes — rather than from a list written here, so a variant
        // added to the family and left out of `from_codename` fails this rather than going unnoticed.
        for row in ALL {
            for precision in FloatPrecision::ALL {
                let variant = FaceRecoveryVariant::from_codename(row.codename, precision.into())
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
                FaceRecoveryVariant::from_codename(variant.codename(), variant.precision()),
                Some(variant),
                "{variant:?} did not round-trip through the codename and precision it reports"
            );
        }
    }

    #[test]
    fn athens_binds_the_fidelity_it_carries_and_the_maximum_where_it_carries_none() {
        // The trap this lives on the row to avoid, as `weight` records: a deserialized Athens operation carries `None`
        // for a different reason than Santorini does.
        for precision in FloatPrecision::ALL {
            let athens = FaceRecoveryVariant::Athens(precision);

            for value in [Fidelity::MIN, 0.25, 0.5, Fidelity::MAX] {
                let carried = Fidelity::new(value).expect("the test supplied a fidelity in range");

                assert_eq!(
                    athens.weight(Some(carried)),
                    Some(value as f32),
                    "Athens at {precision:?} did not bind the fidelity it was given"
                );
            }

            assert_eq!(
                athens.weight(None),
                Some(Fidelity::MAX as f32),
                "Athens at {precision:?} carrying no fidelity did not restore at the maximum"
            );
        }
    }

    #[test]
    fn santorini_binds_nothing_at_either_precision() {
        // Not a default in place of the value it does not take: its graph has one input, and a weight sent anyway is
        // refused by the runtime rather than quietly ignored.
        for precision in FloatPrecision::ALL {
            let santorini = FaceRecoveryVariant::Santorini(precision);

            assert_eq!(santorini.weight(None), None, "Santorini at {precision:?} bound a weight");

            let carried = Fidelity::new(0.5).expect("0.5 is in range");
            assert_eq!(
                santorini.weight(Some(carried)),
                None,
                "Santorini at {precision:?} bound a weight it has nowhere to put"
            );
        }
    }

    #[test]
    fn a_pairing_this_family_does_not_publish_names_nothing() {
        assert_eq!(
            FaceRecoveryVariant::from_codename("newyork", Precision::Fp32),
            None,
            "a detection codename named a face recovery variant"
        );
        assert_eq!(FaceRecoveryVariant::from_codename("", Precision::Fp32), None);
        assert_eq!(
            FaceRecoveryVariant::from_codename(&ALL[0].codename.to_uppercase(), Precision::Fp32),
            None,
            "a codename was matched by something other than the spelling that is published"
        );

        for row in ALL {
            assert_eq!(
                FaceRecoveryVariant::from_codename(row.codename, Precision::Int8),
                None,
                "{} named an INT8 build the family does not publish",
                row.codename
            );
        }
    }
}
