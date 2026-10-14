//! The flags, or what autopilot chose, turned into the chain the library runs.
//!
//! The terminal's own vocabulary — `hd`, `sd`, `--up`, `--dn-str` and the rest — mapped onto the rows of
//! the library's catalogue, the same way the GUI and perf build theirs. The scale ladder is the library's
//! [`opai::suggested_scale`], and a model's builds and parameter defaults are the ones its catalogue row publishes,
//! none restated.

use opai::{Bias, Operation, ParameterKind, ParameterValues, Strength, Subject, Suggestion, VariantEntry};

use crate::args::{Quality, Selection};
use crate::autopilot::{AutopilotModels, FamilyModel};

/// What a run will do, and what the user should be told about it before it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The chain, in [`Family::APPLY_ORDER`]. The library puts it into that order anyway, so the flags' order cannot
    /// matter.
    pub operations: Vec<Operation>,
    /// Lines to show the user about what was planned, such as an upscale left out.
    pub notes: Vec<String>,
}

impl Selection {
    /// What autopilot chose for one photograph: each family in `suggestions` with its model from `models`, at the
    /// catalogue's default parameters, and an upscale at the scale the analysis read off the photograph.
    pub fn from_suggestions(suggestions: &[Suggestion], models: &AutopilotModels) -> Self {
        let mut selection = Self::default();

        for suggestion in suggestions {
            match *suggestion {
                Suggestion::Denoise => selection.denoise = models.model_for(),
                Suggestion::FaceRecovery => selection.face_recovery = models.model_for(),
                Suggestion::Colorization => selection.colorization = models.model_for(),
                Suggestion::LightAdjustment => selection.light_adjustment = models.model_for(),
                Suggestion::ColorBalance => selection.color_balance = models.model_for(),
                Suggestion::Sharpen => selection.sharpen = models.model_for(),
                Suggestion::Upscale { scale } => {
                    selection.upscale = models.model_for();
                    selection.scale = Some(scale);
                }
            }
        }

        selection
    }
}

/// The chain `selection` asks for at `quality` over a `width` x `height` photograph.
///
/// An upscale with no scale runs at the factor the photograph needs, and is left out with a note where it needs
/// none. Every face recovery restores nobody until the faces are found and handed to it.
pub fn plan(selection: &Selection, quality: Quality, width: u32, height: u32) -> Plan {
    let mut operations = Vec::new();
    let mut notes = Vec::new();

    if let Some(model) = selection.denoise {
        operations.push(build(model, quality, |values| set(values, Strength::NAME, selection.denoise_strength)));
    }

    if let Some(model) = selection.face_recovery {
        // Maximum fidelity: as close to the original face as the model goes, which is what the catalogue publishes.
        operations.push(build(model, quality, |_| {}));
    }

    if let Some(model) = selection.colorization {
        operations.push(build(model, quality, |_| {}));
    }

    if let Some(model) = selection.light_adjustment {
        operations.push(build(model, quality, |values| set(values, Bias::NAME, selection.light_bias)));
    }

    if let Some(model) = selection.color_balance {
        operations.push(build(model, quality, |values| set(values, Bias::NAME, selection.balance_bias)));
    }

    if let Some(model) = selection.sharpen {
        operations.push(build(model, quality, |values| set(values, Strength::NAME, selection.sharpen_strength)));
    }

    if let Some(model) = selection.upscale {
        match selection.scale.or_else(|| opai::suggested_scale(width, height)) {
            Some(scale) => {
                operations.push(build(model, quality, |values| *values = std::mem::take(values).with_scale(scale)));
            }
            None => notes.push(format!(
                "the upscale was skipped: the photograph is already large enough ({width}x{height}); \
                 pass --scale to upscale it anyway"
            )),
        }
    }

    Plan { operations, notes }
}

/// The operation `model` names at `quality`, built by the library's catalogue: at the parameters that model's row
/// publishes as defaults, then whatever `overrides` sets.
///
/// `quality` picks a build by its position in the row's precisions, so a model published at other precisions than
/// FP32 and FP16 — Osaka, at FP16 and INT8 — needs no case of its own.
fn build<M: FamilyModel>(model: M, quality: Quality, overrides: impl FnOnce(&mut ParameterValues)) -> Operation {
    let name = model.to_possible_value().expect("no model is skipped");
    let variant =
        VariantEntry::published(M::FAMILY, name.get_name()).expect("every model the command line names is published");
    let precision = variant.precisions.get(quality.position()).unwrap_or(&variant.precisions[0]);

    let mut values = ParameterValues::new();
    for parameter in variant.parameters {
        if let ParameterKind::Range { default, .. } = parameter.kind {
            values.set(parameter.name, default).expect("a published parameter is known by its name");
        }
    }
    overrides(&mut values);

    match variant.build(*precision, &values) {
        Ok(Subject::Enhancement(operation)) => operation,
        other => panic!("{:?} {} did not build an enhancement: {other:?}", M::FAMILY, variant.codename),
    }
}

