//! What distinguishes one upscale variant from another, as data.

// A row per variant rather than a type per variant: everything that differs between Kyoto and Saitama is a codename, a
// display label and a table, and it is the exhaustiveness of `UpscaleVariant` that makes a new row mandatory rather
// than a registration step someone can forget.

use serde::{Deserialize, Serialize};

use super::super::catalogue::ParameterEntry;
use super::super::precision::{FloatPrecision, Precision};
use super::super::scale::Scale;
use super::MODELS;
use super::conv::PassShape;
use super::kyoto::KYOTO;
use super::osaka::OSAKA;
use super::osaka::precision::OsakaPrecision;
use super::resolve::Resolution;
use super::saitama::SAITAMA;
use super::tokyo::TOKYO;

use crate::providers::profile::EpProfile;

// Object-safe on purpose: every method answers with **data** — a name, a precision list, a profile value, a
// `Resolution` — and none of them is generic over a backend, holds a session or returns a pipeline.
// That is what lets the four rows be `static`s reached as `&'static dyn`, with no `Arc`, no reference count and no
// lifetime shorter than the process.
//
// Once per contract rather than once per model: every difference between Tokyo, Kyoto and Saitama is data in their
// rows, and every difference between those three and Osaka is the contract — so a unit struct per model would be four
// near-identical bodies delegating to one family-tier function, which is the copying `crate::models`' tier rule warns
// against.
/// What the family asks of a model, and the only thing [`UpscaleVariant::model`] hands back.
///
/// Implemented **once per contract**: [`ConvVariant`](super::conv::ConvVariant) in [`conv`](super::conv) and [`DiffusionVariant`](super::osaka::graph::DiffusionVariant) in
/// [`graph`](super::osaka::graph).
pub(crate) trait UpscaleModel: Send + Sync {
    /// The developer-facing identifier every artifact name for this model is composed from.
    fn codename(&self) -> &'static str;

    /// The display text a user sees, with no precision or scale appended.
    fn label(&self) -> &'static str;

    // Asked of the model rather than decided beside it, which is what keeps a chooser from offering a build that was
    // never published the moment a second model of either contract publishes a different pair.
    /// Every precision this model is published in, in the order a chooser should offer them.
    fn precisions(&self) -> &'static [Precision];

    // Asked of the model for the reason `precisions` is, and the reason `SimpleVariant` carries the same thing on its
    // row: what a model takes is a property of that model. A family-wide constant would give an upscale model taking
    // something other than a scale a silently wrong catalogue entry rather than a compile error.
    /// What a request for this model carries, as the catalogue publishes it.
    fn parameters(&self) -> &'static [ParameterEntry];

    /// The execution-provider tuning measured for this model at `precision`.
    fn profile(&self, precision: Precision) -> EpProfile;

    /// The tensor shape this model's passes run at, or `None` where this model runs no pass sequence.
    ///
    /// `None` for the diffusion contract, which runs graphs at a geometry its own module declares. The arm of
    /// [`Upscale::pipeline`](super::Upscale::pipeline) that builds a pass sequence relies on it being `Some` for every
    /// model that resolves to passes.
    fn pass_shape(&self) -> Option<PassShape>;

    /// The variant naming this row at `precision`, or `None` where this row publishes nothing at it.
    ///
    /// The inverse of [`UpscaleVariant::model`]. Each contract narrows `precision` to its own set here, which is the
    /// same fact [`precisions`](Self::precisions) reports: answering both from the row is what keeps them agreeing.
    fn variant(&self, precision: Precision) -> Option<UpscaleVariant>;

    /// What a request for `scale` at `precision` resolves to — the native passes covering it, or the graphs loaded
    /// together for one pass.
    fn resolve(&self, precision: Precision, scale: Scale) -> Resolution;
}

// The precision inside the variant rather than beside it as one shared enum plus a per-variant check.
/// Which upscale model an operation runs, and at which precision.
///
/// The precision travels **inside** the variant, in the domain that variant is actually published in. Osaka paired
/// with FP32 is then not a runtime error — it does not compile, and the artifact `up_osaka_fp32` has no path that
/// could name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "codename", content = "precision", rename_all = "lowercase")]
pub enum UpscaleVariant {
    /// Tokyo: convolutional, native 4x weights only.
    Tokyo(FloatPrecision),
    /// Kyoto: convolutional, native 2x and 4x weights.
    Kyoto(FloatPrecision),
    /// Saitama: convolutional, native 4x weights only.
    Saitama(FloatPrecision),
    /// Osaka: the diffusion upscaler, three graphs run as one pass.
    Osaka(OsakaPrecision),
}

impl UpscaleVariant {
    /// The precision this variant was asked for, widened to the common currency artifact names are composed from.
    pub fn precision(self) -> Precision {
        match self {
            Self::Tokyo(precision) | Self::Kyoto(precision) | Self::Saitama(precision) => precision.into(),
            Self::Osaka(precision) => precision.into(),
        }
    }

