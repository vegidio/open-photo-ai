//! The command line: what `opai` accepts, its defaults, and the checks clap can make before anything is touched.

use std::ffi::OsString;
use std::path::PathBuf;

use clap::{CommandFactory, Parser, ValueEnum};
use opai::Scale;

/// Enhance one photograph with Open Photo AI.
///
/// The enhancements run in one order whatever order the flags are given in: denoise, face recovery, colorization, light
/// adjustment, colour balance, sharpen, and then upscale. The output's format is the one its extension names, and an
/// existing file there is overwritten.
#[derive(Debug, Parser)]
#[command(name = "opai", version = opai::version(), override_usage = "opai <INPUT> [OPTIONS] --output <FILE>")]
pub struct Args {
    // Positional and first, so a bare enhancement flag is never handed the input as its model name. A user who writes
    // `opai --up photo.jpg` is told `photo.jpg` is not a model, which is clear and costs nothing.
    /// The photograph to enhance.
    #[arg(value_name = "INPUT")]
    pub input: PathBuf,

    // The enhancements are declared in the library's apply order, which is the order `--help` lists them in.
    /// Remove the noise from the photograph, with MODEL or Stockholm where none is named.
    #[arg(
        long = "denoise",
        visible_alias = "dn",
        value_name = "MODEL",
        num_args = 0..=1,
        default_missing_value = "stockholm",
        ignore_case = true
    )]
    pub denoise: Option<DenoiseModel>,

    /// The denoise strength, from 1 to 300 percent of the model's own effect; default 100.
    #[arg(long = "dn-str", value_name = "STRENGTH", value_parser = parse_strength, requires = "denoise")]
    pub denoise_strength: Option<u16>,

    /// Restore the faces in the photograph, with MODEL or Athens where none is named.
    #[arg(
        long = "face-recovery",
        visible_alias = "fr",
        value_name = "MODEL",
        num_args = 0..=1,
        default_missing_value = "athens",
        ignore_case = true
    )]
    pub face_recovery: Option<FaceModel>,

    /// Colorize the photograph, with MODEL or Delhi where none is named.
    #[arg(
        long = "colorization",
        visible_alias = "cl",
        value_name = "MODEL",
        num_args = 0..=1,
        default_missing_value = "delhi",
        ignore_case = true
    )]
    pub colorization: Option<ColorizationModel>,

    /// Adjust the photograph's light, with MODEL or Paris where none is named.
    #[arg(
        long = "light-adjustment",
        visible_alias = "la",
        value_name = "MODEL",
        num_args = 0..=1,
        default_missing_value = "paris",
        ignore_case = true
    )]
    pub light_adjustment: Option<LightModel>,

    /// The light adjustment bias, from -100 to 100 percent of the model's own effect; default 50.
    #[arg(
        long = "la-bias",
        value_name = "BIAS",
        value_parser = parse_bias,
        requires = "light_adjustment",
        allow_negative_numbers = true
    )]
    pub light_bias: Option<i16>,

    /// Balance the photograph's colours, with MODEL or Rio where none is named.
    #[arg(
        long = "color-balance",
        visible_alias = "cb",
        value_name = "MODEL",
        num_args = 0..=1,
        default_missing_value = "rio",
        ignore_case = true
    )]
    pub color_balance: Option<BalanceModel>,

    /// The colour balance bias, from -100 to 100 percent of the model's own effect; default 50.
    #[arg(
        long = "cb-bias",
        value_name = "BIAS",
        value_parser = parse_bias,
        requires = "color_balance",
        allow_negative_numbers = true
    )]
    pub balance_bias: Option<i16>,

    /// Sharpen the photograph, with MODEL or Moscow where none is named.
    #[arg(
        long = "sharpen",
        visible_alias = "sh",
        value_name = "MODEL",
        num_args = 0..=1,
        default_missing_value = "moscow",
        ignore_case = true
    )]
    pub sharpen: Option<SharpenModel>,

    /// The sharpen strength, from 1 to 300 percent of the model's own effect; default 100.
    #[arg(long = "sh-str", value_name = "STRENGTH", value_parser = parse_strength, requires = "sharpen")]
    pub sharpen_strength: Option<u16>,

    /// Upscale the photograph, with MODEL or Tokyo where none is named.
    #[arg(
        long = "upscale",
        visible_alias = "up",
        value_name = "MODEL",
        num_args = 0..=1,
        default_missing_value = "tokyo",
        ignore_case = true
    )]
    pub upscale: Option<UpscaleModel>,

    /// The factor to upscale by, from 1.0 to 8.0. Without it, the factor is what the photograph needs: 4x up to
    /// 1 MP, 2x up to 4 MP, and no upscale above that.
    #[arg(long, value_name = "FACTOR", value_parser = parse_scale, requires = "upscale")]
    pub scale: Option<Scale>,

    /// `hd` runs every model at its higher-fidelity build, `sd` at its lighter, faster one.
    #[arg(long, short = 'q', value_enum, default_value_t = Quality::Hd, ignore_case = true)]
    pub quality: Quality,

    /// The file to write. Its extension chooses the format.
    #[arg(long, short = 'o', value_name = "FILE")]
    pub output: PathBuf,
}