/// Sets `name` to `percent` of the model's own effect, where one was given.
fn set<T: Into<f64>>(values: &mut ParameterValues, name: &str, percent: Option<T>) {
    if let Some(percent) = percent {
        // Every whole percentage the parser accepts is a whole number of the library's 0.001 steps, inside its range.
        values.set(name, percent.into() / 100.0).expect("a published parameter is known by its name");
    }
}

#[cfg(test)]
mod tests {
    use opai::{
        ColorBalance, Colorization, Denoise, FaceRecovery, Faces, Family, Fidelity, FloatPrecision, LightAdjustment,
        OsakaPrecision, Scale, Sharpen, Upscale,
    };

    use super::*;
    use crate::args::{Args, parse_from};

    fn args(line: &str) -> Args {
        parse_from(line.split_whitespace()).unwrap_or_else(|error| panic!("{line}: {error}"))
    }

    /// The chain the command line `line` asks for over a `width` x `height` photograph.
    fn plan(line: &str, width: u32, height: u32) -> Plan {
        let args = args(line);
        super::plan(&args.selection, args.quality, width, height)
    }

    fn scale(factor: f64) -> Scale {
        Scale::new(factor).expect("in range")
    }

    fn strength(value: f64) -> Strength {
        Strength::new(value).expect("in range")
    }

    fn bias(value: f64) -> Bias {
        Bias::new(value).expect("in range")
    }

    #[test]
    fn every_row_of_the_quality_table() {
        let four = scale(4.0);

        let rows = [
            ("--up kyoto -q hd", Upscale::kyoto(FloatPrecision::Fp32, four)),
            ("--up kyoto -q sd", Upscale::kyoto(FloatPrecision::Fp16, four)),
            ("--up tokyo -q hd", Upscale::tokyo(FloatPrecision::Fp32, four)),
            ("--up tokyo -q sd", Upscale::tokyo(FloatPrecision::Fp16, four)),
            ("--up saitama -q hd", Upscale::saitama(FloatPrecision::Fp32, four)),
            ("--up saitama -q sd", Upscale::saitama(FloatPrecision::Fp16, four)),
            ("--up osaka -q hd", Upscale::osaka(OsakaPrecision::Fp16, four)),
            ("--up osaka -q sd", Upscale::osaka(OsakaPrecision::Int8, four)),
            (
                "--fr athens -q hd",
                FaceRecovery::athens(FloatPrecision::Fp32, Faces::empty(), Fidelity::MAXIMUM),
            ),
            (
                "--fr athens -q sd",
                FaceRecovery::athens(FloatPrecision::Fp16, Faces::empty(), Fidelity::MAXIMUM),
            ),
            ("--fr santorini -q hd", FaceRecovery::santorini(FloatPrecision::Fp32, Faces::empty())),
            ("--fr santorini -q sd", FaceRecovery::santorini(FloatPrecision::Fp16, Faces::empty())),
        ];
        let one = strength(1.0);
        let half = bias(0.5);
        let rows = rows.into_iter().chain([
            ("--dn stockholm -q hd", Denoise::stockholm(FloatPrecision::Fp32, one)),
            ("--dn stockholm -q sd", Denoise::stockholm(FloatPrecision::Fp16, one)),
            ("--dn gothenburg -q hd", Denoise::gothenburg(FloatPrecision::Fp32, one)),
            ("--dn gothenburg -q sd", Denoise::gothenburg(FloatPrecision::Fp16, one)),
            ("--dn malmo -q hd", Denoise::malmo(FloatPrecision::Fp32, one)),
            ("--dn malmo -q sd", Denoise::malmo(FloatPrecision::Fp16, one)),
            ("--sh moscow -q hd", Sharpen::moscow(FloatPrecision::Fp32, one)),
            ("--sh moscow -q sd", Sharpen::moscow(FloatPrecision::Fp16, one)),
            ("--sh petersburg -q hd", Sharpen::stpetersburg(FloatPrecision::Fp32, one)),
            ("--sh petersburg -q sd", Sharpen::stpetersburg(FloatPrecision::Fp16, one)),
            ("--sh novgorod -q hd", Sharpen::novgorod(FloatPrecision::Fp32, one)),
            ("--sh novgorod -q sd", Sharpen::novgorod(FloatPrecision::Fp16, one)),
            ("--cl delhi -q hd", Colorization::delhi(FloatPrecision::Fp32)),
            ("--cl delhi -q sd", Colorization::delhi(FloatPrecision::Fp16)),
            ("--cl mumbai -q hd", Colorization::mumbai(FloatPrecision::Fp32)),
            ("--cl mumbai -q sd", Colorization::mumbai(FloatPrecision::Fp16)),
            ("--cl jaipur -q hd", Colorization::jaipur(FloatPrecision::Fp32)),
            ("--cl jaipur -q sd", Colorization::jaipur(FloatPrecision::Fp16)),
            ("--la paris -q hd", LightAdjustment::paris(FloatPrecision::Fp32, half)),
            ("--la paris -q sd", LightAdjustment::paris(FloatPrecision::Fp16, half)),
            ("--la lyon -q hd", LightAdjustment::lyon(FloatPrecision::Fp32, half)),
            ("--la lyon -q sd", LightAdjustment::lyon(FloatPrecision::Fp16, half)),
            ("--cb rio -q hd", ColorBalance::rio(FloatPrecision::Fp32, half)),
            ("--cb rio -q sd", ColorBalance::rio(FloatPrecision::Fp16, half)),
            ("--cb saopaulo -q hd", ColorBalance::saopaulo(FloatPrecision::Fp32, half)),
            ("--cb saopaulo -q sd", ColorBalance::saopaulo(FloatPrecision::Fp16, half)),
        ]);

        for (flags, expected) in rows {
            let plan = plan(&format!("opai photo.jpg {flags} -o out.png"), 1000, 800);
            assert_eq!(plan.operations, vec![expected], "{flags}");
        }
    }

