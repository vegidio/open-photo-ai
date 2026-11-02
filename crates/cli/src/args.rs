//! The command line: what `opai` accepts, its defaults, and the checks clap can make before anything is touched.

use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;

use clap::{ArgAction, CommandFactory, Parser, ValueEnum};
use opai::Scale;
use opai::export::{ExportFormat, FormatChoice};

use crate::autopilot::AutopilotModels;

/// Enhance photographs with Open Photo AI.
///
/// Each input is a photograph or a directory of them; a directory contributes the images directly inside it, or its
/// whole tree with `--recursive`. The enhancements run in one order whatever order the flags are given in: denoise,
/// face recovery, colorization, light adjustment, colour balance, sharpen, and then upscale.
///
/// The output is a file when its extension names a format opai writes and it is not an existing directory; it then
/// takes exactly one input photograph. Any other output is a directory, created if missing, that mirrors each
/// directory input. Without an output, each result is written beside its source, with the suffix `_opai` unless
/// `--prefix` or `--suffix` is given. An existing file is never overwritten unless `--overwrite` is given: the result
/// is numbered `_1`, `_2`, … instead.
#[derive(Debug, Parser)]
#[command(name = "opai", display_name = "Open Photo AI - CLI", version = opai::version(), disable_version_flag = true, override_usage = "opai <INPUT>... [OPTIONS] [--output <OUTPUT>]")]
pub struct Args {
    // Positional and first, so a bare enhancement flag is never handed an input as its model name. A user who writes
    // `opai --up photo.jpg` is told `photo.jpg` is not a model, which is clear and costs nothing.
    /// The photographs, and the directories of photographs, to enhance.
    #[arg(value_name = "INPUT", required = true, num_args = 1..)]
    pub inputs: Vec<PathBuf>,

    /// The enhancements, and their parameters.
    #[command(flatten)]
    pub selection: Selection,

    /// Analyse each photograph and apply the enhancements it calls for, each with its family's default model. Cannot
    /// be combined with the enhancement flags or their parameters.
    ///
    /// A family's model is chosen with its environment variable, set to one of that family's models or to `off`,
    /// which leaves the family out: OPAI_AP_DENOISE, OPAI_AP_FACE, OPAI_AP_COLORIZATION, OPAI_AP_LIGHT,
    /// OPAI_AP_COLOR, OPAI_AP_SHARPEN and OPAI_AP_UPSCALE.
    #[arg(long = "autopilot", visible_alias = "ap", conflicts_with = "Selection")]
    pub autopilot: bool,

    /// `hd` runs every model at its higher-fidelity build, `sd` at its lighter, faster one.
    #[arg(long, short = 'q', value_enum, default_value_t = Quality::Hd, ignore_case = true)]
    pub quality: Quality,

    /// The hardware to run on: `auto` (the best this machine has; the default), or one processor by name. One this
    /// machine does not offer is refused once initialization has finished.
    #[arg(long, short = 'p', value_enum, default_value_t = Processor::Auto, ignore_case = true)]
    pub processor: Processor,

    /// Take a directory input's whole tree, not only the images directly inside it.
    #[arg(long, short = 'r')]
    pub recursive: bool,

    /// The format every result is written in, unless the output is a file: `preserve` (each source's own, TIFF for
    /// camera RAW; the default), avif, bmp, gif, heic, jpeg, png, tiff or webp.
    #[arg(long, value_name = "FORMAT", value_parser = parse_format)]
    pub format: Option<FormatChoice>,

    /// Written before each source's name, unless the output is a file.
    #[arg(long, value_name = "TEXT", allow_hyphen_values = true)]
    pub prefix: Option<String>,

    /// Written after each source's name, before the extension, unless the output is a file. Without an output it
    /// defaults to `_opai`, unless `--prefix` is given; `--suffix ""` turns it off.
    #[arg(long, value_name = "TEXT", allow_hyphen_values = true)]
    pub suffix: Option<String>,

    /// Replace a file already at a destination, once the new one is complete, instead of numbering the result.
    #[arg(long)]
    pub overwrite: bool,

