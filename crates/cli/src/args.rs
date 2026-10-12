//! The command line: what `opai` accepts, its defaults, and the checks clap can make before anything is touched.

use std::ffi::OsString;
use std::path::PathBuf;

use clap::{CommandFactory, Parser, ValueEnum};
use opai::Scale;

/// Enhance one photograph with Open Photo AI.
///
/// Face recovery runs before an upscale whatever order the flags are given in. The output's format is the one its
/// extension names, and an existing file there is overwritten.
#[derive(Debug, Parser)]
#[command(name = "opai", version = opai::version(), override_usage = "opai <INPUT> [OPTIONS] --output <FILE>")]
pub struct Args {
    // Positional and first, so a bare `--up` or `--fr` is never handed the input as its model name. A user who writes
    // `opai --up photo.jpg` is told `photo.jpg` is not a model, which is clear and costs nothing.
    /// The photograph to enhance.
    #[arg(value_name = "INPUT")]
    pub input: PathBuf,

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
    if args.upscale.is_none() && args.face_recovery.is_none() {
        return Err(Args::command().error(
            clap::error::ErrorKind::MissingRequiredArgument,
            "at least one enhancement is required: --upscale (--up) or --face-recovery (--fr)",
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
        assert!(error.to_string().contains("at least one enhancement is required"), "{error}");
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