    #[test]
    fn the_default_quality_runs_tokyo_at_fp32() {
        let plan = plan("opai photo.jpg --up -o out.png", 1000, 800);

        assert_eq!(plan.operations, vec![Upscale::tokyo(FloatPrecision::Fp32, scale(4.0))]);
    }

    #[test]
    fn the_scale_is_what_the_photograph_needs() {
        let up = "opai photo.jpg --up -o out.png";

        let small = plan(up, 1000, 800);
        assert_eq!(small.operations, vec![Upscale::tokyo(FloatPrecision::Fp32, scale(4.0))]);
        assert!(small.notes.is_empty());

        let medium = plan(up, 2000, 1500);
        assert_eq!(medium.operations, vec![Upscale::tokyo(FloatPrecision::Fp32, scale(2.0))]);
        assert!(medium.notes.is_empty());

        let large = plan(up, 4000, 3000);
        assert!(large.operations.is_empty(), "a 12 MP photograph was upscaled: {:?}", large.operations);
        assert_eq!(large.notes.len(), 1);
        assert!(large.notes[0].contains("already large enough"), "{:?}", large.notes);
    }

    #[test]
    fn a_skipped_upscale_leaves_the_face_recovery() {
        let plan = plan("opai photo.jpg --up --fr -o out.png", 4000, 3000);

        assert_eq!(
            plan.operations,
            vec![FaceRecovery::athens(FloatPrecision::Fp32, Faces::empty(), Fidelity::MAXIMUM)]
        );
        assert_eq!(plan.notes.len(), 1);
    }

    #[test]
    fn an_explicit_scale_is_always_honoured() {
        let plan = plan("opai photo.jpg --up --scale 2 -o out.png", 4000, 3000);

        assert_eq!(plan.operations, vec![Upscale::tokyo(FloatPrecision::Fp32, scale(2.0))]);
        assert!(plan.notes.is_empty());
    }

    #[test]
    fn each_parameter_is_a_percentage_of_its_own_enhancement() {
        let plan = plan("opai photo.jpg --dn --dn-str 200 --sh --sh-str 80 --la --la-bias -20 --cb -o x", 1, 1);

        assert_eq!(
            plan.operations,
            vec![
                Denoise::stockholm(FloatPrecision::Fp32, strength(2.0)),
                LightAdjustment::paris(FloatPrecision::Fp32, bias(-0.2)),
                ColorBalance::rio(FloatPrecision::Fp32, bias(0.5)),
                Sharpen::moscow(FloatPrecision::Fp32, strength(0.8)),
            ]
        );
    }

    #[test]
    fn every_accepted_percentage_builds() {
        for percent in 1..=300 {
            let line = format!("opai photo.jpg --dn --dn-str {percent} -o out.png");
            assert_eq!(plan(&line, 1, 1).operations.len(), 1, "{line}");
        }
        for percent in -100..=100 {
            let line = format!("opai photo.jpg --la --la-bias {percent} -o out.png");
            assert_eq!(plan(&line, 1, 1).operations.len(), 1, "{line}");
        }
    }

