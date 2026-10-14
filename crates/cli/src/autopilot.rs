//! Which model autopilot runs each family with: the bare flag's default, or what the family's `OPAI_AP_*` variable
//! names, or nothing at all where it says `off`.
//!
//! Read once, at startup and only under `--autopilot`, through a lookup the caller hands in, so the tests read a map
//! and nothing here touches the process environment. Only this front end reads these variables: the GUI answers the
//! same question from its own settings.

use std::ffi::OsString;

use clap::error::ErrorKind;
use clap::{CommandFactory, ValueEnum};
use opai::Family;

use crate::args::{
    Args, BalanceModel, ColorizationModel, DenoiseModel, FaceModel, LightModel, SharpenModel, UpscaleModel,
};

/// The word that leaves a family out of the analysis. No model is named `off`, so it is free.
const OFF: &str = "off";

/// A family's model vocabulary, as autopilot reads it from the environment.
pub trait FamilyModel: ValueEnum + Copy {
    /// The family these models belong to.
    const FAMILY: Family;
    /// The environment variable that chooses one.
    const VARIABLE: &'static str;
    /// The model the family's flag runs when given with no model, and autopilot's default.
    const DEFAULT: Self;

    /// This family's model in `models`, or `None` where it is switched off.
    fn choice(models: &AutopilotModels) -> Option<Self>;
}

macro_rules! family_model {
    ($model:ty, $family:expr, $variable:literal, $default:expr, $field:ident) => {
        impl FamilyModel for $model {
            const FAMILY: Family = $family;
            const VARIABLE: &'static str = $variable;
            const DEFAULT: Self = $default;

            fn choice(models: &AutopilotModels) -> Option<Self> {
                models.$field
            }
        }
    };
}

// The defaults are the bare flags' `default_missing_value`s, which clap does not expose for reading back;
// `every_default_is_the_bare_flags` holds the two together.
family_model!(DenoiseModel, Family::Denoise, "OPAI_AP_DENOISE", DenoiseModel::Stockholm, denoise);
family_model!(FaceModel, Family::FaceRecovery, "OPAI_AP_FACE", FaceModel::Athens, face_recovery);
family_model!(
    ColorizationModel,
    Family::Colorization,
    "OPAI_AP_COLORIZATION",
    ColorizationModel::Delhi,
    colorization
);
family_model!(LightModel, Family::LightAdjustment, "OPAI_AP_LIGHT", LightModel::Paris, light_adjustment);
family_model!(BalanceModel, Family::ColorBalance, "OPAI_AP_COLOR", BalanceModel::Rio, color_balance);
family_model!(SharpenModel, Family::Sharpen, "OPAI_AP_SHARPEN", SharpenModel::Moscow, sharpen);
family_model!(UpscaleModel, Family::Upscale, "OPAI_AP_UPSCALE", UpscaleModel::Tokyo, upscale);

/// Autopilot's model for every family, in the library's apply order: `None` where the family's variable says `off`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutopilotModels {
    pub denoise: Option<DenoiseModel>,
    pub face_recovery: Option<FaceModel>,
    pub colorization: Option<ColorizationModel>,
    pub light_adjustment: Option<LightModel>,
    pub color_balance: Option<BalanceModel>,
    pub sharpen: Option<SharpenModel>,
    pub upscale: Option<UpscaleModel>,
}

impl Default for AutopilotModels {
    fn default() -> Self {
        Self {
            denoise: Some(DenoiseModel::DEFAULT),
            face_recovery: Some(FaceModel::DEFAULT),
            colorization: Some(ColorizationModel::DEFAULT),
            light_adjustment: Some(LightModel::DEFAULT),
            color_balance: Some(BalanceModel::DEFAULT),
            sharpen: Some(SharpenModel::DEFAULT),
            upscale: Some(UpscaleModel::DEFAULT),
        }
    }
}

impl AutopilotModels {
    /// The seven `OPAI_AP_*` variables, each read through `lookup`.
    ///
    /// # Errors
    ///
    /// A usage error, as a bad flag value is one, for the first variable that is neither empty, `off`, nor a model of
    /// its family, naming the variable and the value and listing what it accepts.
    pub fn read(lookup: impl Fn(&str) -> Option<OsString>) -> Result<Self, clap::Error> {
        Ok(Self {
            denoise: read_one(&lookup)?,
            face_recovery: read_one(&lookup)?,
            colorization: read_one(&lookup)?,
            light_adjustment: read_one(&lookup)?,
            color_balance: read_one(&lookup)?,
            sharpen: read_one(&lookup)?,
            upscale: read_one(&lookup)?,
        })
    }

