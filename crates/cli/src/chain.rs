//! The flags, turned into the chain the library runs.
//!
//! The terminal's own vocabulary — `hd`, `sd`, `--up`, `--fr` — mapped onto `opai`'s constructors, in the same way the
//! GUI maps its window's request onto them. The scale ladder is the library's [`opai::suggested_scale`], not restated.

use opai::{FaceRecovery, Faces, Fidelity, FloatPrecision, Operation, OsakaPrecision, Upscale};

use crate::args::{Args, FaceModel, Quality, UpscaleModel};

/// What a run will do, and what the user should be told about it before it starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The chain, in a fixed order. The library puts it into apply order anyway, so the flags' order cannot matter.
    pub operations: Vec<Operation>,
    /// Lines to show the user about what was planned, such as an upscale left out.
    pub notes: Vec<String>,
}

/// The chain `args` asks for over a `width` x `height` photograph.
///
/// An upscale with no `--scale` runs at the factor the photograph needs, and is left out with a note where it needs
/// none. Every face recovery restores nobody until the faces are found and handed to it.
pub fn plan(args: &Args, width: u32, height: u32) -> Plan {
    let mut operations = Vec::new();
    let mut notes = Vec::new();

    if let Some(model) = args.face_recovery {
        let precision = float_precision(args.quality);

        // Maximum fidelity: as close to the original face as the model goes, which is the library's own default.
        operations.push(match model {
            FaceModel::Athens => FaceRecovery::athens(precision, Faces::empty(), Fidelity::MAXIMUM),
            FaceModel::Santorini => FaceRecovery::santorini(precision, Faces::empty()),
        });
    }

    if let Some(model) = args.upscale {
        match args.scale.or_else(|| opai::suggested_scale(width, height)) {
            Some(scale) => operations.push(match model {
                UpscaleModel::Kyoto => Upscale::kyoto(float_precision(args.quality), scale),
                UpscaleModel::Osaka => Upscale::osaka(osaka_precision(args.quality), scale),
                UpscaleModel::Tokyo => Upscale::tokyo(float_precision(args.quality), scale),
                UpscaleModel::Saitama => Upscale::saitama(float_precision(args.quality), scale),
            }),
            None => notes.push(format!(
                "the upscale was skipped: the photograph is already large enough ({width}x{height}); \
                 pass --scale to upscale it anyway"
            )),
        }
    }

    Plan { operations, notes }
}

/// The build `quality` names for every model published in FP32 and FP16.
const fn float_precision(quality: Quality) -> FloatPrecision {
    match quality {
        Quality::Hd => FloatPrecision::Fp32,
        Quality::Sd => FloatPrecision::Fp16,
    }
}

/// The build `quality` names for Osaka, which has no FP32 build: its two are FP16 and INT8.
const fn osaka_precision(quality: Quality) -> OsakaPrecision {
    match quality {
        Quality::Hd => OsakaPrecision::Fp16,
        Quality::Sd => OsakaPrecision::Int8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::parse_from;
    use opai::Scale;

    fn args(line: &str) -> Args {
        parse_from(line.split_whitespace()).unwrap_or_else(|error| panic!("{line}: {error}"))
    }

    fn scale(factor: f64) -> Scale {
        Scale::new(factor).expect("in range")
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

        for (flags, expected) in rows {
            let plan = plan(&args(&format!("opai photo.jpg {flags} -o out.png")), 1000, 800);
            assert_eq!(plan.operations, vec![expected], "{flags}");
        }
    }

    #[test]
    fn the_default_quality_runs_tokyo_at_fp32() {
        let plan = plan(&args("opai photo.jpg --up -o out.png"), 1000, 800);

        assert_eq!(plan.operations, vec![Upscale::tokyo(FloatPrecision::Fp32, scale(4.0))]);
    }

    #[test]
    fn the_scale_is_what_the_photograph_needs() {
        let up = args("opai photo.jpg --up -o out.png");

        let small = plan(&up, 1000, 800);
        assert_eq!(small.operations, vec![Upscale::tokyo(FloatPrecision::Fp32, scale(4.0))]);
        assert!(small.notes.is_empty());

        let medium = plan(&up, 2000, 1500);
        assert_eq!(medium.operations, vec![Upscale::tokyo(FloatPrecision::Fp32, scale(2.0))]);
        assert!(medium.notes.is_empty());

        let large = plan(&up, 4000, 3000);
        assert!(large.operations.is_empty(), "a 12 MP photograph was upscaled: {:?}", large.operations);
        assert_eq!(large.notes.len(), 1);
        assert!(large.notes[0].contains("already large enough"), "{:?}", large.notes);
    }

    #[test]
    fn a_skipped_upscale_leaves_the_face_recovery() {
        let plan = plan(&args("opai photo.jpg --up --fr -o out.png"), 4000, 3000);

        assert_eq!(
            plan.operations,
            vec![FaceRecovery::athens(FloatPrecision::Fp32, Faces::empty(), Fidelity::MAXIMUM)]
        );
        assert_eq!(plan.notes.len(), 1);
    }

    #[test]
    fn an_explicit_scale_is_always_honoured() {
        let plan = plan(&args("opai photo.jpg --up --scale 2 -o out.png"), 4000, 3000);

        assert_eq!(plan.operations, vec![Upscale::tokyo(FloatPrecision::Fp32, scale(2.0))]);
        assert!(plan.notes.is_empty());
    }

    #[test]
    fn the_flags_order_does_not_change_the_chain() {
        let up_first = plan(&args("opai photo.jpg --up saitama --fr santorini -q sd -o out.png"), 1000, 800);
        let fr_first = plan(&args("opai photo.jpg --fr santorini --up saitama -q sd -o out.png"), 1000, 800);

        assert_eq!(up_first, fr_first);
        assert_eq!(up_first.operations.len(), 2);
    }
}