    /// The model this variant runs — **seam 1**, and one of the two matches a model is named in.
    pub(crate) fn model(self) -> &'static dyn UpscaleModel {
        // One arm per model, exhaustive with no wildcard: a variant added to the enum without a directory does not
        // compile. And everything a row answers is a required field of the row rather than an arm someone could
        // forget, so a model written without a profile does not compile either.
        match self {
            Self::Tokyo(_) => &TOKYO,
            Self::Kyoto(_) => &KYOTO,
            Self::Saitama(_) => &SAITAMA,
            Self::Osaka(_) => &OSAKA,
        }
    }

    /// The execution-provider tuning measured for this variant, at the precision it carries.
    pub(crate) fn profile(self) -> EpProfile {
        // Asked of the model, which is what names the graph a measurement was made against; the precision it was
        // measured at travels inside the variant and is widened on the way through.
        self.model().profile(self.precision())
    }

    /// The display text a user sees for this variant, with no precision or scale appended.
    pub(crate) fn label(self) -> &'static str {
        self.model().label()
    }

    /// The developer-facing identifier every artifact name for this variant is composed from.
    pub(crate) fn codename(self) -> &'static str {
        self.model().codename()
    }

    /// The variant this family publishes under `codename` at `precision`, or `None` where it publishes none.
    ///
    /// The inverse of the codename each variant reports: a front end that filled a chooser from [`fn@crate::catalogue`]
    /// hands the codename and the precision it was given straight back and gets the typed variant, restating none of
    /// this vocabulary in code of its own.
    ///
    /// A published model at a precision it is not published in names nothing: `osaka` at FP32 rather than
    /// `up_osaka_fp32`, and the three convolutional variants at INT8.
    pub fn from_codename(codename: &str, precision: Precision) -> Option<Self> {
        // Over `MODELS` — the same list `crate::catalogue` builds the forward direction from — so the two directions
        // cannot disagree about which models exist. A second, hand-written list of the arms, left out of date, would
        // produce a model that resolves and runs perfectly and cannot be built from the codename a chooser offers.
        MODELS
            .iter()
            .find(|model| model.codename() == codename)
            .and_then(|model| model.variant(precision))
    }

    /// Whether one set of weights serves every scale, which is what decides whether the display name carries the
    /// scale.
    pub(crate) const fn is_diffusion(self) -> bool {
        matches!(self, Self::Osaka(_))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::providers::profile::{CoreMlComputeUnits, CoreMlSpecialization, ExecutionMode};

    /// Every operation the tests here and in the sibling modules build, one per variant and contract.
    pub(crate) fn every_variant() -> Vec<UpscaleVariant> {
        // Derived from `MODELS` crossed with each row's own published precisions, rather than listed by hand. A
        // model added to `MODELS` and forgotten here would leave every test driven off this list silently covering
        // one model fewer — including the ones that exist to catch exactly that kind of omission elsewhere.
        MODELS
            .iter()
            .flat_map(|model| model.precisions().iter().filter_map(|precision| model.variant(*precision)))
            .collect()
    }

    #[test]
    fn every_variant_reports_its_label_and_codename() {
        assert_eq!(UpscaleVariant::Tokyo(FloatPrecision::Fp32).label(), "Tokyo");
        assert_eq!(UpscaleVariant::Kyoto(FloatPrecision::Fp32).label(), "Kyoto");
        assert_eq!(UpscaleVariant::Saitama(FloatPrecision::Fp32).label(), "Saitama");
        assert_eq!(UpscaleVariant::Osaka(OsakaPrecision::Fp16).label(), "Osaka");

        for variant in every_variant() {
            assert_eq!(variant.codename(), variant.label().to_lowercase(), "{variant:?}");
        }
    }

    #[test]
    fn every_variant_is_backed_by_exactly_one_contracts_row() {
        for variant in every_variant() {
            // Restated over the seam the two rows were reached through: `model()` is exhaustive with no wildcard, so
            // every variant reaches exactly one row. Asked of what the row actually *resolves to* rather than of a
            // second declared flag — the resolution is the contract, so this cannot agree with the enum by being a
            // restatement of it.
            let resolved = variant.model().resolve(variant.precision(), Scale::new(2.0).unwrap());
            assert_eq!(
                matches!(resolved, Resolution::Graphs(_)),
                !matches!(variant, UpscaleVariant::Tokyo(_) | UpscaleVariant::Kyoto(_) | UpscaleVariant::Saitama(_)),
                "{variant:?} is backed by both contracts or by neither"
            );
        }
    }

    #[test]
    fn every_variant_reports_the_precision_it_was_built_with() {
        assert_eq!(UpscaleVariant::Kyoto(FloatPrecision::Fp32).precision(), Precision::Fp32);
        assert_eq!(UpscaleVariant::Tokyo(FloatPrecision::Fp16).precision(), Precision::Fp16);
        assert_eq!(UpscaleVariant::Saitama(FloatPrecision::Fp32).precision(), Precision::Fp32);
        assert_eq!(UpscaleVariant::Osaka(OsakaPrecision::Fp16).precision(), Precision::Fp16);
        assert_eq!(UpscaleVariant::Osaka(OsakaPrecision::Int8).precision(), Precision::Int8);
    }

    #[test]
    fn only_osaka_uses_the_diffusion_contract() {
        for variant in every_variant() {
            assert_eq!(variant.is_diffusion(), matches!(variant, UpscaleVariant::Osaka(_)), "{variant:?}");
        }
    }

    #[test]
    fn every_published_pairing_is_reachable_from_its_codename() {
        // Driven from `every_variant` — every pairing the family publishes, built from the two precision contracts
        // themselves — so a variant added to either contract and left out of `from_codename` fails here.
        for variant in every_variant() {
            assert_eq!(
                UpscaleVariant::from_codename(variant.codename(), variant.precision()),
                Some(variant),
                "{variant:?} did not round-trip through the codename and precision it reports"
            );
        }
    }

    #[test]
    fn a_pairing_this_family_does_not_publish_names_nothing() {
        // The pairing the whole precision split exists for: Osaka has no FP32 build, so the catalogue never offers
        // one and reading one back must name nothing rather than resolving to `up_osaka_fp32`.
        assert_eq!(
            UpscaleVariant::from_codename(OSAKA.codename, Precision::Fp32),
            None,
            "a catalogue row named the Osaka build that was never published"
        );

        for row in [TOKYO, KYOTO, SAITAMA] {
            assert_eq!(
                UpscaleVariant::from_codename(row.codename, Precision::Int8),
                None,
                "{} named an INT8 build the convolutional contract does not publish",
                row.codename
            );
        }

        assert_eq!(
            UpscaleVariant::from_codename("stockholm", Precision::Fp32),
            None,
            "a denoise codename named an upscale variant"
        );
        assert_eq!(UpscaleVariant::from_codename("", Precision::Fp32), None);
        assert_eq!(
            UpscaleVariant::from_codename(&TOKYO.codename.to_uppercase(), Precision::Fp32),
            None,
            "a codename was matched by something other than the spelling that is published"
        );
    }

    #[test]
    fn osaka_declares_every_measured_setting_at_both_precisions() {
        // Field by field and then whole, so "carries its arm's profile" cannot pass by both sides being the default —
        // and at both precisions, because nothing in this profile is a property of the export's types.
        for precision in OsakaPrecision::ALL {
            let osaka = UpscaleVariant::Osaka(precision).profile();

            assert_eq!(osaka.execution_mode, ExecutionMode::Sequential, "at {precision:?}");
            assert!(osaka.disable_mem_pattern, "at {precision:?}: the memory planner was left on");
            // Both names, spelled out rather than compared against `BROKEN_OPTIMIZERS`, so that dropping one of them
            // fails here instead of agreeing with itself. The order is the declaration's; the runtime reads them as a
            // set, but the value written is a list.
            assert_eq!(
                osaka.disabled_optimizers,
                ["ReshapeFusion", "SimplifiedLayerNormFusion"],
                "at {precision:?}: the transformers the reference names for this graph were not both declared"
            );
            assert_eq!(osaka.coreml_compute_units, CoreMlComputeUnits::CpuAndGpu, "at {precision:?}");
            assert!(osaka.cuda_prefer_nhwc, "at {precision:?}: the VAE halves' layout was not declared");
            assert_eq!(
                osaka.trt_options.get("trt_builder_optimization_level").map(String::as_str),
                Some("3"),
                "at {precision:?}"
            );

            // And nothing else. TensorRT's FP16 and INT8 modes are the two absences that are decisions rather than
            // omissions — see `osaka::profile` — so a profile that named either would fail here.
            assert!(!osaka.trt_options.contains_key("trt_fp16_enable"), "at {precision:?}: FP16 mode was declared");
            assert!(!osaka.trt_options.contains_key("trt_int8_enable"), "at {precision:?}: INT8 mode was declared");
            assert_eq!(osaka.trt_options.len(), 1, "at {precision:?}: an unmeasured TensorRT option was declared");
            assert_eq!(osaka.coreml_specialization, CoreMlSpecialization::default(), "at {precision:?}");
        }

        // The two arms are the same declaration rather than two that happen to agree today, which is what the
        // precision-independence above actually claims.
        assert_eq!(
            UpscaleVariant::Osaka(OsakaPrecision::Fp16).profile(),
            UpscaleVariant::Osaka(OsakaPrecision::Int8).profile()
        );
    }

    #[test]
    fn no_variant_beyond_the_four_measured_arms_declares_anything() {
        // Driven over every variant this family publishes, so a profile nothing measured cannot arrive by accident.
        let measured = [
            UpscaleVariant::Tokyo(FloatPrecision::Fp16),
            UpscaleVariant::Tokyo(FloatPrecision::Fp32),
            UpscaleVariant::Kyoto(FloatPrecision::Fp16),
            UpscaleVariant::Saitama(FloatPrecision::Fp16),
            UpscaleVariant::Osaka(OsakaPrecision::Fp16),
            UpscaleVariant::Osaka(OsakaPrecision::Int8),
        ];

        for variant in every_variant() {
            if measured.contains(&variant) {
                continue;
            }

            assert_eq!(variant.profile(), EpProfile::default(), "{variant:?} declared a profile nothing measured");
        }
    }
}