    /// The file or directory to write to. Without it, each result is written beside its source, named with the suffix
    /// `_opai` unless `--prefix` or `--suffix` is given.
    #[arg(long, short = 'o', value_name = "OUTPUT")]
    pub output: Option<PathBuf>,

    /// Name, on each written image's line, the enhancements it received: each model, its strength, bias or upscale
    /// factor, and whether the run cache served them. Without it the line names only the path.
    #[arg(long)]
    pub verbose: bool,

    /// Send no usage or diagnostic data to the project for this run. Setting OPAI_ANALYTICS to 0, off, false or no
    /// does the same.
    #[arg(long)]
    pub no_analytics: bool,

    /// Print version.
    #[arg(long, short = 'v', action = ArgAction::Version)]
    version: (),

    /// The model autopilot runs each family with, read from the `OPAI_AP_*` variables by [`parse`] under
    /// `--autopilot` only, and `None` otherwise.
    #[arg(skip)]
    pub autopilot_models: Option<AutopilotModels>,
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

impl Quality {
    /// How the processor line names it: `HD` or `SD`.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hd => "HD",
            Self::Sd => "SD",
        }
    }

    /// Which of a model's published precisions this names, by position: the first build the catalogue lists, or the
    /// second.
    pub const fn position(self) -> usize {
        match self {
            Self::Hd => 0,
            Self::Sd => 1,
        }
    }
}

// `#[cfg]` is resolved before `ValueEnum` expands, so a value no machine running this OS could ever offer is missing
// from the parser, its `[possible values: …]` and `--help` alike. Whether this machine offers the rest is `opai`'s to
// say, once initialization has run.
/// The execution providers, as the command line names them. Only the ones this OS can ever offer exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum Processor {
    #[default]
    Auto,
    Cpu,
    // The explicit names stop clap from spelling these `core-ml` and `tensor-rt`.
    #[cfg(target_os = "macos")]
    #[value(name = "coreml")]
    CoreMl,
    #[cfg(not(target_os = "macos"))]
    Cuda,
    #[cfg(not(target_os = "macos"))]
    #[value(name = "tensorrt")]
    TensorRt,
    #[value(name = "webgpu")]
    WebGpu,
}

impl Processor {
    /// The name the command line takes for this processor.
    pub fn name(self) -> String {
        self.to_possible_value().expect("no processor is skipped").get_name().to_owned()
    }

    /// The processor `provider` is named by on this build's command line, or `None` where this OS has no value for it.
    pub fn from_provider(provider: opai::ExecutionProvider) -> Option<Self> {
        Self::value_variants()
            .iter()
            .copied()
            .find(|processor| opai::ExecutionProvider::from(*processor) == provider)
    }
}

impl From<Processor> for opai::ExecutionProvider {
    fn from(processor: Processor) -> Self {
        match processor {
            Processor::Auto => Self::Auto,
            Processor::Cpu => Self::Cpu,
            #[cfg(target_os = "macos")]
            Processor::CoreMl => Self::CoreMl,
            #[cfg(not(target_os = "macos"))]
            Processor::Cuda => Self::Cuda,
            #[cfg(not(target_os = "macos"))]
            Processor::TensorRt => Self::TensorRt,
            Processor::WebGpu => Self::WebGpu,
        }
    }
}

/// The command line this process was started with, or the process ended with clap's message and exit status.
///
/// Read through `wild`, which expands the wildcards `cmd` and PowerShell leave as written, and on every other platform
/// hands the arguments over unchanged. Under `--autopilot`, the `OPAI_AP_*` variables are read here too, so a bad one
/// is refused as a bad flag is, before anything is installed; without it they are not looked at.
pub fn parse() -> Args {
    let mut args = parse_from(wild::args_os()).unwrap_or_else(|error| error.exit());

    if args.autopilot {
        let models = AutopilotModels::read(|name| std::env::var_os(name)).unwrap_or_else(|error| error.exit());
        args.autopilot_models = Some(models);
    }

    args
}

/// Which enhancement runs with which model and parameters: what the flags ask for, or what autopilot chose for one
/// photograph, and what [`plan`](crate::chain::plan) reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, clap::Args)]
pub struct Selection {
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
}

