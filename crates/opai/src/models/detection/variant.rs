//! What can be asked for in the detection family: the one row, and the enum that names it at a precision.

use serde::{Deserialize, Serialize};

use super::super::precision::{FloatPrecision, Precision};
use super::super::simple::SimpleVariant;

use crate::providers::profile::{EpProfile, ExecutionMode};

// The codename and label are not one another's case: the codename is what an artifact name, a download URL and a
// cache directory are composed from, so it carries no space. Both are transcribed — the reference's
// `models/ids_test.go` pins `dt_newyork_fp32`, and `detection/newyork/newyork.go` pins the label.
/// New York, the RetinaFace detector and the only face-detection model published.
pub(crate) const NEW_YORK: SimpleVariant = SimpleVariant { codename: "newyork", label: "New York", parameters: &[] };

/// Every detection row, in the order a chooser should offer them. Read by the catalogue.
pub(crate) const ALL: [SimpleVariant; 1] = [NEW_YORK];

// The precision travels inside the variant rather than beside it as one shared enum plus a check.
/// Which detection model an operation runs, and at which precision.
///
/// The precision travels **inside** the variant, in the domain this model is published in. New York ships FP32 and
/// FP16 and nothing else, so `dt_newyork_int8` is not a request this refuses — it does not compile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "codename", content = "precision", rename_all = "lowercase")]
pub enum DetectionVariant {
    /// New York.
    NewYork(FloatPrecision),
}

impl DetectionVariant {
    /// The precision this variant was asked for, widened to the common currency artifact names are composed from.
    pub fn precision(self) -> Precision {
        match self {
            Self::NewYork(precision) => precision.into(),
        }
    }

    /// The execution-provider tuning measured for this variant.
    pub(crate) fn profile(self) -> EpProfile {
        // New York: NHWC on CUDA and sequential execution, at **both** precisions, and every figure the reference's.
        //
        // The two are independent and compose. NHWC removes a transpose pair around every convolution — RetinaFace is
        // a ResNet-34 backbone, a feature pyramid and three convolutional heads, dense convolution end to end — and is
        // worth -13.9% at FP32 on the graph on its own. Sequential execution drops the inter-op handoff that a
        // 176-node backbone has no branch wide enough to pay for, so the parallel mode can only charge one per node.
        // Together they are **-17.5% at FP32 and -35.1% at FP16** on the graph, and -11.6% and -23.4% end to end on
        // an RTX 5090. The output is unaffected, checked across twenty consecutive runs on one session — the check
        // that matters here, because the failure this graph has shown is a session that goes wrong only from its
        // second run onwards.
        //
        // **That the layout is declared at FP32 as well as FP16 is the load-bearing part.** Athens declares it for
        // FP16 **only**, because at FP32 it measured +8% there; `EpProfile::cuda_prefer_nhwc` says why the layout is
        // measured per graph at each precision. A reader tidying this for symmetry with Athens would remove exactly
        // the half worth 17.5%, and nothing would report it: the model still loads and still returns the same faces,
        // only slower.
        //
        // CoreML stays on the defaults, and that is a measurement: every CoreML setting this system can express was
        // measured against this graph and none of them won. The expensive one is restricting CoreML to the CPU and
        // GPU — the setting most likely to be copied over from Athens for symmetry — which costs the FP16 graph 20%,
        // because dense convolution is exactly what the Neural Engine is built for. Asking for fast prediction
        // measured as a tie. Sequential execution is a session setting rather than a per-provider one, so it reaches
        // CoreML too, where it is worth -6.5% at FP16 and a tie at FP32.
        //
        // Two CUDA defaults must not change for this graph, and the `..EpProfile::default()` below carries them.
        // Disabling TF32 costs 114% at FP32 and 29% at FP16. Convolution-bias fusion is broken here: at FP32 it
        // decodes zero faces from intact confidence scores, and at FP16 it fails on the first convolution.
        //
        // What made this model fast is not a provider setting but the export: see `newyork::TARGET_SIZE`.
        match self {
            Self::NewYork(_) => {
                EpProfile { cuda_prefer_nhwc: true, execution_mode: ExecutionMode::Sequential, ..EpProfile::default() }
            }
        }
    }

    /// The variant this family publishes under `codename` at `precision`, or `None` where it publishes none.
    ///
    /// The inverse of the codename each variant reports: a front end that filled a chooser from [`fn@crate::catalogue`]
    /// hands the codename and the precision it was given straight back and gets the typed variant, restating none of
    /// this vocabulary in code of its own.
    ///
    /// A precision outside this contract answers `None` rather than being narrowed to something published — an
    /// INT8 row would name `dt_newyork_int8`, which does not exist.
    pub fn from_codename(codename: &str, precision: Precision) -> Option<Self> {
        let precision = FloatPrecision::narrow(precision)?;

        [Self::NewYork(precision)].into_iter().find(|variant| variant.codename() == codename)
    }

    /// The row backing this variant.
    pub(crate) const fn row(self) -> SimpleVariant {
        // An exhaustive match rather than a lookup, so a variant added later cannot compile without one.
        match self {
            Self::NewYork(_) => NEW_YORK,
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
    pub(crate) fn every_variant() -> Vec<DetectionVariant> {
        FloatPrecision::ALL.iter().map(|precision| DetectionVariant::NewYork(*precision)).collect()
    }

    #[test]
    fn every_variant_reports_its_label_codename_and_precision() {
        assert_eq!(DetectionVariant::NewYork(FloatPrecision::Fp32).label(), "New York");
        assert_eq!(DetectionVariant::NewYork(FloatPrecision::Fp32).codename(), "newyork");

        for variant in every_variant() {
            assert!(
                variant.codename().chars().all(|c| c.is_ascii_lowercase()),
                "{variant:?} is not spelled the way an artifact name, a URL and a directory name all accept"
            );
            assert!(matches!(variant.precision(), Precision::Fp32 | Precision::Fp16), "{variant:?}");
        }
    }

    #[test]
    fn the_family_publishes_exactly_one_variant_at_both_float_precisions() {
        assert_eq!(ALL.len(), 1);
        assert_eq!(
            every_variant().iter().map(|variant| variant.precision()).collect::<Vec<_>>(),
            vec![Precision::Fp32, Precision::Fp16]
        );
    }

    #[test]
    fn every_published_row_is_reachable_from_its_codename() {
        // Driven from `ALL` — the rows the catalogue publishes — rather than from a list written here, so a variant
        // added to the family and left out of `from_codename` fails this rather than going unnoticed.
        for row in ALL {
            for precision in FloatPrecision::ALL {
                let variant = DetectionVariant::from_codename(row.codename, precision.into())
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
                DetectionVariant::from_codename(variant.codename(), variant.precision()),
                Some(variant),
                "{variant:?} did not round-trip through the codename and precision it reports"
            );
        }
    }

    #[test]
    fn a_pairing_this_family_does_not_publish_names_nothing() {
        assert_eq!(
            DetectionVariant::from_codename("athens", Precision::Fp32),
            None,
            "a face recovery codename named a detection variant"
        );
        assert_eq!(DetectionVariant::from_codename("", Precision::Fp32), None);
        assert_eq!(
            DetectionVariant::from_codename(&ALL[0].codename.to_uppercase(), Precision::Fp32),
            None,
            "a codename was matched by something other than the spelling that is published"
        );

        for row in ALL {
            assert_eq!(
                DetectionVariant::from_codename(row.codename, Precision::Int8),
                None,
                "{} named an INT8 build the family does not publish",
                row.codename
            );
        }
    }
}