/// The upscale models, as the command line names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum UpscaleModel {
    Kyoto,
    Osaka,
    Tokyo,
    Saitama,
}

/// The face-recovery models, as the command line names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum FaceModel {
    Athens,
    Santorini,
}

/// The denoise models, as the command line names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DenoiseModel {
    Stockholm,
    Gothenburg,
    Malmo,
}

/// The sharpen models, as the command line names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SharpenModel {
    Moscow,
    Petersburg,
    Novgorod,
}

/// The colorization models, as the command line names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorizationModel {
    Delhi,
    Mumbai,
    Jaipur,
}

/// The light adjustment models, as the command line names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LightModel {
    Paris,
    Lyon,
}

/// The colour balance models, as the command line names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BalanceModel {
    Rio,
    // The library's codename, which clap would otherwise spell `sao-paulo`.
    #[value(name = "saopaulo")]
    SaoPaulo,
}

/// Which of its published builds each model runs at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Quality {
    /// The higher-fidelity build: FP32, or FP16 for Osaka.
    Hd,
    /// The lighter build: FP16, or INT8 for Osaka.
    Sd,
}

/// The command line this process was started with, or the process ended with clap's message and exit status.
pub fn parse() -> Args {
    parse_from(std::env::args_os()).unwrap_or_else(|error| error.exit())
}

/// `arguments` parsed and checked, every refusal a clap error so it is reported and exits as clap's own are.
pub fn parse_from<I, T>(arguments: I) -> Result<Args, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args = Args::try_parse_from(arguments)?;

    // Not a clap `ArgGroup`: its refusal lists the group's members as "required arguments", where what the user needs
    // to hear is that an enhancement is.
    let requested = [
        args.denoise.is_some(),
        args.face_recovery.is_some(),
        args.colorization.is_some(),
        args.light_adjustment.is_some(),
        args.color_balance.is_some(),
        args.sharpen.is_some(),
        args.upscale.is_some(),
    ];
    if !requested.contains(&true) {
        return Err(Args::command().error(
            clap::error::ErrorKind::MissingRequiredArgument,
            "at least one enhancement is required: --denoise (--dn), --face-recovery (--fr), --colorization (--cl), \
             --light-adjustment (--la), --color-balance (--cb), --sharpen (--sh) or --upscale (--up)",
        ));
    }

    Ok(args)
}

/// A `--scale` value, refused with the accepted range where the library's [`Scale`] refuses it.
fn parse_scale(text: &str) -> Result<Scale, String> {
    let range = || format!("the scale must be from {:.1} to {:.1}", Scale::MIN, Scale::MAX);

    let value: f64 = text.trim().parse().map_err(|_| format!("{text:?} is not a number; {}", range()))?;
    Scale::new(value).map_err(|_| range())
}