impl Selection {
    /// Whether no enhancement is asked for.
    pub const fn is_empty(&self) -> bool {
        self.denoise.is_none()
            && self.face_recovery.is_none()
            && self.colorization.is_none()
            && self.light_adjustment.is_none()
            && self.color_balance.is_none()
            && self.sharpen.is_none()
            && self.upscale.is_none()
    }
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
    if !args.autopilot && args.selection.is_empty() {
        return Err(Args::command().error(
            clap::error::ErrorKind::MissingRequiredArgument,
            "at least one enhancement is required: --denoise (--dn), --face-recovery (--fr), --colorization (--cl), \
             --light-adjustment (--la), --color-balance (--cb), --sharpen (--sh), --upscale (--up) or \
             --autopilot (--ap)",
        ));
    }

    Ok(args)
}

/// A `--format` value, in any case, with every extension a format is written under — `jpg`, `tif`, `heif` — read as
/// the format it names.
///
/// The aliases are this command's: the library's [`FormatChoice`] reads the one spelling the GUI stores, and widening it
/// would make the GUI's wire accept words its window never sends.
fn parse_format(text: &str) -> Result<FormatChoice, String> {
    let word = text.trim().to_lowercase();

    FormatChoice::from_str(&word)
        .ok()
        .or_else(|| ExportFormat::preserving(&word).map(FormatChoice::Format))
        .ok_or_else(|| {
            let names: Vec<_> = ExportFormat::ALL.into_iter().map(ExportFormat::name).collect();
            format!("{text:?} is not a format; use preserve, {}", names.join(", "))
        })
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
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a whole, positive percentage"
)]
const STRENGTH_MAX: u16 = (opai::Strength::MAX * 100.0) as u16;

/// The lowest `--la-bias` or `--cb-bias`, the library's [`opai::Bias::MIN`] as a percentage.
#[allow(clippy::cast_possible_truncation, reason = "a whole percentage")]
const BIAS_MIN: i16 = (opai::Bias::MIN * 100.0) as i16;