    /// The families not switched off, in the library's apply order: what the analysis is asked to look for.
    pub fn families(&self) -> Vec<Family> {
        fn on<M: FamilyModel>(models: &AutopilotModels) -> Option<Family> {
            M::choice(models).map(|_| M::FAMILY)
        }

        [
            on::<DenoiseModel>(self),
            on::<FaceModel>(self),
            on::<ColorizationModel>(self),
            on::<LightModel>(self),
            on::<BalanceModel>(self),
            on::<SharpenModel>(self),
            on::<UpscaleModel>(self),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// The model the family `M` runs with, the bare flag's where the variable left the default, or `None` where the
    /// family is switched off.
    pub fn model_for<M: FamilyModel>(&self) -> Option<M> {
        M::choice(self)
    }
}

/// The model `M`'s variable chooses, read through `lookup`: the bare flag's where it is unset or empty, `None` where
/// it says `off`.
fn read_one<M: FamilyModel>(lookup: impl Fn(&str) -> Option<OsString>) -> Result<Option<M>, clap::Error> {
    let Some(value) = lookup(M::VARIABLE).filter(|value| !value.is_empty()) else {
        return Ok(Some(M::DEFAULT));
    };

    let text = value.to_str();
    if text.is_some_and(|text| text.eq_ignore_ascii_case(OFF)) {
        return Ok(None);
    }

    text.and_then(|text| M::from_str(text, true).ok()).map(Some).ok_or_else(|| {
        let accepted: Vec<String> = M::value_variants()
            .iter()
            .filter_map(ValueEnum::to_possible_value)
            .map(|value| value.get_name().to_owned())
            .chain([OFF.to_owned()])
            .collect();

        Args::command().error(
            ErrorKind::InvalidValue,
            format!(
                "invalid value '{}' for the environment variable {}\n  [possible values: {}]",
                value.to_string_lossy(),
                M::VARIABLE,
                accepted.join(", ")
            ),
        )
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    /// Reads the variables from `pairs` alone.
    fn read(pairs: &[(&str, &str)]) -> Result<AutopilotModels, clap::Error> {
        let map: HashMap<String, OsString> =
            pairs.iter().map(|(name, value)| ((*name).to_owned(), OsString::from(value))).collect();

        AutopilotModels::read(|name| map.get(name).cloned())
    }

    #[test]
    fn no_variable_leaves_every_family_at_its_default() {
        let models = read(&[]).expect("reads");

        assert_eq!(models, AutopilotModels::default());
        assert_eq!(models.families(), Family::APPLY_ORDER);
        assert_eq!(models.model_for(), Some(DenoiseModel::Stockholm));
        assert_eq!(models.model_for(), Some(FaceModel::Athens));
        assert_eq!(models.model_for(), Some(ColorizationModel::Delhi));
        assert_eq!(models.model_for(), Some(LightModel::Paris));
        assert_eq!(models.model_for(), Some(BalanceModel::Rio));
        assert_eq!(models.model_for(), Some(SharpenModel::Moscow));
        assert_eq!(models.model_for(), Some(UpscaleModel::Tokyo));
    }

    #[test]
    fn a_named_model_is_taken_whatever_its_case() {
        let models = read(&[
            ("OPAI_AP_DENOISE", "MALMO"),
            ("OPAI_AP_FACE", "Santorini"),
            ("OPAI_AP_COLORIZATION", "jaiPUR"),
            ("OPAI_AP_LIGHT", "Lyon"),
            ("OPAI_AP_COLOR", "SaoPaulo"),
            ("OPAI_AP_SHARPEN", "petersburg"),
            ("OPAI_AP_UPSCALE", "Osaka"),
        ])
        .expect("reads");

        assert_eq!(models.model_for(), Some(DenoiseModel::Malmo));
        assert_eq!(models.model_for(), Some(FaceModel::Santorini));
        assert_eq!(models.model_for(), Some(ColorizationModel::Jaipur));
        assert_eq!(models.model_for(), Some(LightModel::Lyon));
        assert_eq!(models.model_for(), Some(BalanceModel::SaoPaulo));
        assert_eq!(models.model_for(), Some(SharpenModel::Petersburg));
        assert_eq!(models.model_for(), Some(UpscaleModel::Osaka));
        assert_eq!(models.families(), Family::APPLY_ORDER);
    }

    #[test]
    fn off_in_any_case_leaves_the_family_out() {
        let models = read(&[("OPAI_AP_SHARPEN", "off"), ("OPAI_AP_FACE", "OFF")]).expect("reads");

        assert_eq!(models.sharpen, None);
        assert_eq!(models.face_recovery, None);
        assert_eq!(models.model_for::<SharpenModel>(), None);
        assert_eq!(models.model_for::<FaceModel>(), None);
        assert_eq!(
            models.families(),
            [
                Family::Denoise,
                Family::Colorization,
                Family::LightAdjustment,
                Family::ColorBalance,
                Family::Upscale
            ]
        );
    }

    #[test]
    fn an_empty_value_is_the_default() {
        let models = read(&[("OPAI_AP_UPSCALE", ""), ("OPAI_AP_DENOISE", "")]).expect("reads");

        assert_eq!(models, AutopilotModels::default());
    }

    #[test]
    fn another_familys_model_is_refused_naming_the_variable_and_listing_the_accepted_ones() {
        let error = read(&[("OPAI_AP_UPSCALE", "paris")]).expect_err("paris is not an upscale model");

        assert_eq!(error.kind(), ErrorKind::InvalidValue);
        assert_eq!(error.exit_code(), 2);
        let message = error.to_string();
        assert!(message.contains("OPAI_AP_UPSCALE") && message.contains("'paris'"), "{message}");
        assert!(message.contains("[possible values: kyoto, osaka, tokyo, saitama, off]"), "{message}");
    }

    #[test]
    fn every_family_refuses_an_unknown_model_with_its_own_list() {
        for (variable, accepted) in [
            ("OPAI_AP_DENOISE", "stockholm, gothenburg, malmo, off"),
            ("OPAI_AP_FACE", "athens, santorini, off"),
            ("OPAI_AP_COLORIZATION", "delhi, mumbai, jaipur, off"),
            ("OPAI_AP_LIGHT", "paris, lyon, off"),
            ("OPAI_AP_COLOR", "rio, saopaulo, off"),
            ("OPAI_AP_SHARPEN", "moscow, petersburg, novgorod, off"),
        ] {
            let error = read(&[(variable, "tokyo")]).expect_err(variable);
            let message = error.to_string();

            assert!(message.contains(variable), "{message}");
            assert!(message.contains(&format!("[possible values: {accepted}]")), "{message}");
        }
    }

    #[test]
    fn every_default_is_the_bare_flags() {
        let bare = crate::args::parse_from("opai photo.jpg --dn --fr --cl --la --cb --sh --up -o out.png".split(' '))
            .expect("parses");

        assert_eq!(bare.selection.denoise, Some(DenoiseModel::DEFAULT));
        assert_eq!(bare.selection.face_recovery, Some(FaceModel::DEFAULT));
        assert_eq!(bare.selection.colorization, Some(ColorizationModel::DEFAULT));
        assert_eq!(bare.selection.light_adjustment, Some(LightModel::DEFAULT));
        assert_eq!(bare.selection.color_balance, Some(BalanceModel::DEFAULT));
        assert_eq!(bare.selection.sharpen, Some(SharpenModel::DEFAULT));
        assert_eq!(bare.selection.upscale, Some(UpscaleModel::DEFAULT));
    }

    /// Autopilot's default model is the one the library's catalogue lists first, which is what the GUI falls back to.
    #[test]
    fn every_default_is_the_catalogues_first_variant() {
        fn name(value: &impl ValueEnum) -> String {
            value.to_possible_value().expect("no variant is skipped").get_name().to_owned()
        }

        fn first(family: Family) -> &'static str {
            let entry = opai::catalogue().iter().find(|entry| entry.family == family).expect("published");
            entry.variants[0].codename
        }

        fn check<M: FamilyModel>(models: &AutopilotModels) {
            let model: M = models.model_for().expect("on by default");
            assert_eq!(name(&model), first(M::FAMILY), "{:?}", M::FAMILY);
        }

        let models = AutopilotModels::default();
        check::<DenoiseModel>(&models);
        check::<FaceModel>(&models);
        check::<ColorizationModel>(&models);
        check::<LightModel>(&models);
        check::<BalanceModel>(&models);
        check::<SharpenModel>(&models);
        check::<UpscaleModel>(&models);
    }
}