/// The lowest `--dn-str` or `--sh-str`. Not the library's 0: a strength of 0 applies nothing, which on a command line is
/// a mistake rather than a choice.
const STRENGTH_MIN: u16 = 1;

/// The highest `--dn-str` or `--sh-str`, the library's [`opai::Strength::MAX`] as a percentage.
const STRENGTH_MAX: u16 = 300;

/// The lowest `--la-bias` or `--cb-bias`, the library's [`opai::Bias::MIN`] as a percentage.
const BIAS_MIN: i16 = -100;

/// The highest `--la-bias` or `--cb-bias`, the library's [`opai::Bias::MAX`] as a percentage.
const BIAS_MAX: i16 = 100;

/// A `--dn-str` or `--sh-str` value: a whole percentage, refused with the accepted range otherwise.
fn parse_strength(text: &str) -> Result<u16, String> {
    text.trim()
        .parse()
        .ok()
        .filter(|value| (STRENGTH_MIN..=STRENGTH_MAX).contains(value))
        .ok_or_else(|| format!("the strength must be a whole number from {STRENGTH_MIN} to {STRENGTH_MAX}"))
}

/// A `--la-bias` or `--cb-bias` value: a whole percentage, refused with the accepted range otherwise.
fn parse_bias(text: &str) -> Result<i16, String> {
    text.trim()
        .parse()
        .ok()
        .filter(|value| (BIAS_MIN..=BIAS_MAX).contains(value))
        .ok_or_else(|| format!("the bias must be a whole number from {BIAS_MIN} to {BIAS_MAX}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    fn parse(line: &str) -> Result<Args, clap::Error> {
        parse_from(line.split_whitespace())
    }

    #[test]
    fn bare_flags_take_their_default_models_and_quality() {
        let args = parse("opai photo.jpg --up --fr -o out.png").expect("parses");

        assert_eq!(args.input, PathBuf::from("photo.jpg"));
        assert_eq!(args.upscale, Some(UpscaleModel::Tokyo));
        assert_eq!(args.face_recovery, Some(FaceModel::Athens));
        assert_eq!(args.quality, Quality::Hd);
        assert_eq!(args.scale, None);
        assert_eq!(args.output, PathBuf::from("out.png"));
    }

    #[test]
    fn the_long_names_are_the_aliases_equals() {
        let args = parse("opai photo.jpg --upscale --face-recovery --quality sd --output out.png").expect("parses");

        assert_eq!(args.upscale, Some(UpscaleModel::Tokyo));
        assert_eq!(args.face_recovery, Some(FaceModel::Athens));
        assert_eq!(args.quality, Quality::Sd);
    }

    #[test]
    fn a_named_model_is_taken_whatever_its_case() {
        let args = parse("opai photo.jpg --up OSAKA --fr Santorini -q SD -o out.png").expect("parses");

        assert_eq!(args.upscale, Some(UpscaleModel::Osaka));
        assert_eq!(args.face_recovery, Some(FaceModel::Santorini));
        assert_eq!(args.quality, Quality::Sd);

        for (name, model) in [
            ("kyoto", UpscaleModel::Kyoto),
            ("osaka", UpscaleModel::Osaka),
            ("tokyo", UpscaleModel::Tokyo),
            ("saitama", UpscaleModel::Saitama),
        ] {
            let args = parse(&format!("opai photo.jpg --up {name} -o out.png")).expect("parses");
            assert_eq!(args.upscale, Some(model), "{name}");
        }
    }

    #[test]
    fn an_unknown_model_is_a_usage_error_listing_the_accepted_ones() {
        let error = parse("opai photo.jpg --up paris -o out.png").expect_err("paris is not an upscale model");

        assert_eq!(error.kind(), ErrorKind::InvalidValue);
        assert_eq!(error.exit_code(), 2);
        let message = error.to_string();
        for model in ["kyoto", "osaka", "tokyo", "saitama"] {
            assert!(message.contains(model), "{model} is not listed in: {message}");
        }

        let error = parse("opai photo.jpg --fr tokyo -o out.png").expect_err("tokyo is not a face model");
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn a_scale_requires_an_upscale() {
        let error = parse("opai photo.jpg --fr --scale 2 -o out.png").expect_err("--scale without --up");

        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
        assert!(error.to_string().contains("--upscale"), "{error}");
    }

    #[test]
    fn a_scale_out_of_range_is_refused_naming_the_range() {
        for scale in ["9", "0.5", "abc"] {
            let error = parse(&format!("opai photo.jpg --up --scale {scale} -o out.png")).expect_err(scale);

            assert_eq!(error.kind(), ErrorKind::ValueValidation, "{scale}");
            assert!(error.to_string().contains("from 1.0 to 8.0"), "{error}");
        }

        let args = parse("opai photo.jpg --up --scale 2.5 -o out.png").expect("2.5 is in range");
        assert_eq!(args.scale.map(Scale::get), Some(2.5));
    }

    #[test]
    fn no_enhancement_is_a_usage_error() {
        let error = parse("opai photo.jpg -o out.png").expect_err("nothing to do");

        assert_eq!(error.exit_code(), 2);
        let message = error.to_string();
        assert!(message.contains("at least one enhancement is required"), "{message}");
        for (flag, alias) in [
            ("--denoise", "--dn"),
            ("--face-recovery", "--fr"),
            ("--colorization", "--cl"),
            ("--light-adjustment", "--la"),
            ("--color-balance", "--cb"),
            ("--sharpen", "--sh"),
            ("--upscale", "--up"),
        ] {
            assert!(message.contains(&format!("{flag} ({alias})")), "{flag} is not listed in: {message}");
        }
    }

    #[test]
    fn a_new_family_alone_is_enough() {
        let args = parse("opai photo.jpg --cl -o out.png").expect("parses");

        assert_eq!(args.colorization, Some(ColorizationModel::Delhi));
        assert_eq!(args.upscale, None);
        assert_eq!(args.face_recovery, None);
    }

    #[test]
    fn bare_new_flags_take_their_default_models_and_no_parameters() {
        let args = parse("opai photo.jpg --dn --sh --cl --la --cb -o out.png").expect("parses");

        assert_eq!(args.denoise, Some(DenoiseModel::Stockholm));
        assert_eq!(args.sharpen, Some(SharpenModel::Moscow));
        assert_eq!(args.colorization, Some(ColorizationModel::Delhi));
        assert_eq!(args.light_adjustment, Some(LightModel::Paris));
        assert_eq!(args.color_balance, Some(BalanceModel::Rio));
        assert_eq!(
            (args.denoise_strength, args.sharpen_strength, args.light_bias, args.balance_bias),
            (None, None, None, None)
        );

        let long = parse("opai photo.jpg --denoise --sharpen --colorization --light-adjustment --color-balance -o x")
            .expect("parses");
        assert_eq!(long.denoise, Some(DenoiseModel::Stockholm));
        assert_eq!(long.sharpen, Some(SharpenModel::Moscow));
        assert_eq!(long.colorization, Some(ColorizationModel::Delhi));
        assert_eq!(long.light_adjustment, Some(LightModel::Paris));
        assert_eq!(long.color_balance, Some(BalanceModel::Rio));
    }

    #[test]
    fn a_named_new_model_is_taken_whatever_its_case() {
        let args = parse("opai photo.jpg --dn MALMO --sh Petersburg --cl jaiPUR --la LYON --cb SaoPaulo -o out.png")
            .expect("parses");

        assert_eq!(args.denoise, Some(DenoiseModel::Malmo));
        assert_eq!(args.sharpen, Some(SharpenModel::Petersburg));
        assert_eq!(args.colorization, Some(ColorizationModel::Jaipur));
        assert_eq!(args.light_adjustment, Some(LightModel::Lyon));
        assert_eq!(args.color_balance, Some(BalanceModel::SaoPaulo));

        let args = parse("opai photo.jpg --dn gothenburg --sh novgorod --cl mumbai -o out.png").expect("parses");
        assert_eq!(args.denoise, Some(DenoiseModel::Gothenburg));
        assert_eq!(args.sharpen, Some(SharpenModel::Novgorod));
        assert_eq!(args.colorization, Some(ColorizationModel::Mumbai));
    }

    #[test]
    fn an_unknown_new_model_is_a_usage_error_listing_the_accepted_ones() {
        let cases: [(&str, &[&str]); 5] = [
            ("--dn tokyo", &["stockholm", "gothenburg", "malmo"]),
            ("--sh tokyo", &["moscow", "petersburg", "novgorod"]),
            ("--cl tokyo", &["delhi", "mumbai", "jaipur"]),
            ("--la tokyo", &["paris", "lyon"]),
            ("--cb sao-paulo", &["rio", "saopaulo"]),
        ];

        for (flags, accepted) in cases {
            let error = parse(&format!("opai photo.jpg {flags} -o out.png")).expect_err(flags);

            assert_eq!(error.kind(), ErrorKind::InvalidValue, "{flags}: {error}");
            let message = error.to_string();
            for model in accepted {
                assert!(message.contains(model), "{model} is not listed in: {message}");
            }
        }
    }

    #[test]
    fn a_strength_out_of_range_or_fractional_is_refused_naming_the_range() {
        for (flags, range) in [
            ("--dn --dn-str 0", "1 to 300"),
            ("--dn --dn-str 301", "1 to 300"),
            ("--dn --dn-str 150.5", "1 to 300"),
            ("--sh --sh-str 1000", "1 to 300"),
            ("--cb --cb-bias 101", "-100 to 100"),
            ("--la --la-bias -101", "-100 to 100"),
            ("--la --la-bias 0.5", "-100 to 100"),
        ] {
            let error = parse(&format!("opai photo.jpg {flags} -o out.png")).expect_err(flags);

            assert_eq!(error.kind(), ErrorKind::ValueValidation, "{flags}: {error}");
            assert!(error.to_string().contains(&format!("a whole number from {range}")), "{error}");
        }
    }

    #[test]
    fn each_parameter_is_read_for_its_own_enhancement() {
        let args = parse("opai photo.jpg --dn --dn-str 200 --sh --sh-str 80 -o out.png").expect("parses");
        assert_eq!(args.denoise_strength, Some(200));
        assert_eq!(args.sharpen_strength, Some(80));

        let args = parse("opai photo.jpg --la --la-bias -20 -o out.png").expect("parses");
        assert_eq!(args.light_bias, Some(-20));
        assert_eq!(args.output, PathBuf::from("out.png"));

        let args = parse("opai photo.jpg --cb --cb-bias -100 --la --la-bias 100 -o out.png").expect("parses");
        assert_eq!(args.balance_bias, Some(-100));
        assert_eq!(args.light_bias, Some(100));
    }

    #[test]
    fn a_parameter_requires_its_enhancement() {
        for (flags, required) in [
            ("--sh --dn-str 150", "--denoise"),
            ("--dn --sh-str 150", "--sharpen"),
            ("--cb --la-bias 20", "--light-adjustment"),
            ("--la --cb-bias 20", "--color-balance"),
        ] {
            let error = parse(&format!("opai photo.jpg {flags} -o out.png")).expect_err(flags);

            assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument, "{flags}: {error}");
            assert!(error.to_string().contains(required), "{error}");
        }
    }

    /// The one place the CLI's model names, default models, parameter ranges and documented defaults are checked
    /// against what the library publishes, so a variant added or renamed there fails here rather than going unoffered.
    #[test]
    fn the_new_families_match_the_librarys_catalogue() {
        use opai::{Family, ParameterKind};

        fn name(value: &impl ValueEnum) -> String {
            value.to_possible_value().expect("no variant is skipped").get_name().to_owned()
        }

        fn names<T: ValueEnum>() -> Vec<String> {
            T::value_variants().iter().map(name).collect()
        }

        let command = Args::command();
        let bare = parse("opai photo.jpg --dn --sh --cl --la --cb -o out.png").expect("parses");
        let families = [
            (Family::Denoise, names::<DenoiseModel>(), bare.denoise.as_ref().map(name)),
            (Family::Sharpen, names::<SharpenModel>(), bare.sharpen.as_ref().map(name)),
            (Family::Colorization, names::<ColorizationModel>(), bare.colorization.as_ref().map(name)),
            (Family::LightAdjustment, names::<LightModel>(), bare.light_adjustment.as_ref().map(name)),
            (Family::ColorBalance, names::<BalanceModel>(), bare.color_balance.as_ref().map(name)),
        ];

        for (family, names, default_model) in families {
            let entry = opai::catalogue().iter().find(|entry| entry.family == family).expect("published");
            let published: Vec<_> = entry.variants.iter().map(|variant| variant.codename).collect();
            assert_eq!(names, published, "{family:?}");
            assert_eq!(default_model.as_deref(), Some(published[0]), "{family:?}'s default model");

            // Every variant of these families takes the same parameter, so the first one speaks for the family.
            let documented = match family {
                Family::Denoise | Family::Sharpen => Some(("strength", 100.0)),
                Family::LightAdjustment | Family::ColorBalance => Some(("bias", 50.0)),
                _ => None,
            };
            let parameters = entry.variants[0].parameters;
            let Some((name, default)) = documented else {
                assert!(parameters.is_empty(), "{family:?} takes {parameters:?}");
                continue;
            };
            let [parameter] = parameters else { panic!("{family:?} takes {parameters:?}") };
            let ParameterKind::Range { min, max, default: published_default } = parameter.kind else {
                panic!("{family:?}'s {name} is not a range")
            };
            assert_eq!(parameter.name, name, "{family:?}");
            assert_eq!(published_default * 100.0, default, "{family:?}'s documented default");

            if name == "strength" {
                assert_eq!(max * 100.0, f64::from(STRENGTH_MAX), "{family:?}");
                assert!(f64::from(STRENGTH_MIN) > min * 100.0, "{family:?}");
            } else {
                assert_eq!((min * 100.0, max * 100.0), (f64::from(BIAS_MIN), f64::from(BIAS_MAX)), "{family:?}");
            }

            let help = argument_help(&command, family);
            assert!(help.contains(&format!("default {default}")), "{family:?}: {help}");
        }
    }

    /// The help line of the parameter flag `family` takes.
    fn argument_help(command: &clap::Command, family: opai::Family) -> String {
        use opai::Family;

        let id = match family {
            Family::Denoise => "denoise_strength",
            Family::Sharpen => "sharpen_strength",
            Family::LightAdjustment => "light_bias",
            Family::ColorBalance => "balance_bias",
            _ => unreachable!("{family:?} takes no parameter flag"),
        };
        let argument = command.get_arguments().find(|argument| argument.get_id() == id).expect(id);
        argument.get_help().expect("documented").to_string()
    }

    #[test]
    fn the_output_is_required() {
        let error = parse("opai photo.jpg --up").expect_err("no -o");

        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn a_bare_flag_before_another_flag_takes_its_default() {
        let args = parse("opai photo.jpg --up -o x").expect("parses");

        assert_eq!(args.input, PathBuf::from("photo.jpg"));
        assert_eq!(args.upscale, Some(UpscaleModel::Tokyo));
        assert_eq!(args.output, PathBuf::from("x"));
    }

    #[test]
    fn an_input_after_a_bare_flag_is_read_as_its_model() {
        let error = parse("opai --up photo.jpg -o out.png").expect_err("photo.jpg is not a model");

        assert_eq!(error.kind(), ErrorKind::InvalidValue);
        assert!(error.to_string().contains("photo.jpg"), "{error}");
    }

    #[test]
    fn the_version_is_the_librarys() {
        let error = parse("opai --version").expect_err("--version ends the parse");

        assert_eq!(error.kind(), ErrorKind::DisplayVersion);
        assert!(error.to_string().contains(opai::version()), "{error}");
    }
}