/// The highest `--la-bias` or `--cb-bias`, the library's [`opai::Bias::MAX`] as a percentage.
#[allow(clippy::cast_possible_truncation, reason = "a whole percentage")]
const BIAS_MAX: i16 = (opai::Bias::MAX * 100.0) as i16;

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

        assert_eq!(args.inputs, [PathBuf::from("photo.jpg")]);
        assert_eq!(args.selection.upscale, Some(UpscaleModel::Tokyo));
        assert_eq!(args.selection.face_recovery, Some(FaceModel::Athens));
        assert_eq!(args.quality, Quality::Hd);
        assert_eq!(args.selection.scale, None);
        assert_eq!(args.output, Some(PathBuf::from("out.png")));
    }

    #[test]
    fn analytics_is_on_unless_the_flag_turns_it_off() {
        assert!(!parse("opai photo.jpg --up -o out.png").expect("parses").no_analytics);
        assert!(parse("opai photo.jpg --up --no-analytics -o out.png").expect("parses").no_analytics);
    }

    #[test]
    fn the_long_names_are_the_aliases_equals() {
        let args = parse("opai photo.jpg --upscale --face-recovery --quality sd --output out.png").expect("parses");

        assert_eq!(args.selection.upscale, Some(UpscaleModel::Tokyo));
        assert_eq!(args.selection.face_recovery, Some(FaceModel::Athens));
        assert_eq!(args.quality, Quality::Sd);
    }

    #[test]
    fn a_named_model_is_taken_whatever_its_case() {
        let args = parse("opai photo.jpg --up OSAKA --fr Santorini -q SD -o out.png").expect("parses");

        assert_eq!(args.selection.upscale, Some(UpscaleModel::Osaka));
        assert_eq!(args.selection.face_recovery, Some(FaceModel::Santorini));
        assert_eq!(args.quality, Quality::Sd);

        for (name, model) in [
            ("kyoto", UpscaleModel::Kyoto),
            ("osaka", UpscaleModel::Osaka),
            ("tokyo", UpscaleModel::Tokyo),
            ("saitama", UpscaleModel::Saitama),
        ] {
            let args = parse(&format!("opai photo.jpg --up {name} -o out.png")).expect("parses");
            assert_eq!(args.selection.upscale, Some(model), "{name}");
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
        assert_eq!(args.selection.scale.map(Scale::get), Some(2.5));
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
            ("--autopilot", "--ap"),
        ] {
            assert!(message.contains(&format!("{flag} ({alias})")), "{flag} is not listed in: {message}");
        }
    }

    #[test]
    fn autopilot_alone_is_enough() {
        let args = parse("opai photo.jpg --ap -o out.png").expect("parses");
        assert!(args.autopilot);
        assert_eq!(args.selection, Selection::default());

        assert!(parse("opai photo.jpg --autopilot -q sd -p cpu -o out.png").expect("parses").autopilot);
        assert!(!parse("opai photo.jpg --up -o out.png").expect("parses").autopilot);
    }

    #[test]
    fn the_environment_is_not_read_by_parse_from() {
        assert_eq!(parse("opai photo.jpg --up -o out.png").expect("parses").autopilot_models, None);
        assert_eq!(parse("opai photo.jpg --ap -o out.png").expect("parses").autopilot_models, None);
    }

    #[test]
    fn autopilot_cannot_be_combined_with_an_enhancement_or_a_parameter() {
        // Each flag with what it needs to parse on its own, and the name clap gives it in the refusal. A parameter is
        // given with its enhancement, so the refusal is about autopilot rather than about a missing enhancement.
        let cases = [
            ("--dn", "--denoise"),
            ("--fr", "--face-recovery"),
            ("--cl", "--colorization"),
            ("--la", "--light-adjustment"),
            ("--cb", "--color-balance"),
            ("--sh", "--sharpen"),
            ("--up", "--upscale"),
            ("--scale 2", "--scale"),
            ("--dn-str 150", "--dn-str"),
            ("--sh-str 150", "--sh-str"),
            ("--la-bias 20", "--la-bias"),
            ("--cb-bias 20", "--cb-bias"),
        ];

        for (flags, name) in cases {
            let error = parse(&format!("opai photo.jpg --ap {flags} -o out.png")).expect_err(flags);

            assert_eq!(error.kind(), ErrorKind::ArgumentConflict, "{flags}: {error}");
            assert_eq!(error.exit_code(), 2);
            let message = error.to_string();
            assert!(message.contains("--autopilot") && message.contains(name), "{flags}: {message}");
            assert!(message.contains("cannot be used with"), "{flags}: {message}");
        }
    }

    #[test]
    fn a_new_family_alone_is_enough() {
        let args = parse("opai photo.jpg --cl -o out.png").expect("parses");

        assert_eq!(args.selection.colorization, Some(ColorizationModel::Delhi));
        assert_eq!(args.selection.upscale, None);
        assert_eq!(args.selection.face_recovery, None);
    }

    #[test]
    fn bare_new_flags_take_their_default_models_and_no_parameters() {
        let args = parse("opai photo.jpg --dn --sh --cl --la --cb -o out.png").expect("parses");

        assert_eq!(args.selection.denoise, Some(DenoiseModel::Stockholm));
        assert_eq!(args.selection.sharpen, Some(SharpenModel::Moscow));
        assert_eq!(args.selection.colorization, Some(ColorizationModel::Delhi));
        assert_eq!(args.selection.light_adjustment, Some(LightModel::Paris));
        assert_eq!(args.selection.color_balance, Some(BalanceModel::Rio));
        assert_eq!(
            (
                args.selection.denoise_strength,
                args.selection.sharpen_strength,
                args.selection.light_bias,
                args.selection.balance_bias
            ),
            (None, None, None, None)
        );

        let long = parse("opai photo.jpg --denoise --sharpen --colorization --light-adjustment --color-balance -o x")
            .expect("parses");
        assert_eq!(long.selection.denoise, Some(DenoiseModel::Stockholm));
        assert_eq!(long.selection.sharpen, Some(SharpenModel::Moscow));
        assert_eq!(long.selection.colorization, Some(ColorizationModel::Delhi));
        assert_eq!(long.selection.light_adjustment, Some(LightModel::Paris));
        assert_eq!(long.selection.color_balance, Some(BalanceModel::Rio));
    }

    #[test]
    fn a_named_new_model_is_taken_whatever_its_case() {
        let args = parse("opai photo.jpg --dn MALMO --sh Petersburg --cl jaiPUR --la LYON --cb SaoPaulo -o out.png")
            .expect("parses");

        assert_eq!(args.selection.denoise, Some(DenoiseModel::Malmo));
        assert_eq!(args.selection.sharpen, Some(SharpenModel::Petersburg));
        assert_eq!(args.selection.colorization, Some(ColorizationModel::Jaipur));
        assert_eq!(args.selection.light_adjustment, Some(LightModel::Lyon));
        assert_eq!(args.selection.color_balance, Some(BalanceModel::SaoPaulo));

        let args = parse("opai photo.jpg --dn gothenburg --sh novgorod --cl mumbai -o out.png").expect("parses");
        assert_eq!(args.selection.denoise, Some(DenoiseModel::Gothenburg));
        assert_eq!(args.selection.sharpen, Some(SharpenModel::Novgorod));
        assert_eq!(args.selection.colorization, Some(ColorizationModel::Mumbai));
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
        assert_eq!(args.selection.denoise_strength, Some(200));
        assert_eq!(args.selection.sharpen_strength, Some(80));

        let args = parse("opai photo.jpg --la --la-bias -20 -o out.png").expect("parses");
        assert_eq!(args.selection.light_bias, Some(-20));
        assert_eq!(args.output, Some(PathBuf::from("out.png")));

        let args = parse("opai photo.jpg --cb --cb-bias -100 --la --la-bias 100 -o out.png").expect("parses");
        assert_eq!(args.selection.balance_bias, Some(-100));
        assert_eq!(args.selection.light_bias, Some(100));
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
            (Family::Denoise, names::<DenoiseModel>(), bare.selection.denoise.as_ref().map(name)),
            (Family::Sharpen, names::<SharpenModel>(), bare.selection.sharpen.as_ref().map(name)),
            (
                Family::Colorization,
                names::<ColorizationModel>(),
                bare.selection.colorization.as_ref().map(name),
            ),
            (
                Family::LightAdjustment,
                names::<LightModel>(),
                bare.selection.light_adjustment.as_ref().map(name),
            ),
            (Family::ColorBalance, names::<BalanceModel>(), bare.selection.color_balance.as_ref().map(name)),
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
    fn the_output_is_optional() {
        let args = parse("opai photo.jpg --ap").expect("no -o parses");

        assert_eq!(args.output, None);
        assert_eq!(args.suffix, None);
    }

    #[test]
    fn an_empty_suffix_is_read_as_given() {
        // Not through `parse`, which splits on whitespace and so cannot pass an empty argument.
        let args = parse_from(["opai", "photo.jpg", "--dn", "--suffix", "", "--overwrite"]).expect("parses");

        assert_eq!(args.suffix.as_deref(), Some(""));
        assert_eq!(args.output, None);
    }

    #[test]
    fn a_bare_flag_before_another_flag_takes_its_default() {
        let args = parse("opai photo.jpg --up -o x").expect("parses");

        assert_eq!(args.inputs, [PathBuf::from("photo.jpg")]);
        assert_eq!(args.selection.upscale, Some(UpscaleModel::Tokyo));
        assert_eq!(args.output, Some(PathBuf::from("x")));
    }

    #[test]
    fn an_input_after_a_bare_flag_is_read_as_its_model() {
        let error = parse("opai --up photo.jpg -o out.png").expect_err("photo.jpg is not a model");

        assert_eq!(error.kind(), ErrorKind::InvalidValue);
        assert!(error.to_string().contains("photo.jpg"), "{error}");
    }

    #[test]
    fn several_inputs_are_taken_in_order() {
        let args = parse("opai a.jpg b.png scans --dn -o out").expect("parses");

        assert_eq!(args.inputs, ["a.jpg", "b.png", "scans"].map(PathBuf::from));
        assert!(!args.recursive && !args.overwrite);
        assert_eq!((args.format, args.prefix, args.suffix), (None, None, None));
    }

    #[test]
    fn an_input_is_required() {
        let error = parse("opai --up -o out").expect_err("no input");

        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn the_batch_flags_are_read() {
        let args = parse("opai photos -r --up --prefix new- --suffix -opai --overwrite -o out").expect("parses");
        assert!(args.recursive && args.overwrite);
        assert_eq!(args.prefix.as_deref(), Some("new-"));
        assert_eq!(args.suffix.as_deref(), Some("-opai"));

        let args = parse("opai photos --recursive --up -o out").expect("parses");
        assert!(args.recursive);
    }

    #[test]
    fn every_format_spelling_and_alias_is_read_whatever_its_case() {
        for (word, choice) in [
            ("preserve", FormatChoice::Preserve),
            ("PRESERVE", FormatChoice::Preserve),
            ("avif", ExportFormat::Avif.into()),
            ("BMP", ExportFormat::Bmp.into()),
            ("gif", ExportFormat::Gif.into()),
            ("heic", ExportFormat::Heic.into()),
            ("HEIF", ExportFormat::Heic.into()),
            ("jpeg", ExportFormat::Jpeg.into()),
            ("JPG", ExportFormat::Jpeg.into()),
            ("Png", ExportFormat::Png.into()),
            ("tiff", ExportFormat::Tiff.into()),
            ("Tif", ExportFormat::Tiff.into()),
            ("WebP", ExportFormat::Webp.into()),
        ] {
            let args = parse(&format!("opai photos --up --format {word} -o out")).expect(word);
            assert_eq!(args.format, Some(choice), "{word}");
        }
    }

    #[test]
    fn an_unknown_format_is_refused_listing_the_accepted_ones() {
        let error = parse("opai photos --up --format raw -o out").expect_err("raw is not a format");

        assert_eq!(error.kind(), ErrorKind::ValueValidation);
        assert_eq!(error.exit_code(), 2);
        let message = error.to_string();
        for name in ["preserve", "avif", "bmp", "gif", "heic", "jpeg", "png", "tiff", "webp"] {
            assert!(message.contains(name), "{name} is not listed in: {message}");
        }
    }

    /// The processors this OS's build accepts, in the order `--help` lists them.
    #[cfg(target_os = "macos")]
    const PROCESSORS: [&str; 4] = ["auto", "cpu", "coreml", "webgpu"];
    #[cfg(not(target_os = "macos"))]
    const PROCESSORS: [&str; 5] = ["auto", "cpu", "cuda", "tensorrt", "webgpu"];

    /// The processors this OS's build has no value for.
    #[cfg(target_os = "macos")]
    const FOREIGN_PROCESSORS: [&str; 2] = ["cuda", "tensorrt"];
    #[cfg(not(target_os = "macos"))]
    const FOREIGN_PROCESSORS: [&str; 1] = ["coreml"];

    #[test]
    fn no_processor_is_auto() {
        let args = parse("opai photo.jpg --up -o out.png").expect("parses");

        assert_eq!(args.processor, Processor::Auto);
    }

    #[test]
    fn a_processor_is_taken_whatever_its_case() {
        let args = parse("opai photo.jpg --up -p CPU -o out.png").expect("parses");
        assert_eq!(args.processor, Processor::Cpu);

        let args = parse("opai photo.jpg --up --processor WebGPU -o out.png").expect("parses");
        assert_eq!(args.processor, Processor::WebGpu);

        for name in PROCESSORS {
            let args = parse(&format!("opai photo.jpg --up -p {} -o out.png", name.to_uppercase())).expect(name);
            assert_eq!(args.processor.name(), name);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn coreml_is_a_processor_on_macos() {
        let args = parse("opai photo.jpg --up -p coreml -o out.png").expect("parses");

        assert_eq!(args.processor, Processor::CoreMl);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn cuda_and_tensorrt_are_processors_off_macos() {
        let args = parse("opai photo.jpg --up -p cuda -o out.png").expect("parses");
        assert_eq!(args.processor, Processor::Cuda);

        let args = parse("opai photo.jpg --up -p TensorRT -o out.png").expect("parses");
        assert_eq!(args.processor, Processor::TensorRt);
    }

    #[test]
    fn a_processor_this_os_never_offers_is_an_unknown_value_listing_this_builds() {
        for name in FOREIGN_PROCESSORS {
            let error = parse(&format!("opai photo.jpg --up -p {name} -o out.png")).expect_err(name);

            assert_eq!(error.kind(), ErrorKind::InvalidValue, "{name}: {error}");
            assert_eq!(error.exit_code(), 2);
            let message = error.to_string();
            assert!(message.contains(&format!("[possible values: {}]", PROCESSORS.join(", "))), "{message}");
        }
    }

    #[test]
    fn help_lists_exactly_this_builds_processors() {
        let help = Args::command().render_help().to_string();
        let line = help.lines().find(|line| line.contains("--processor")).expect("--processor is in the help");

        assert!(line.trim_start().starts_with("-p, --processor <PROCESSOR>"), "{line}");
        assert!(help.contains(&format!("[possible values: {}]", PROCESSORS.join(", "))), "{help}");
        for name in FOREIGN_PROCESSORS {
            assert!(!help.contains(name), "{name} is in the help: {help}");
        }
    }

    #[test]
    fn every_processor_is_the_provider_of_the_same_name() {
        use opai::ExecutionProvider;

        let names: Vec<_> = Processor::value_variants().iter().map(|processor| processor.name()).collect();
        assert_eq!(names, PROCESSORS);

        for processor in Processor::value_variants() {
            let provider = ExecutionProvider::from(*processor);

            assert_eq!(provider.as_str().to_lowercase(), processor.name(), "{processor:?}");
            assert_eq!(Processor::from_provider(provider), Some(*processor));
        }
        for name in FOREIGN_PROCESSORS {
            let provider: ExecutionProvider = name.parse().expect("the library publishes it");
            assert_eq!(Processor::from_provider(provider), None, "{name}");
        }
    }

    /// Every `opai …` command in `docs/CLI_REFERENCE.md`, from its code blocks and its inline code, without trailing
    /// comments or the environment variables set before it.
    fn documented_commands() -> Vec<String> {
        let reference = include_str!("../../../docs/CLI_REFERENCE.md");
        let mut commands = Vec::new();
        let mut fenced = false;

        for line in reference.lines() {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                continue;
            }

            let candidates: Vec<&str> =
                if fenced { vec![line.trim()] } else { line.split('`').skip(1).step_by(2).collect() };
            for candidate in candidates {
                let mut command = candidate.trim_start_matches("NO_COLOR=1 ");
                // Autopilot's model variables, as many as are set before the command.
                while let Some(rest) = command.strip_prefix("OPAI_AP_") {
                    command = rest.split_once(' ').map_or("", |(_, rest)| rest);
                }
                let command = command.split(" #").next().unwrap_or_default().trim();
                if command.starts_with("opai ") {
                    commands.push(command.to_owned());
                }
            }
        }

        commands
    }

    #[test]
    fn every_example_in_the_reference_parses() {
        // The synopsis, and the commands the reference shows failing or ending the parse on purpose.
        let not_runs = ["opai <INPUT>...", "opai --up photo.jpg", "opai --help", "opai --version"];

        let commands = documented_commands();
        assert!(commands.len() > 20, "the reference's examples were not found: {commands:?}");

        for command in commands {
            if not_runs.iter().any(|prefix| command.starts_with(prefix)) {
                continue;
            }
            if let Err(error) = parse(&command) {
                panic!("`{command}` does not parse: {error}");
            }
        }
    }

    #[test]
    fn verbose_is_off_unless_asked_for_and_works_with_autopilot() {
        assert!(!parse("opai photo.jpg --up").expect("parses").verbose);
        assert!(parse("opai photo.jpg --up --verbose").expect("parses").verbose);

        let args = parse("opai photo.jpg --ap --verbose").expect("parses");
        assert!(args.autopilot && args.verbose);
    }

    #[test]
    fn the_version_is_the_librarys() {
        let error = parse("opai --version").expect_err("--version ends the parse");

        assert_eq!(error.kind(), ErrorKind::DisplayVersion);
        assert_eq!(error.to_string().trim_end(), format!("Open Photo AI - CLI {}", opai::version()));

        let error = parse("opai -v").expect_err("-v ends the parse");

        assert_eq!(error.kind(), ErrorKind::DisplayVersion);
    }
}