    #[test]
    fn the_flags_order_does_not_change_the_chain() {
        let up_first = plan("opai photo.jpg --up saitama --fr santorini -q sd -o out.png", 1000, 800);
        let fr_first = plan("opai photo.jpg --fr santorini --up saitama -q sd -o out.png", 1000, 800);

        assert_eq!(up_first, fr_first);
        assert_eq!(up_first.operations.len(), 2);

        let up_first = plan("opai photo.jpg --up --sh --dn -o out.png", 1000, 800);
        let dn_first = plan("opai photo.jpg --dn --sh --up -o out.png", 1000, 800);

        assert_eq!(up_first, dn_first);
        assert_eq!(
            up_first.operations,
            vec![
                Denoise::stockholm(FloatPrecision::Fp32, strength(1.0)),
                Sharpen::moscow(FloatPrecision::Fp32, strength(1.0)),
                Upscale::tokyo(FloatPrecision::Fp32, scale(4.0)),
            ]
        );
    }

    #[test]
    fn the_chain_is_pushed_in_the_librarys_apply_order() {
        let plan = plan("opai photo.jpg --up --sh --cb --la --cl --fr --dn -o out.png", 1000, 800);
        let families: Vec<_> = plan.operations.iter().map(Operation::family).collect();

        assert_eq!(families, Family::APPLY_ORDER);
    }

    /// The chain autopilot plans for `suggested` with `models` at `quality` over a `width` x `height` photograph.
    fn autopilot(
        suggested: &[Suggestion],
        models: &AutopilotModels,
        quality: Quality,
        width: u32,
        height: u32,
    ) -> Plan {
        super::plan(&Selection::from_suggestions(suggested, models), quality, width, height)
    }

    #[test]
    fn each_suggestion_with_the_default_models_plans_what_its_bare_flag_does() {
        let defaults = AutopilotModels::default();
        let cases = [
            (Suggestion::Denoise, "--dn"),
            (Suggestion::FaceRecovery, "--fr"),
            (Suggestion::Colorization, "--cl"),
            (Suggestion::LightAdjustment, "--la"),
            (Suggestion::ColorBalance, "--cb"),
            (Suggestion::Sharpen, "--sh"),
            (Suggestion::Upscale { scale: scale(4.0) }, "--up"),
        ];

        for quality in ["hd", "sd"] {
            let parsed = args(&format!("opai photo.jpg --up -q {quality} -o out.png")).quality;
            for (suggestion, flag) in cases {
                let expected = plan(&format!("opai photo.jpg {flag} -q {quality} -o out.png"), 1000, 800);

                assert_eq!(autopilot(&[suggestion], &defaults, parsed, 1000, 800), expected, "{flag} -q {quality}");
            }
        }
    }

    #[test]
    fn a_model_chosen_through_the_environment_runs_at_the_quality_asked_for() {
        let models = AutopilotModels::read(|name| (name == "OPAI_AP_UPSCALE").then(|| "osaka".into())).expect("reads");

        let plan = autopilot(&[Suggestion::Upscale { scale: scale(4.0) }], &models, Quality::Sd, 1000, 800);

        assert_eq!(plan.operations, vec![Upscale::osaka(OsakaPrecision::Int8, scale(4.0))]);
    }

    #[test]
    fn the_suggested_scale_is_used_whatever_the_photographs_size() {
        let suggested = [Suggestion::Upscale { scale: scale(2.0) }];

        let plan = autopilot(&suggested, &AutopilotModels::default(), Quality::Hd, 4000, 3000);

        assert_eq!(plan.operations, vec![Upscale::tokyo(FloatPrecision::Fp32, scale(2.0))]);
        assert!(plan.notes.is_empty(), "{:?}", plan.notes);
    }

    #[test]
    fn several_suggestions_plan_in_the_librarys_apply_order() {
        let suggested = [Suggestion::Denoise, Suggestion::ColorBalance, Suggestion::Upscale { scale: scale(4.0) }];

        let plan = autopilot(&suggested, &AutopilotModels::default(), Quality::Hd, 1000, 800);

        assert_eq!(
            plan.operations,
            vec![
                Denoise::stockholm(FloatPrecision::Fp32, strength(1.0)),
                ColorBalance::rio(FloatPrecision::Fp32, bias(0.5)),
                Upscale::tokyo(FloatPrecision::Fp32, scale(4.0)),
            ]
        );
    }

    #[test]
    fn no_suggestion_plans_an_empty_chain() {
        let plan = autopilot(&[], &AutopilotModels::default(), Quality::Hd, 1000, 800);

        assert_eq!(plan, Plan { operations: Vec::new(), notes: Vec::new() });
    }
}
