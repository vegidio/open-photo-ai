//! What the crate root offers, checked from outside the crate.
//!
//! An integration test rather than a unit one on purpose: it can only see the public surface, so a type that stopped
//! being exported fails to compile here rather than going unnoticed until a front end is written against it.
//!
//! The fixtures are the crate's own, pulled in by path rather than retyped. `image/test_support.rs` is a
//! `cfg(test)` module, so it cannot be reached through `opai::` from out here — but it names nothing from its own
//! crate, only `std`, `image` and `rust-sak`, so compiling the same file as a module of this test crate gives one
//! DNG builder instead of two. Only part of it is used here, hence the `dead_code` allowance.

#[allow(dead_code)]
#[path = "../src/image/test_support.rs"]
mod test_support;

use test_support::{DNG_MAKE, DNG_MODEL, write_fixture, written_dng};

use opai::{
    ArtifactId, Bias, ColorBalance, ColorBalanceParams, ColorBalanceVariant, Colorization, ColorizationVariant,
    Confidence, Denoise, DenoiseParams, DenoiseVariant, Dependency, Detection, DetectionOutput, DetectionVariant,
    EncodeOptions, ExecutionProvider, Face, FaceRecovery, FaceRecoveryParams, FaceRecoveryVariant, Faces, Family,
    FamilyEntry, FloatPrecision, GraphRole, GraphSet, ImageError, ImageFormat, ImageInfo, ImageIoError, InitError,
    LightAdjustment, LightAdjustmentParams, LightAdjustmentVariant, OnProgress, Opai, Operation, OsakaPrecision,
    ParameterEntry, Pass, Phase, Picture, Point, Precision, Progress, RangeError, RawFormat, RawImageInfo, Rect,
    Resolution, Scale, Sharpen, SharpenParams, SharpenVariant, Strength, SupportedProviders, UnknownRole, Upscale,
    UpscaleParams, UpscaleVariant, VariantEntry, catalogue, prepare_library_path,
};
// What this slice added: the fidelity Athens carries, the carrier for the run path that produces no image, the kind
// a published parameter reports, and the value a progress report names a run by — each reached only through the
// crate root, as a front end reaches it.
use opai::{Analysis, Fidelity, ParameterKind, Subject};
// What this slice added: the catalogue's inverse - the refusal a lookup or a build answers with, and the per-run
// values a build is handed - reached only through the crate root, as a front end reaches them.
use opai::{BuildError, ParameterValues};
// The vocabulary a run is asked for and reports in, and — added by the cache slice — what a front end reads to find
// out what is behind the store a run may use.
use opai::{
    CacheMode, CancellationToken, InferenceError, InferenceProgress, OnInference, OutputDepth, ProcessOptions, Stage,
    UnsupportedReason,
};
// What this slice added: the bundle a run returns, the report inside it, the declaration a process makes about the
// model files already on disk, and the release every consumer can now ask for.
use opai::{Enhanced, ModelTrust, ProviderReport};
// What this slice added: the second inference entry point's options and result, and the trait that binds an
// operation to the type it produces — the surface a caller that wants faces rather than pixels reaches.
use opai::{DataOperation, ExecuteOptions, Executed, ProviderVerdict};
// What a binary has to name in order to initialize at all, and what it reads off a refusal. Added by the
// single-instance slice; the identity became a string and the bundle arrived with `refactor-init-options`.
use opai::{CLI, GUI, Holder, InitOptions, PERF};
// What this slice added: what an analysis of an untouched photograph concluded, and the one enhancement it
// concluded that photograph calls for — the surface a front end wiring its Autopilot toggle reaches.
use opai::{Suggestion, Suggestions};
// What a binary needs to get itself a log, and the name it resolves every directory from. Added by the logging
// slice — a front end calls `logging::init` before anything else it does, so this is the first thing in the crate's
// surface a `main` reaches.
use opai::{APP_NAME, LogError, logging};
// What this slice added: the row of the install plan a front end draws its component list from, and the callback it
// registers to be handed them — the surface a setup dialog reaches before the first byte moves.
use opai::{OnPlan, PlannedDependency};

/// A face, built from the crate root the way a front end that decoded a detection result would build one.
fn face_at(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Face {
    Face::new(
        Rect::new(Point::new(min_x, min_y), Point::new(max_x, max_y)),
        [
            Point::new(min_x + 10.0, min_y + 10.0),
            Point::new(max_x - 10.0, min_y + 10.0),
            Point::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0),
            Point::new(min_x + 12.0, max_y - 10.0),
            Point::new(max_x - 12.0, max_y - 10.0),
        ],
        Confidence::new(0.92).expect("0.92 is in range"),
    )
}

/// The exports this slice found in place, named so that removing or renaming one stops compiling. This slice is
/// additive: it changes none of them.
#[test]
fn the_existing_public_api_is_unchanged() {
    // Named through a function pointer, so a change to its signature — arguments, return type or its `unsafe` —
    // fails to compile here.
    let _: unsafe fn(&str) -> Result<Vec<std::path::PathBuf>, InitError> = prepare_library_path;

    // Named in a type position each, which is what a rename or a removal would break.
    let _: Option<Opai> = None;
    let _: Option<InitError> = None;
    let _: Option<SupportedProviders> = None;
    let _: Option<Progress> = None;
    let _: Option<Phase> = None;
    let _: Option<Dependency> = None;
    let _: Option<&OnProgress> = None;

    assert!(!opai::version().is_empty());
}

/// The report a front end builds its provider list from is serializable from outside this crate.
///
/// The bound rather than the JSON, because nothing public constructs one: `SupportedProviders::detect` is
/// `pub(crate)` and the struct is `#[non_exhaustive]`, so the only route to a value from here is an initialization
/// that installs things. What a front end depends on is that the type is reachable and crosses a process boundary as
/// itself, and that is what this pins — removing the derive stops it compiling. The four field names it lands as are
/// pinned where an instance exists, in `providers`' own tests.
#[test]
fn the_supported_providers_report_crosses_a_process_boundary() {
    fn serializable<T: serde::Serialize>() {}

    serializable::<SupportedProviders>();
}

/// Every type this slice adds, reached through the crate root the way a front end reaches it.
#[test]
fn the_model_vocabulary_is_reachable_from_the_crate_root() {
    let scale: Scale = Scale::new(2.5).expect("2.5x is in range");
    let variant: UpscaleVariant = UpscaleVariant::Kyoto(FloatPrecision::Fp32);
    let operation: Upscale = Upscale::new(variant, scale);

    assert_eq!(operation.display_name(), "Kyoto 2.5x (FP32)");
    assert_eq!(operation.precision(), Precision::Fp32);
    assert_eq!(operation.cache_tag(), "up-kyoto-fp32-s2.5");

    let params: UpscaleParams = operation.params();
    assert_eq!(params.scale, scale);

    let required: Vec<ArtifactId> = operation.required_artifacts();
    assert_eq!(required.iter().map(ArtifactId::as_str).collect::<Vec<_>>(), vec!["up_kyoto_4x_fp32"]);

    match operation.resolve() {
        Resolution::Passes(passes) => {
            let first: &Pass = passes.first().expect("a pass sequence is never empty");
            assert_eq!(first.scale, 4);
        }
        Resolution::Graphs(_) => panic!("a convolutional variant resolved to graphs"),
    }

    let osaka = Upscale::new(UpscaleVariant::Osaka(OsakaPrecision::Int8), scale);
    match osaka.resolve() {
        Resolution::Graphs(graphs) => {
            let graphs: GraphSet = graphs;
            assert_eq!(
                graphs.graph(GraphRole::Transformer).expect("Osaka declares a transformer").as_str(),
                "up_osaka_int8"
            );
        }
        Resolution::Passes(_) => panic!("a diffusion variant resolved to passes"),
    }

    // A convolutional variant declares no graphs at all, which is the reachable way to see the error type.
    let unknown: UnknownRole = operation.graph(GraphRole::Encoder).expect_err("Kyoto declares no graphs");
    assert_eq!(unknown.role, GraphRole::Encoder);
    assert_eq!(unknown.variant, "kyoto");
}

/// The catalogue, reached the way a front end populating a chooser reaches it.
#[test]
fn the_catalogue_is_reachable_from_the_crate_root() {
    let families: &'static [FamilyEntry] = catalogue();
    let upscale = families.iter().find(|entry| entry.family == Family::Upscale).expect("upscale is published");

    let variants: &Vec<VariantEntry> = &upscale.variants;
    assert_eq!(variants.len(), 4);

    // Read off the variant rather than the family, which is where a front end reads it: it draws controls for a
    // selected variant, never for a family.
    let kyoto: &VariantEntry = variants.iter().find(|entry| entry.codename == "kyoto").expect("Kyoto is published");
    let scale_parameter: &ParameterEntry = kyoto.parameters.first().expect("upscale takes a scale");
    assert_eq!(scale_parameter.name, "scale");

    let ParameterKind::Range { min, max, .. } = scale_parameter.kind else {
        panic!("a scale is a range a control offers, not {:?}", scale_parameter.kind);
    };
    assert_eq!((min, max), (1.0, 8.0));

    let refused: RangeError = Scale::new(12.0).expect_err("12x is out of range");
    assert_eq!((refused.min, refused.max), (min, max));
}

/// The two parameter types the five enhancement families take, reached the way a control configuring itself from the
/// catalogue would reach them.
#[test]
fn the_enhancement_parameters_are_reachable_from_the_crate_root() {
    let strength: Strength = Strength::new(0.5).expect("0.5 is in range");
    assert_eq!(strength.get(), 0.5);
    assert_eq!(strength.to_string(), "0.5");
    assert_eq!(Strength::clamped(4.0).get(), Strength::MAX);

    let bias: Bias = Bias::new(-0.35).expect("-0.35 is in range");
    assert_eq!(bias.get(), -0.35);
    assert_eq!(bias.to_string(), "-0.35");
    assert_eq!(Bias::clamped(-2.0).get(), Bias::MIN);

    // The rejecting constructor names the same range the catalogue publishes, which is what a flag reports.
    let refused: RangeError = Strength::new(4.0).expect_err("4 is out of range");
    assert_eq!((refused.parameter, refused.min, refused.max), ("strength", Strength::MIN, Strength::MAX));
    assert_eq!(Bias::new(2.0).expect_err("2 is out of range").parameter, "bias");
}

/// Each of the five enhancement operations, reached the way a front end building one would reach it.
///
/// Through the **per-variant constructors**, which is the rule every model now follows: name the model, give it what
/// it takes, and get back the carrier the library's entry point takes. No `Operation::Family(...)` wrapping appears
/// at any of these call sites, which is the whole point of the constructors returning the carrier.
#[test]
fn the_five_enhancement_families_are_reachable_from_the_crate_root() {
    let strength = Strength::new(0.5).expect("in range");
    let bias = Bias::new(-0.5).expect("in range");

    let denoise: Operation = Denoise::stockholm(FloatPrecision::Fp32, strength);
    assert_eq!(denoise.display_name(), "Stockholm (FP32)");
    assert_eq!(denoise.precision(), Precision::Fp32);
    assert_eq!(denoise.cache_tag(), "dn-stockholm-fp32-t0.5");
    let required: Vec<ArtifactId> = denoise.required_artifacts();
    assert_eq!(required.iter().map(ArtifactId::as_str).collect::<Vec<_>>(), vec!["dn_stockholm_fp32"]);

    // Every model of every family has one, named for the model. Collected into a chain with no conversion, which is
    // what `process` takes.
    let chain: Vec<Operation> = vec![
        Denoise::gothenburg(FloatPrecision::Fp16, strength),
        Denoise::malmo(FloatPrecision::Fp32, strength),
        Sharpen::moscow(FloatPrecision::Fp32, strength),
        Sharpen::stpetersburg(FloatPrecision::Fp16, strength),
        Sharpen::novgorod(FloatPrecision::Fp32, strength),
        Colorization::delhi(FloatPrecision::Fp32),
        Colorization::mumbai(FloatPrecision::Fp16),
        Colorization::jaipur(FloatPrecision::Fp32),
        LightAdjustment::paris(FloatPrecision::Fp16, bias),
        LightAdjustment::lyon(FloatPrecision::Fp32, bias),
        ColorBalance::rio(FloatPrecision::Fp16, bias),
        ColorBalance::saopaulo(FloatPrecision::Fp16, bias),
    ];
    assert_eq!(chain.len(), 12);
    assert!(chain.iter().all(|operation| !operation.required_artifacts().is_empty()));

    assert_eq!(Sharpen::moscow(FloatPrecision::Fp32, strength).display_name(), "Moscow (FP32)");
    assert_eq!(LightAdjustment::paris(FloatPrecision::Fp16, bias).cache_tag(), "la-paris-fp16-b-0.5");
    assert_eq!(ColorBalance::saopaulo(FloatPrecision::Fp16, bias).display_name(), "São Paulo (FP16)");
    assert_eq!(Colorization::delhi(FloatPrecision::Fp32).cache_tag(), "cl-delhi-fp32");

    // The typed value stays reachable by matching the carrier, which is where a params type is read.
    let Operation::Sharpen(sharpen) = Sharpen::moscow(FloatPrecision::Fp32, strength) else {
        panic!("Sharpen::moscow built another family");
    };
    let sharpen: Sharpen = sharpen;
    assert_eq!(sharpen.artifact().as_str(), "sh_moscow_fp32");
    let sharpen_params: SharpenParams = sharpen.params();
    assert_eq!(sharpen_params.strength, strength);

    let Operation::Denoise(typed) = denoise.clone() else {
        panic!("Denoise::stockholm built another family")
    };
    let denoise_params: DenoiseParams = typed.params();
    assert_eq!(denoise_params.strength, strength);

    let Operation::LightAdjustment(light) = LightAdjustment::paris(FloatPrecision::Fp16, bias) else {
        panic!("LightAdjustment::paris built another family")
    };
    let light_params: LightAdjustmentParams = light.params();
    assert_eq!(light_params.bias, bias);

    let Operation::ColorBalance(colour) = ColorBalance::saopaulo(FloatPrecision::Fp16, bias) else {
        panic!("ColorBalance::saopaulo built another family")
    };
    let colour_params: ColorBalanceParams = colour.params();
    assert_eq!(colour_params.bias, bias);

    // The one family built from a precision alone: there is no amount to pass and no params type to read back.
    let Operation::Colorization(colorization) = Colorization::delhi(FloatPrecision::Fp32) else {
        panic!("Colorization::delhi built another family")
    };
    let colorization: Colorization = colorization;
    assert_eq!(colorization.artifact().as_str(), "cl_delhi_fp32");

    // The per-run parameter is **inside** the identity: two strengths of one variant are two requests, which is
    // what a front end asking "did the user change anything?" needs. It was the reverse of this, to serve a registry
    // keyed on an operation that was never built — see `Operation`. Nothing is reloaded by their differing: both
    // name the same artifact, and that is what decides which weights are opened.
    let louder = Denoise::stockholm(FloatPrecision::Fp32, Strength::new(2.5).expect("in range"));
    assert_ne!(denoise, louder);
    assert_ne!(denoise.cache_tag(), louder.cache_tag());
    assert_eq!(denoise.required_artifacts(), louder.required_artifacts());
}

/// What this slice adds to the crate's surface: the one refusal a colour balance run can make that no other
/// family's can.
///
/// The mechanical sign that the slice is visible from outside the crate, and the reason it is recorded here rather
/// than absorbed: [`InferenceError`] is `#[non_exhaustive]`, so a front end matching on one already carries a
/// catch-all arm and nothing downstream breaks — but a variant added and never named from out here is a public
/// addition nobody checked the spelling of.
///
/// The fit's own ridge is supposed to make this unreachable, which is exactly why the message has to be right: the
/// first time anyone reads it will be the first time it happens.
#[test]
fn the_refusal_a_colour_balance_fit_makes_is_reachable_from_the_crate_root() {
    let bias = Bias::new(0.5).expect("in range");
    let rio = ColorBalance::rio(FloatPrecision::Fp16, bias);

    let singular = InferenceError::ColourMapping { operation: rio.display_name(), column: 7 };

    assert_eq!(
        singular.to_string(),
        "Rio (FP16) could not fit a colour mapping: the system is singular at column 7",
        "the refusal does not name the operation a user would recognise, or the column a report would act on"
    );

    // Matched from out here, which is what a front end telling one failure from another does. The catch-all is
    // mandatory on a `#[non_exhaustive]` enum, so what this pins is that the variant and its two fields can be
    // named at all.
    match singular {
        InferenceError::ColourMapping { operation, column } => {
            assert_eq!(operation, "Rio (FP16)");
            assert_eq!(column, 7);
        }
        other => panic!("a singular fit was matched as {other:?}"),
    }
}

/// The face vocabulary, reached the way a front end drawing a face-box overlay and a CLI reporting a face count
/// reach it.
#[test]
fn the_face_vocabulary_is_reachable_from_the_crate_root() {
    let face: Face = face_at(12.34, 56.78, 90.12, 34.56);
    let bounding_box: Rect = face.bounding_box();
    let corner: Point = bounding_box.min;

    assert_eq!((corner.x, corner.y), (12.34, 56.78));
    assert_eq!(face.landmarks().len(), Face::LANDMARKS);

    let confidence: Confidence = face.confidence();
    assert_eq!(confidence.get(), 0.92);

    // The rejecting constructor names the same range the type publishes, as every other bounded value here does.
    let refused: RangeError = Confidence::new(1.5).expect_err("1.5 is out of range");
    assert_eq!((refused.parameter, refused.min, refused.max), ("confidence", 0.0, 1.0));

    // An empty set is a value rather than an error, and what a detection run produces is this rather than an image.
    let found: DetectionOutput = Faces::new([face]);
    assert_eq!(found.len(), 1);
    assert_eq!(found.as_slice(), &[face]);
    assert!(Faces::empty().is_empty());
}

/// Detection and face recovery, reached the way a front end that pre-detects and then restores would reach them.
#[test]
fn the_two_face_families_are_reachable_from_the_crate_root() {
    // Detection's constructor returns an `Analysis`, because its result is a set of faces rather than an image — so
    // naming the model is what decides which of the two run paths it is on, and a caller is never asked.
    let detection: Analysis = Detection::newyork(FloatPrecision::Fp32);
    assert_eq!(detection.display_name(), "New York (FP32)");
    assert_eq!(detection.precision(), Precision::Fp32);
    assert_eq!(detection.cache_tag(), "dt-newyork-fp32");
    assert_eq!(detection.family(), Family::Detection);
    assert_eq!(
        detection.required_artifacts().iter().map(ArtifactId::as_str).collect::<Vec<_>>(),
        vec!["dt_newyork_fp32"]
    );

    // The typed value is reachable by matching the carrier, exactly as an enhancement's is.
    let Analysis::Detection(typed) = detection;
    let typed: Detection = typed;
    assert_eq!(typed.artifact().as_str(), "dt_newyork_fp32");

    let faces: Faces = Faces::new([face_at(12.34, 56.78, 90.12, 34.56), face_at(200.0, 100.5, 260.25, 170.75)]);
    let fidelity: Fidelity = Fidelity::new(0.5).expect("0.5 is in range");

    // Athens takes a fidelity beside the faces; Santorini has nowhere to put one, and there is no argument for a
    // caller to leave out or to supply wrongly.
    let recovery: Operation = FaceRecovery::athens(FloatPrecision::Fp16, faces.clone(), fidelity);
    assert_eq!(recovery.display_name(), "Athens (FP16)");
    assert_eq!(
        recovery.cache_tag(),
        "fr-athens-fp16-w0.500-f12.34,56.78,90.12,34.56;200.00,100.50,260.25,170.75;"
    );

    // Its own graph and nothing else: the faces arrived already detected, so the detection artifact is the caller's
    // to ask for — which is what the operation above did.
    let required: Vec<ArtifactId> = recovery.required_artifacts();
    assert_eq!(required.iter().map(ArtifactId::as_str).collect::<Vec<_>>(), vec!["fr_athens_fp16"]);

    let Operation::FaceRecovery(athens) = recovery.clone() else {
        panic!("Athens built another family")
    };
    let athens: FaceRecovery = athens;
    assert_eq!(athens.faces(), &faces);
    assert_eq!(athens.fidelity(), Some(fidelity));

    let params: FaceRecoveryParams = athens.params();
    assert_eq!(params.faces, faces);
    assert_eq!(params.fidelity, Some(fidelity));

    // Santorini reports no fidelity, and its tag carries no segment for one — so two of its runs over one selection
    // cannot be made to miss the cache by a value neither of them has. Now that the model runs, that is the
    // requirement's other half rather than a property of a value nothing consumed: one selection restored twice at
    // one set of settings is **one** result, and the second is served the first's.
    let santorini: Operation = FaceRecovery::santorini(FloatPrecision::Fp32, faces.clone());
    let Operation::FaceRecovery(typed) = santorini.clone() else {
        panic!("Santorini built another family")
    };
    assert_eq!(typed.fidelity(), None);
    assert_eq!(typed.params().fidelity, None, "Santorini published a fidelity into what a run takes");
    assert_eq!(typed.artifact().as_str(), "fr_santorini_fp32");
    assert!(!santorini.cache_tag().contains("-w"), "{}", santorini.cache_tag());
    assert_eq!(
        santorini.cache_tag(),
        FaceRecovery::santorini(FloatPrecision::Fp32, faces.clone()).cache_tag(),
        "one Santorini selection built twice reported two tags, so the second run would not be served the first"
    );

    // The two models are two requests and two images over one selection, which is what makes choosing between them
    // a choice a user can make and then undo.
    assert_ne!(santorini, recovery);
    assert_ne!(santorini.cache_tag(), recovery.cache_tag());
    assert_ne!(santorini.required_artifacts(), recovery.required_artifacts());

    // The selection and the fidelity are both **inside** the identity, so a front end comparing two operations is
    // told whether the user changed anything. Both are inside the cache tag too, which is the separate question of
    // whether the two runs produce the same image.
    let fewer = FaceRecovery::athens(FloatPrecision::Fp16, Faces::empty(), fidelity);
    assert_ne!(recovery, fewer);
    assert_ne!(recovery.cache_tag(), fewer.cache_tag());
    assert_eq!(fewer.cache_tag(), "fr-athens-fp16-w0.500-f0");

    let keener = FaceRecovery::athens(FloatPrecision::Fp16, faces, Fidelity::new(1.0).expect("in range"));
    assert_ne!(recovery, keener);
    assert_ne!(recovery.cache_tag(), keener.cache_tag());
    assert_eq!(recovery.required_artifacts(), keener.required_artifacts(), "a fidelity changed the weights");

    // The range is the one the catalogue publishes, refused by the same constructor a control reads the bounds from.
    let refused: RangeError = Fidelity::new(1.5).expect_err("1.5 is out of range");
    assert_eq!((refused.parameter, refused.min, refused.max), ("fidelity", Fidelity::MIN, Fidelity::MAX));

    // What a front end reads off an operation it cannot run. Every published variant of every family runs, so the one
    // reason left is an incomplete declaration, which no shipped variant produces; **which** operation would answer it
    // is not visible from here, because the seam that resolves one is `pub(crate)`. What this pins is the rendering a
    // front end shows, and that a caller can match the reason, since `#[non_exhaustive]` makes the catch-all arm
    // mandatory.
    let incomplete: UnsupportedReason = UnsupportedReason::IncompleteGraphSet { missing: "decoder" };
    match incomplete {
        UnsupportedReason::IncompleteGraphSet { missing } => assert_eq!(missing, "decoder"),
        _ => panic!("the incomplete declaration was matched as something else"),
    }

    let osaka = Upscale::osaka(OsakaPrecision::Fp16, Scale::new(4.0).expect("in range"));
    let unsupported = InferenceError::Unsupported { operation: osaka.display_name(), reason: incomplete };
    assert_eq!(
        unsupported.to_string(),
        format!("no pipeline for {}: this variant does not declare its decoder graph", osaka.display_name()),
        "a refusal does not name the operation a user would recognise"
    );
}

/// The catalogue as a front end offering all eight families reads it: every chooser and every control configured
/// from this alone.
#[test]
fn the_catalogue_publishes_all_eight_families_to_the_crate_root() {
    let families: &'static [FamilyEntry] = catalogue();

    assert_eq!(
        families.iter().map(|entry| entry.family).collect::<Vec<_>>(),
        vec![
            Family::Denoise,
            Family::Sharpen,
            Family::LightAdjustment,
            Family::ColorBalance,
            Family::Colorization,
            Family::Upscale,
            Family::Detection,
            Family::FaceRecovery,
        ]
    );

    let entry = |family: Family| families.iter().find(|entry| entry.family == family).expect("published");

    // `Malmö` carries its accent: the label is what a front end shows, and nothing is composed from it. Its
    // codename stays `malmo`, which is the half that names the artifact - asserted below.
    assert_eq!(
        entry(Family::Denoise).variants.iter().map(|variant| variant.label).collect::<Vec<_>>(),
        vec!["Stockholm", "Gothenburg", "Malmö"]
    );
    assert_eq!(
        entry(Family::Denoise).variants.iter().map(|variant| variant.codename).collect::<Vec<_>>(),
        vec!["stockholm", "gothenburg", "malmo"]
    );

    // Parameters are published **per variant**, which is where a front end reads them: it draws controls for a
    // selected variant, never for a family.
    let published = |family: Family, codename: &str| -> &'static [ParameterEntry] {
        entry(family)
            .variants
            .iter()
            .find(|variant| variant.codename == codename)
            .unwrap_or_else(|| panic!("{family:?} publishes {codename}"))
            .parameters
    };
    let range = |family: Family, codename: &str, name: &str| -> (f64, f64) {
        let parameter = published(family, codename)
            .iter()
            .find(|parameter| parameter.name == name)
            .unwrap_or_else(|| panic!("{codename} publishes {name}"));

        match parameter.kind {
            ParameterKind::Range { min, max, .. } => (min, max),
            ParameterKind::Faces => panic!("{name} is not a range"),
        }
    };

    assert_eq!(range(Family::Sharpen, "moscow", "strength"), (0.0, 3.0));
    assert_eq!(range(Family::ColorBalance, "rio", "bias"), (-1.0, 1.0));

    // Listed, with its variants, and with nothing to configure — so a front end draws no control rather than having
    // to know on its own that there is none.
    assert_eq!(entry(Family::Colorization).variants.len(), 3);
    assert!(entry(Family::Colorization).variants.iter().all(|variant| variant.parameters.is_empty()));

    // Which model detects faces is model vocabulary, so a front end that pre-detects reads it here rather than
    // naming New York itself.
    let detection: &VariantEntry = entry(Family::Detection).variants.first().expect("detection publishes a variant");
    assert_eq!((detection.codename, detection.label), ("newyork", "New York"));
    assert_eq!(detection.precisions, vec![Precision::Fp32, Precision::Fp16]);
    assert!(detection.parameters.is_empty(), "a detection run is its variant and precision alone");

    assert_eq!(
        entry(Family::FaceRecovery).variants.iter().map(|variant| variant.label).collect::<Vec<_>>(),
        vec!["Athens", "Santorini"]
    );

    // Face recovery and colorization are told apart, which they were not when both published an empty list: only
    // colorization's was true. Both face-recovery variants report the faces they cannot be built without, as a kind
    // that is not a control — so a front end learns here that choosing one means running a detection first.
    for codename in ["athens", "santorini"] {
        assert!(
            published(Family::FaceRecovery, codename)
                .iter()
                .any(|parameter| parameter.name == "faces" && parameter.kind == ParameterKind::Faces),
            "{codename} did not publish the faces it requires"
        );
    }

    // And Athens publishes a fidelity its sibling does not, which is the whole reason parameters moved onto the
    // variant: a control drawn from the catalogue is drawn for one and not the other.
    assert_eq!(range(Family::FaceRecovery, "athens", "fidelity"), (0.0, 1.0));
    assert!(
        !published(Family::FaceRecovery, "santorini")
            .iter()
            .any(|parameter| parameter.name == "fidelity"),
        "Santorini published a fidelity it has nowhere to put"
    );
}

/// The seam over the eight families, reached the way a front end that keeps a list of operations to apply reaches it:
/// one type it can store, hand about and dispatch on without knowing which family it is holding.
#[test]
fn the_operation_seam_is_reachable_from_the_crate_root() {
    let strength = Strength::new(0.5).unwrap();
    let carried: Operation = Denoise::stockholm(FloatPrecision::Fp32, strength);
    let Operation::Denoise(denoise) = carried.clone() else {
        panic!("Denoise::stockholm built another family")
    };
    let denoise: Denoise = denoise;

    // Everything the typed operation reports, reported by the value that does not say which family it is.
    assert_eq!(carried.family(), Family::Denoise);
    assert_eq!(carried.precision(), denoise.precision());
    assert_eq!(carried.display_name(), denoise.display_name());
    assert_eq!(carried.required_artifacts(), denoise.required_artifacts());
    assert_eq!(carried.cache_tag(), denoise.cache_tag());

    // A queue of operations of different families, which is the shape the seam exists for: one list, one type, and
    // the family asked of each entry rather than known by whoever built the list. Built with no conversion at any
    // element, because each constructor already returns the carrier.
    //
    // **No detection here, and it is not an omission.** A detection run produces a set of faces rather than an
    // image, so it is carried in `Analysis` and cannot be put in this list at all — the request is unwritable
    // rather than accepted and then refused. See `Opai::execute`, which is where that operation belongs.
    let queue: Vec<Operation> = vec![
        carried.clone(),
        FaceRecovery::athens(FloatPrecision::Fp16, Faces::empty(), Fidelity::new(1.0).unwrap()),
        FaceRecovery::santorini(FloatPrecision::Fp32, Faces::empty()),
        Upscale::osaka(OsakaPrecision::Int8, Scale::new(2.5).unwrap()),
    ];

    // What whoever downloads a queue's models asks, of every entry alike — one file for three of these and three for
    // the Osaka run, without the caller knowing which entry is which. Both face-recovery variants sit in the queue
    // now that both run, and each names one graph and no detector: the faces reached the operation already
    // detected.
    let wanted: Vec<String> = queue
        .iter()
        .flat_map(Operation::required_artifacts)
        .map(|artifact| artifact.as_str().to_owned())
        .collect();
    assert_eq!(
        wanted,
        vec![
            "dn_stockholm_fp32",
            "fr_athens_fp16",
            "fr_santorini_fp32",
            "up_osaka_vae_encoder_fp16",
            "up_osaka_int8",
            "up_osaka_vae_decoder_fp16",
        ]
    );

    // Matched on to reach what only one family answers, which is how a question this type does not forward is asked.
    match &queue[3] {
        Operation::Upscale(operation) => assert_eq!(operation.params().scale, Scale::new(2.5).unwrap()),
        other => panic!("the queue's fourth entry is an upscale, not {other:?}"),
    }

    // The other carrier, for the other path. Both report the same five questions, which is what lets a consumer that
    // only reports on a run be written once over either — see `Subject`.
    let analysis: Analysis = Detection::newyork(FloatPrecision::Fp32);
    let subject: Subject = Subject::Analysis(analysis);
    assert_eq!(subject.family(), Family::Detection);
    assert_eq!(subject.display_name(), analysis.display_name());
    assert_eq!(subject.cache_tag(), analysis.cache_tag());
    assert_eq!(subject.required_artifacts(), analysis.required_artifacts());

    let subject: Subject = Subject::Enhancement(carried.clone());
    assert_eq!(subject.family(), Family::Denoise);
    assert_eq!(subject.display_name(), carried.display_name());
    assert_eq!(subject.cache_tag(), carried.cache_tag());
}

/// The catalogue's inverse, reached the way a front end turning a chooser's selection back into something runnable
/// reaches it: with the row the catalogue itself published, and nothing of its own.
///
/// **The inverse is the library's now.** It was a match written out at this call site - one arm per model, calling
/// the constructor for the variant `from_codename` resolved - which is what a front end had to write and therefore a
/// second record of which models exist. What replaces it is [`VariantEntry::published`] and [`VariantEntry::build`]:
/// the lookup hands back the row, and the row builds the run it names, so the check and the construction read the
/// same fields and a model the library publishes is runnable by being published.
///
/// A [`Subject`] comes back rather than an [`Operation`], because the eight families are carried in two values and
/// this traversal crosses both.
#[test]
fn a_published_catalogue_row_can_be_turned_back_into_an_operation_from_the_crate_root() {
    /// The values a control built from one row would hand back: every published range at its midpoint, and the
    /// faces a detection run would have supplied where the row says one is needed.
    ///
    /// Read off the row, which is the whole of what a front end has: it never names a model, never spells a
    /// precision in a model's own domain, and never asks which models admit which precisions.
    fn values_for(variant: &VariantEntry) -> ParameterValues {
        let mut values = ParameterValues::new();

        for parameter in variant.parameters {
            values = match parameter.kind {
                ParameterKind::Faces => values.with_faces(Faces::new([face_at(12.34, 56.78, 90.12, 34.56)])),
                ParameterKind::Range { min, max, .. } => {
                    let midpoint = f64::midpoint(min, max);

                    // Matched against the constants the library publishes the names under, not against literals
                    // of this test's own: a caller reading the catalogue does not restate the vocabulary, and that
                    // is the property being asserted here rather than one to make an exception to.
                    match parameter.name {
                        Scale::NAME => values.with_scale(Scale::clamped(midpoint)),
                        Strength::NAME => values.with_strength(Strength::clamped(midpoint)),
                        Bias::NAME => values.with_bias(Bias::clamped(midpoint)),
                        Fidelity::NAME => values.with_fidelity(Fidelity::clamped(midpoint)),
                        other => {
                            panic!("{} publishes a range this test supplies no value for: {other}", variant.codename)
                        }
                    }
                }
            };
        }

        values
    }

    let families: &'static [FamilyEntry] = catalogue();

    for entry in families {
        for variant in &entry.variants {
            assert_eq!(variant.family, entry.family, "{} is filed under another family", variant.codename);

            for &precision in variant.precisions {
                let run: Subject = variant.build(precision, &values_for(variant)).unwrap_or_else(|error| {
                    panic!(
                        "{:?}'s {} at {precision} is published but builds nothing: {error}",
                        entry.family, variant.codename
                    )
                });

                assert_eq!(run.family(), entry.family);
                assert_eq!(run.precision(), precision);
                assert!(!run.required_artifacts().is_empty());
            }
        }
    }

    // The two ways a request can be wrong, and there is no third: what a caller supplies beyond the codename and
    // the precision is values that were bounded when they were built.
    let osaka: &VariantEntry = VariantEntry::published(Family::Upscale, "osaka").expect("upscale publishes Osaka");
    let refused: BuildError = osaka
        .build(Precision::Fp32, &values_for(osaka))
        .expect_err("no FP32 build of the diffusion transformer was ever published");
    assert_eq!(refused, BuildError::UnpublishedPrecision { codename: "osaka", precision: Precision::Fp32 });
    assert_eq!(refused.to_string(), "`osaka` is not published at fp32");

    let newyork = VariantEntry::published(Family::Detection, "newyork").expect("detection publishes New York");
    assert!(newyork.build(Precision::Int8, &ParameterValues::new()).is_err());

    // A codename no family publishes, and one another family publishes than the one it was asked of.
    for codename in ["helsinki", "stockholm"] {
        assert_eq!(
            VariantEntry::published(Family::Upscale, codename).expect_err("upscale publishes neither"),
            BuildError::UnknownModel { family: Family::Upscale, codename: codename.to_string() }
        );
    }
    assert_eq!(
        VariantEntry::published(Family::Upscale, "helsinki").expect_err("refused").to_string(),
        "Upscale publishes no model called `helsinki`"
    );

    // A caller that sets nothing gets the neutral run rather than a refusal it would have to handle: the values
    // are total, which is what leaves the refusal with exactly two arms.
    assert_eq!(ParameterValues::new(), ParameterValues::default());
    assert_eq!(
        VariantEntry::published(Family::Upscale, "kyoto")
            .expect("upscale publishes Kyoto")
            .build(Precision::Fp16, &ParameterValues::new().with_scale(Scale::clamped(4.0)))
            .expect("Kyoto is published at FP16"),
        Subject::Enhancement(Upscale::kyoto(FloatPrecision::Fp16, Scale::clamped(4.0)))
    );
}

/// The per-family inverse the one above is built from, named once for each of the eight families so that removing or
/// renaming any of them stops compiling here.
#[test]
fn every_familys_codename_inverse_is_reachable_from_the_crate_root() {
    assert_eq!(
        DenoiseVariant::from_codename("stockholm", Precision::Fp32),
        Some(DenoiseVariant::Stockholm(FloatPrecision::Fp32))
    );
    assert_eq!(
        SharpenVariant::from_codename("moscow", Precision::Fp16),
        Some(SharpenVariant::Moscow(FloatPrecision::Fp16))
    );
    assert_eq!(
        LightAdjustmentVariant::from_codename("lyon", Precision::Fp32),
        Some(LightAdjustmentVariant::Lyon(FloatPrecision::Fp32))
    );
    assert_eq!(
        ColorBalanceVariant::from_codename("saopaulo", Precision::Fp16),
        Some(ColorBalanceVariant::SaoPaulo(FloatPrecision::Fp16))
    );
    assert_eq!(
        ColorizationVariant::from_codename("jaipur", Precision::Fp32),
        Some(ColorizationVariant::Jaipur(FloatPrecision::Fp32))
    );
    assert_eq!(
        DetectionVariant::from_codename("newyork", Precision::Fp16),
        Some(DetectionVariant::NewYork(FloatPrecision::Fp16))
    );
    assert_eq!(
        FaceRecoveryVariant::from_codename("santorini", Precision::Fp32),
        Some(FaceRecoveryVariant::Santorini(FloatPrecision::Fp32))
    );
    assert_eq!(
        UpscaleVariant::from_codename("osaka", Precision::Int8),
        Some(UpscaleVariant::Osaka(OsakaPrecision::Int8))
    );

    // Osaka has no FP32 build and the float-only variants have no INT8 one, so neither pairing names a variant.
    assert_eq!(UpscaleVariant::from_codename("osaka", Precision::Fp32), None);
    assert_eq!(DenoiseVariant::from_codename("stockholm", Precision::Int8), None);
    // A codename published by another family names nothing here, which is the pairing the eight types exist to stop.
    assert_eq!(DenoiseVariant::from_codename("moscow", Precision::Fp32), None);
}

/// Image IO, reached the way a front end that opens a file the user picked and writes the result back reaches it:
/// through the module for the operations, and through the crate root for the types needed to call them.
#[test]
fn image_io_is_reachable_from_the_crate_root() {
    // Built here rather than read from a fixture, which is itself part of what is being checked: `DynamicImage` is in
    // this crate's public API, so a consumer must be able to make one and hand it over.
    let picture = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(32, 24, |x, y| {
        image::Rgb([(x * 8) as u8, (y * 10) as u8, 128])
    }));

    let dir = tempfile::tempdir().expect("a temporary directory");
    let destination = dir.path().join("holiday.jpg");

    // The format is an argument and the settings are per format, both named from the crate root.
    let format: ImageFormat = ImageFormat::Jpeg;
    let options: Option<EncodeOptions> = Some(EncodeOptions::Jpeg { quality: 90 });
    let written: u64 = opai::image::save_blocking(&picture, &destination, format, options).expect("writable");
    assert!(written > 0);

    // Described without being decoded, and then decoded, both agreeing about the picture.
    let info: ImageInfo = opai::image::probe_blocking(&destination).expect("the header is readable");
    assert_eq!((info.format, info.width, info.height), (ImageFormat::Jpeg, 32, 24));

    let loaded: Picture = opai::image::load_blocking(&destination).expect("decodable");
    assert_eq!(loaded.dimensions(), (32, 24));
    assert_eq!(loaded.path(), destination);
    // The identity a file listing computes from the path alone, which is what the cache will be keyed on.
    assert_eq!(loaded.identity().len(), 16, "an XXH3-64 is 16 hexadecimal characters: {}", loaded.identity());

    // The shared handle the asynchronous form takes, and the constructor that pairs pixels with an identity.
    let rebuilt = Picture::new(loaded.path(), loaded.shared_pixels(), loaded.identity());
    assert_eq!(rebuilt.identity(), loaded.identity());

    // The identity without the decode, which is what a file listing is built on: the same value the loaded picture
    // above carries, for a file that was never opened.
    let named: String = opai::image::identity_blocking(&destination).expect("readable");
    assert_eq!(named, loaded.identity(), "a described file and a loaded one must be one photograph");

    // The same encode `save_blocking` performs, with no file at the end of it — what a preview streamed to a window
    // needs. Its failure is the one variant of `ImageIoError` that names no path, there being none.
    let bytes: Vec<u8> = opai::image::encode_blocking(loaded.pixels(), ImageFormat::Jpeg, options).expect("encodable");
    assert!(!bytes.is_empty());
    let mismatched: ImageIoError =
        opai::image::encode_blocking(loaded.pixels(), ImageFormat::Png, options).expect_err("JPEG options, PNG");
    assert!(
        matches!(mismatched, ImageIoError::Encode { .. }),
        "expected an encode refusal, got {mismatched:?}"
    );

    // What a file picker filters on, which is the other thing a front end opening a file needs from this module.
    let extensions: Vec<&'static str> = opai::image::input_extensions();
    assert!(
        extensions.contains(&"jpg") && extensions.contains(&"nef"),
        "the picker's filter is built from this"
    );
    assert!(!extensions.contains(&"gpr"), "a RAW format this crate refuses must not be offered");

    // The error is its own type rather than a variant of `InitError`, and a front end matches on it. A `.gpr`
    // rather than a `.nef`: the refusal is about RAW formats that have no decoder, and NEF is decoded.
    let refused: ImageIoError =
        opai::image::load_blocking(dir.path().join("HERO0001.gpr")).expect_err("GoPro RAW has no decoder");
    match refused {
        ImageIoError::UnsupportedRaw { extension, .. } => assert_eq!(extension, "gpr"),
        other => panic!("expected an unsupported-RAW refusal, got {other:?}"),
    }

    // What a codec said, reached without naming `rust-sak`. `ImageError` is a public field of two `ImageIoError`
    // variants, so a front end that could not name it could display a decode failure and never match on it — and it
    // cannot simply depend on `rust-sak` to get the name, which is pinned by git tag and on no registry.
    //
    // This file is inside the `opai` package, so it could reach `rust_sak::image::ImageError` by that path too and
    // cannot prove the crate is unreachable from outside. What it does pin is the re-export: deleting it from
    // `image/mod.rs` or the crate root stops this line compiling.
    let undecodable = opai::image::load_blocking(write_fixture(dir.path(), "notes.png", b"not an image at all"))
        .expect_err("plain text is not an image");
    match undecodable {
        ImageIoError::Decode { source, .. } => {
            let source: ImageError = source;
            assert!(matches!(source, ImageError::UnrecognizedFormat), "expected no signature to match: {source:?}");
        }
        other => panic!("expected a decode failure, got {other:?}"),
    }
}

/// Camera RAW, reached the way a front end listing a folder of photographs reaches it: recognise the name, pick the
/// operation that matches it, and read back what it reports — all of it named from outside the `image` module and
/// without `rust_sak` appearing anywhere.
///
/// That last part is what this pins. `RawImageInfo` and `RawFormat` are `rust-sak`'s types, and a front end that
/// could not name them could call [`opai::image::probe_raw_blocking`] and never look at what came back — while
/// being unable to add `rust-sak` to its own manifest, which is pinned by git tag and on no registry.
#[test]
fn camera_raw_is_reachable_from_the_crate_root() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = write_fixture(dir.path(), "DSC_0001.dng", &written_dng());

    // The routing question, asked before either describe operation is chosen. It is about the name, so it answers
    // for a path that does not exist — which is the whole reason a listing can use it.
    assert!(opai::image::is_raw(&path));
    assert!(opai::image::is_raw("/pictures/DSC_0001.NEF"), "the case the camera wrote does not matter");
    assert!(
        !opai::image::is_raw(dir.path().join("holiday.tiff")),
        "a TIFF is not RAW, however alike they look"
    );

    // Described without being decoded, both types named from here.
    let info: RawImageInfo = opai::image::probe_raw_blocking(&path).expect("describable");
    let format: Option<RawFormat> = info.format;
    assert_eq!(format, Some(RawFormat::Dng));
    assert_eq!(format.expect("named from the extension").extension(), "dng");
    assert!(info.is_dng);
    assert_eq!((info.width, info.height), (64, 64));
    assert_eq!(info.make, DNG_MAKE);
    assert_eq!(info.model, DNG_MODEL);

    // And then loaded, arriving as the same `Picture` any other format arrives as — same path, same identity,
    // and the dimensions the description already reported.
    let loaded: Picture = opai::image::load_blocking(&path).expect("decodable");
    assert_eq!(loaded.path(), path);
    assert_eq!(loaded.dimensions(), (info.width, info.height));
    assert_eq!(loaded.identity().len(), 16, "an XXH3-64 is 16 hexadecimal characters: {}", loaded.identity());
    // A developed RAW is 16-bit RGB, which a consumer can see because `DynamicImage` is in this crate's API.
    assert!(loaded.pixels().as_rgb16().is_some(), "expected 16-bit RGB, got {:?}", loaded.pixels().color());

    // `probe` covers the eight writable formats, so it refuses a RAW name rather than dispatching to the RAW form.
    // `is_raw` above is how a caller avoids asking, which is why this is a codec refusal and not its own variant.
    let misrouted = opai::image::probe_blocking(&path).expect_err("probing does not cover RAW");
    assert!(matches!(misrouted, ImageIoError::Codec { .. }), "expected a codec refusal, got {misrouted:?}");
}

/// The asynchronous form of each operation, named so that a change to any of their signatures stops compiling here.
#[tokio::test]
async fn the_asynchronous_image_operations_are_reachable_from_the_crate_root() {
    let picture = std::sync::Arc::new(image::DynamicImage::ImageRgb8(image::RgbImage::new(32, 24)));
    let dir = tempfile::tempdir().expect("a temporary directory");
    let destination = dir.path().join("blank.png");

    let written = opai::image::save(std::sync::Arc::clone(&picture), &destination, ImageFormat::Png, None)
        .await
        .expect("writable");
    assert!(written > 0);

    let info = opai::image::probe(&destination).await.expect("the header is readable");
    assert_eq!((info.width, info.height), (32, 24));

    let loaded = opai::image::load(&destination).await.expect("decodable");
    assert_eq!(loaded.pixels(), picture.as_ref());

    let named = opai::image::identity(&destination).await.expect("readable");
    assert_eq!(named, loaded.identity());

    let bytes = opai::image::encode(loaded.shared_pixels(), ImageFormat::Png, None).await.expect("encodable");
    assert_eq!(
        bytes,
        std::fs::read(&destination).expect("readable"),
        "the encode is the one that wrote the file"
    );
}

/// The provider choice, reached the way a front end offering it in a settings pane reaches it: parse what was
/// stored, show what it is, store it again.
///
/// Named from outside the crate because that is the whole reason the type is public. Nothing takes one as a
/// parameter yet — the chain's entry point is a later slice — so this is the only place the surface is exercised
/// from a consumer.
#[test]
fn the_provider_choice_is_reachable_from_the_crate_root() {
    let _: Option<ExecutionProvider> = None;

    // What a settings file holds, read back the way a front end that did not write it has to read it.
    let stored: ExecutionProvider = "coreml".parse().expect("a published spelling parses");
    assert_eq!(stored, ExecutionProvider::CoreMl);
    assert_eq!(stored.to_string(), "CoreML");

    // And refused where the text names no provider this build publishes, with the error the crate root already
    // exports.
    let error: InitError = "openvino".parse::<ExecutionProvider>().expect_err("an unpublished name is refused");
    assert!(matches!(error, InitError::UnknownProvider { .. }), "{error}");

    // Every choice a chooser would offer, from the one list the crate publishes.
    assert_eq!(ExecutionProvider::ALL.len(), 5);
    assert!(ExecutionProvider::ALL.contains(&ExecutionProvider::Auto));
}

/// What a consumer can learn and decide about the run cache, named through the public surface alone.
#[test]
fn the_run_cache_can_be_reported_on_and_turned_off_through_the_public_surface_alone() {
    // The reporter, pinned through a function pointer: it takes nothing but the application, which is what makes it
    // a report rather than a setting. There is deliberately no counterpart that sets one.
    let reported: fn(&Opai) -> CacheMode = Opai::cache_mode;
    let _ = reported;

    // All three answers, matched with a catch-all as `#[non_exhaustive]` requires — which is also how a front end
    // decides whether to warn a user that its runs will be slower than they need to be.
    for mode in [CacheMode::Disk, CacheMode::Memory, CacheMode::None] {
        let warning = match mode {
            CacheMode::Disk => None,
            CacheMode::Memory => Some("results are being kept in memory rather than on disk"),
            CacheMode::None => Some("results are not being kept at all"),
            _ => Some("the cache is degraded"),
        };

        assert_eq!(warning.is_none(), mode == CacheMode::Disk);
    }

    // Copy, comparable and printable, which is what a front end holding one in its own state needs.
    let mode = CacheMode::Disk;
    let held = mode;
    assert_eq!(held, mode);
    assert!(!format!("{mode:?}").is_empty());

    // And the per-run decision, which is a different question from what is backing the store: a run with the cache
    // turned off still reads `Disk` above.
    assert!(ProcessOptions::default().cache);
    assert!(!ProcessOptions { cache: false, ..Default::default() }.cache);
}

/// What a setup dialog reads before anything is transferred, and what it reads when a dependency turns out to have
/// nothing to do — through the public surface alone.
///
/// A consumer rather than an assertion about the crate's internals: the four facts a component list is built on are
/// `opai`'s to state, and a front end that cannot reach one of them from the crate root cannot draw the dialog.
#[test]
fn a_front_end_can_draw_a_component_list_through_the_public_surface_alone() {
    // The row, read field by field: a dependency and the published size of the files backing it, both of which a
    // front end shows before that dependency has started. Named here so a rename of either stops compiling.
    let row = PlannedDependency { dependency: Dependency::Cuda, size: 587_322_552 };
    assert_eq!(row.dependency.as_str(), "NVIDIA CUDA");
    assert_eq!(row.size, 587_322_552);

    // Cloneable and comparable, which is what a front end keeping the list in its own store needs.
    assert_eq!(row.clone(), row);
    assert!(!format!("{row:?}").is_empty());

    // The handler, in the shape a front end registers it in — one call with the whole list, so nothing has to guess
    // when the list is complete — and as a field of the bundle it is registered through.
    let seen: std::sync::Arc<std::sync::Mutex<Vec<PlannedDependency>>> = Default::default();
    let sink = std::sync::Arc::clone(&seen);
    let on_plan: OnPlan = std::sync::Arc::new(move |rows: &[PlannedDependency]| {
        sink.lock().unwrap().extend_from_slice(rows);
    });

    let planning = InitOptions { on_plan: Some(std::sync::Arc::clone(&on_plan)), ..Default::default() };
    assert!(planning.on_plan.is_some());
    assert!(planning.app.is_none() && planning.on_progress.is_none(), "one field was set and the rest moved");
    assert!(InitOptions::default().on_plan.is_none(), "a caller that asks for none is charged for none");

    // Handed a list the way an initialization hands it one, which is the only way a front end ever obtains a row:
    // there is deliberately no constructor and no pre-flight query — the plan comes from the initialization that runs
    // it, so the list shown and the list run cannot disagree.
    let rows = seen.lock().unwrap().clone();
    assert!(rows.is_empty(), "a plan arrived without an initialization");

    // The outcome a dependency with nothing to do reports, matched with a catch-all as `#[non_exhaustive]` requires.
    // It is what tells a finished row from one not yet reached — both of the terminal reports carry a fraction of
    // `1.0`, and only the phase says which.
    let already = Progress {
        dependency: Dependency::Runtime,
        phase: Phase::AlreadyInstalled,
        bytes: 0,
        total: Some(174_834_737),
        fraction: 1.0,
    };

    let state = match already.phase {
        Phase::AlreadyInstalled => "in place",
        Phase::Downloading | Phase::Extracting => "working",
        _ => "unknown",
    };

    assert_eq!(state, "in place");
    assert_eq!(already.bytes, 0, "an install that did not happen claimed bytes");
    assert_eq!(already.fraction, 1.0);
}

/// What a binary must name to initialize, and what it can read off a refusal — through the public surface alone.
///
/// The identity is optional vocabulary and the bundle is not: `Opai::initialize` cannot be called without naming
/// `InitOptions`, and a front end that cannot reach the three spellings from the crate root cannot name itself
/// without a literal. This is a consumer, which is what makes both of those a check.
#[test]
fn a_binary_can_name_itself_and_read_a_refusal_through_the_public_surface_alone() {
    // The exact signature `Opai::initialize` has, pinned through a function pointer: the application's name, and
    // everything else behind one optional bundle. Named rather than called, because calling it would claim this
    // machine's real configuration directory and install a runtime.
    type Init<'a> = std::pin::Pin<Box<dyn Future<Output = Result<Opai, InitError>> + 'a>>;
    let initialize: for<'a> fn(&'a str, Option<InitOptions>) -> Init<'a> =
        |name, options| Box::pin(Opai::initialize(name, options));
    let _ = initialize;

    // `None` is every default, and `..Default::default()` is how one field is set without naming the others — the
    // property that makes the field the next change adds cost no call site. Both spellings from outside the crate.
    let _ = InitOptions::default();
    let declared = InitOptions { app: Some(GUI.to_string()), ..Default::default() };
    assert_eq!(declared.app.as_deref(), Some("gui"));
    assert!(declared.on_progress.is_none());

    // A progress handler still reaches the install, now as a field of the bundle rather than a third argument.
    let with_progress = InitOptions { on_progress: None::<OnProgress>, ..Default::default() };
    assert!(with_progress.on_progress.is_none());

    // And the declaration this slice added, arriving as one more field of the same bundle — which is the property
    // that bundle was introduced for, and this is the call site it cost nothing at. The default is the ordinary rule:
    // a front end that names nothing has every model checked against what is published.
    assert_eq!(InitOptions::default().models, ModelTrust::Published);
    let debugging = InitOptions { models: ModelTrust::LocalFiles, ..Default::default() };
    assert_eq!(debugging.models, ModelTrust::LocalFiles);
    assert!(
        debugging.app.is_none() && debugging.on_progress.is_none(),
        "one field was set and the rest moved"
    );

    // Matched with a catch-all as `#[non_exhaustive]` requires, which is also how a binary decides whether to warn
    // that the models it is about to run were never checked.
    for trust in [ModelTrust::Published, ModelTrust::LocalFiles] {
        let unverified = match trust {
            ModelTrust::Published => false,
            ModelTrust::LocalFiles => true,
            _ => true,
        };

        assert_eq!(unverified, trust == ModelTrust::LocalFiles);
    }

    // Copy, comparable and printable, which is what a binary holding one in its own configuration needs.
    let held = debugging.models;
    assert_eq!(held, debugging.models);
    assert!(!format!("{held:?}").is_empty());

    // **There is deliberately no setter and no per-run field.** A process that did not declare this when it started
    // cannot arrive at it later, which is why it is a field of the initialization bundle and nowhere else. Nothing
    // here can name one, which is the check — `ProcessOptions` below carries `cache` and no counterpart to this.

    // This project's three spellings, reachable as constants rather than as literals a front end could misspell —
    // which is where the closed enum's compile-time protection actually was.
    assert_eq!([GUI, CLI, PERF], ["gui", "cli", "perf"]);

    // The refusal, matched with a catch-all as `#[non_exhaustive]` requires, in both the shapes it comes in: a holder
    // that could be read and one that could not. A front end renders the first as "close the GUI (pid 4821)" and the
    // second as "something else is running", and both messages print unchanged.
    for holder in [Some(Holder { app: GUI.to_string(), pid: 4821 }), None] {
        let error = InitError::AlreadyRunning { holder };

        match &error {
            InitError::AlreadyRunning { holder: Some(held) } => {
                assert_eq!(held.app, GUI);
                assert_eq!(held.pid, 4821);
            }
            InitError::AlreadyRunning { holder: None } => {}
            _ => panic!("a refusal was matched as something else"),
        }

        assert!(error.to_string().contains("already running"));
    }

    // And the identity that would not survive being recorded is reportable from outside the crate, in the shape its
    // sibling `InvalidName` already had.
    let rejected = InitError::InvalidApp { app: "my app".to_string(), reason: "it contains whitespace" };
    match &rejected {
        InitError::InvalidApp { app, reason } => {
            assert_eq!(app, "my app");
            assert_eq!(*reason, "it contains whitespace");
        }
        _ => panic!("a rejected identity was matched as something else"),
    }
    assert!(rejected.to_string().contains("cannot be recorded"));
}

/// A whole enhancement, assembled the way a front end assembles one: pick the operation from the catalogue, state the
/// options, register a progress callback, keep the token that cancels it, and call the one function that runs it.
///
/// Every type named here comes from the crate root. Nothing reaches into a module, and `rust_sak` and `tokio_util`
/// appear nowhere — which is the point, since neither could be added to a front end's manifest for the sake of naming
/// one field.
///
/// **What this cannot do is run a model.** Doing so needs the pinned ONNX Runtime and a multi-hundred-megabyte
/// download, and this suite is hermetic; the live run is the `#[ignore]`d end-to-end test beside it, run by hand. So
/// [`Opai::process`] is named through a function pointer, which pins its exact signature — its arguments, its
/// return type and its `async`-ness — and everything that does not need a model is exercised for real.
#[tokio::test]
async fn a_chain_can_be_built_and_run_through_the_public_surface_alone() {
    // The operation, through the model's own constructor — which is what a front end calls once it has resolved the
    // row it read from the catalogue, and which hands back the carrier `process` takes with no wrapping.
    let kyoto: Operation = Upscale::kyoto(FloatPrecision::Fp16, Scale::new(4.0).expect("4x is in range"));

    // The picture, as `image::load` would have produced one.
    let source = Picture::new(
        "/pictures/holiday.jpg",
        std::sync::Arc::new(image::DynamicImage::ImageRgb8(image::RgbImage::new(64, 48))),
        "cafebabecafebabe",
    );

    // What a run reports, collected by a callback a consumer registers.
    let seen: std::sync::Arc<std::sync::Mutex<Vec<InferenceProgress>>> = Default::default();
    let sink = std::sync::Arc::clone(&seen);
    let on_progress: OnInference = std::sync::Arc::new(move |report: &InferenceProgress| {
        // Every field a front end draws a bar from, read from outside the crate.
        let label = match &report.stage {
            Stage::Installing(progress) => format!("Downloading {}", progress.dependency.as_str()),
            // The run this report is about, whichever path it is on: a `Subject` carries an operation of either,
            // so a front end that only labels a bar asks it for a name and never matches.
            Stage::Running => report.subject.display_name(),
            // The third answer, which is what lets a front end say "from an earlier run" instead of showing a bar
            // jump with no explanation.
            Stage::Cached => format!("{} — from an earlier run", report.subject.display_name()),
            // `Stage` is `#[non_exhaustive]`, as this crate's other public enums are, so a catch-all is required
            // however many variants are named above.
            _ => String::from("working"),
        };

        assert!(!label.is_empty());
        assert!((0.0..=1.0).contains(&report.chain_fraction));
        assert!((0.0..=1.0).contains(&report.operation_fraction));

        sink.lock().unwrap().push(report.clone());
    });

    // The token the consumer keeps, and a child of it — which is how an export queue stops every image in flight from
    // one gesture.
    let cancel = CancellationToken::new();
    let per_image = cancel.child_token();

    // Some fields set, the rest defaulted, which is the form that keeps a field added later source-compatible — and
    // `cache` is the field that form was built for.
    let options = ProcessOptions {
        depth: OutputDepth::Source,
        on_progress: Some(on_progress),
        cancel: per_image.clone(),
        ..Default::default()
    };

    assert_eq!(options.provider, ExecutionProvider::Auto);
    assert_eq!(OutputDepth::default(), OutputDepth::Eight);
    assert!(options.cache, "the cache is on unless a caller turns it off");

    // Turning it off for one run, which is the whole of what a benchmark or an embedder that must not persist its
    // images has to write.
    let uncached = ProcessOptions { cache: false, ..Default::default() };
    assert!(!uncached.cache);
    assert_eq!(uncached.depth, OutputDepth::Eight);

    // The entry point itself, pinned through a function pointer so that a change to any part of its signature stops
    // compiling here — including its **return type**, which this slice widened from the picture alone to the picture
    // beside what the run executed on.
    type Run<'a> = std::pin::Pin<Box<dyn Future<Output = Result<Enhanced, InferenceError>> + Send + 'a>>;
    let process: for<'a> fn(&'a Opai, &'a Picture, &'a [Operation], Option<ProcessOptions>) -> Run<'a> =
        |opai, source, operations, options| Box::pin(opai.process(source, operations, options));
    let _ = process;

    // Cancelling the parent cancels the child this run would have been given, without the consumer holding the child.
    cancel.cancel();
    assert!(per_image.is_cancelled());

    // And the failure a consumer has to be able to tell apart, matched from outside the crate.
    let cancelled = InferenceError::Cancelled;
    assert!(matches!(cancelled, InferenceError::Cancelled));
    assert!(!cancelled.to_string().is_empty());

    // The chain itself is an ordinary slice of operations, built and extended by the consumer.
    let chain = [kyoto.clone(), kyoto];
    assert_eq!(chain.len(), 2);
    assert_eq!(source.dimensions(), (64, 48));
    assert!(seen.lock().unwrap().is_empty(), "nothing ran, so nothing reported");
}

/// The second entry point, assembled the way a front end that wants faces rather than pixels assembles one.
///
/// [`Opai::execute`] is pinned the same way [`Opai::process`] is and for the same reason — running it
/// needs the pinned ONNX Runtime and a multi-hundred-megabyte download, and this suite is hermetic — so the function
/// pointer fixes its exact signature, its `async`-ness included, and everything that does not need a model is
/// exercised for real.
///
/// What the pointer's type spells that prose could not: the result is `Executed<O::Output>`, so the type a run
/// produces follows from the operation being run. There is no second parameter for a caller to state it in.
#[tokio::test]
async fn a_non_image_operation_can_be_built_and_run_through_the_public_surface_alone() {
    // The operation, through the model's own constructor — detection takes no parameter, so its precision is all
    // there is to give. What comes back is an `Analysis`, which is what `execute` takes: naming the model is what
    // decided the path, so there was nothing for the caller to state and nothing to state wrongly.
    let detection: Analysis = Detection::newyork(FloatPrecision::Fp32);
    assert_eq!(detection.family(), Family::Detection);

    // The typed value is still reachable by matching the carrier, exactly as an enhancement's is.
    let Analysis::Detection(typed) = detection;
    assert_eq!(Analysis::Detection(typed), detection);
    assert_eq!(typed.artifact().as_str(), "dt_newyork_fp32");

    let source = Picture::new(
        "/pictures/portrait.jpg",
        std::sync::Arc::new(image::DynamicImage::ImageRgb8(image::RgbImage::new(64, 48))),
        "cafebabecafebabe",
    );

    // What a run reports, collected by a callback a consumer registers — the same `OnInference` the image path takes,
    // because an analysis reports on the same terms an enhancement does.
    let seen: std::sync::Arc<std::sync::Mutex<Vec<InferenceProgress>>> = Default::default();
    let sink = std::sync::Arc::clone(&seen);
    let on_progress: OnInference = std::sync::Arc::new(move |report: &InferenceProgress| {
        assert!((0.0..=1.0).contains(&report.chain_fraction));
        sink.lock().unwrap().push(report.clone());
    });

    let cancel = CancellationToken::new();

    // Some fields set, the rest defaulted — the form that keeps a field added later source-compatible, and the form
    // `cache` arrived through without moving a call site. There is deliberately no `depth`, because bits per channel
    // means nothing for a run that produces no pixels.
    let options = ExecuteOptions { on_progress: Some(on_progress), cancel: cancel.clone(), ..Default::default() };
    assert_eq!(options.provider, ExecutionProvider::Auto);
    assert_eq!(ExecuteOptions::default().provider, ExecutionProvider::Auto);
    assert!(!format!("{options:?}").is_empty());

    // The store, named and read back the way `ProcessOptions::cache` is above: on unless a caller turns it off, and
    // the same question on both paths, because it is the same store under both.
    assert!(options.cache, "an analysis is kept unless a caller turns the store off");
    assert!(ExecuteOptions::default().cache);

    // Turning it off for one run, which is the whole of what a benchmark measuring what a detection costs has to
    // write — a run that read a stored result would measure a read, and one that stored its own would change what the
    // next measurement measured.
    let uncached = ExecuteOptions { cache: false, ..Default::default() };
    assert!(!uncached.cache);
    assert_eq!(uncached.provider, ExecutionProvider::Auto);
    assert!(format!("{uncached:?}").contains("cache"), "the manual `Debug` impl dropped the field");

    // The entry point itself, pinned through a function pointer so that a change to any part of its signature stops
    // compiling here — including the `Executed<Faces>` that the operation, rather than this call site, decided.
    type Run<'a> = std::pin::Pin<Box<dyn Future<Output = Result<Executed<Faces>, InferenceError>> + Send + 'a>>;
    let execute: for<'a> fn(&'a Opai, &'a Picture, &'a Analysis, Option<ExecuteOptions>) -> Run<'a> =
        |opai, source, analysis, options| Box::pin(opai.execute(source, analysis, options));
    let _ = execute;

    // The picture the call above would be given, built the way `image::load` produces one. Named here rather than
    // only inside the pointer so that a change to what `execute` accepts on the way in stops compiling too.
    assert_eq!(source.dimensions(), (64, 48));
    assert_eq!(source.identity(), "cafebabecafebabe");

    // The bound itself, named from outside the crate. A caller writes this to accept any operation on this path
    // without naming the families; what it cannot write is an implementation of its own, the trait being sealed.
    fn result_of<O: DataOperation>(_operation: &O) {}
    result_of(&detection);

    // And the bundle it hands back, constructed and destructured the way a front end takes the half it wants — the
    // `..` being what keeps a field added by a later slice source-compatible here.
    let executed = Executed {
        value: Faces::new([Face::new(
            Rect::new(Point::new(10.0, 20.0), Point::new(110.0, 140.0)),
            [Point::new(1.0, 2.0); Face::LANDMARKS],
            Confidence::new(0.97).expect("a confidence in range"),
        )]),
        providers: ProviderReport { requested: ExecutionProvider::Cuda, actual: vec![ExecutionProvider::Cpu] },
    };

    // The report is on the result rather than the progress stream, which is what lets a caller that registered no
    // callback still find out that a run was downgraded — the whole reason it is here rather than there.
    assert_eq!(
        executed.providers.verdict(),
        ProviderVerdict::Downgraded { requested: ExecutionProvider::Cuda, actual: vec![ExecutionProvider::Cpu] }
    );

    let Executed { value: faces, .. } = executed;
    assert_eq!(faces.len(), 1);
    assert_eq!(faces.as_slice()[0].confidence().get(), 0.97);

    // An empty set is a legitimate finding rather than an error — an image with no face in it.
    let none: Executed<Faces> = Executed {
        value: Faces::empty(),
        providers: ProviderReport { requested: ExecutionProvider::Auto, actual: Vec::new() },
    };
    assert!(none.value.is_empty());
    assert_eq!(none.providers.verdict(), ProviderVerdict::NothingExecuted);

    cancel.cancel();
    assert!(seen.lock().unwrap().is_empty(), "nothing ran, so nothing reported");
}

/// What a run reports having executed on, and the gesture that gives every resident session back — the two halves of
/// this slice that a front end reaches without initializing anything.
#[test]
fn a_consumer_can_read_what_a_run_ran_on_and_release_every_session() {
    // The release, pinned through a function pointer: it takes nothing but the application and answers nothing, which
    // is what makes it a gesture rather than a query. Named rather than called — calling it would need a real
    // initialization, which claims this machine's configuration directory and installs a runtime.
    let release: fn(&Opai) = Opai::release_sessions;
    let _ = release;

    // The report, constructed the way `Opai::process` hands one back and read the way a front end reads one.
    let honoured = ProviderReport { requested: ExecutionProvider::CoreMl, actual: vec![ExecutionProvider::CoreMl] };
    let downgraded = ProviderReport { requested: ExecutionProvider::Cuda, actual: vec![ExecutionProvider::Cpu] };
    let nothing_ran = ProviderReport { requested: ExecutionProvider::Cuda, actual: Vec::new() };

    // The three outcomes a consumer has to be able to tell apart, written the way a front end would decide what to
    // show a user. The third is the one that is **not** "it ran on what was asked for": an empty chain and a
    // fully-cached run both take no session, and claiming the requested provider would be claiming a measurement.
    let describe = |report: &ProviderReport| match report.actual.as_slice() {
        [] => "nothing was executed".to_string(),
        [only] if *only == report.requested => format!("ran on {}", only.as_str()),
        actual => format!(
            "{} could not open every model; ran on {}",
            report.requested.as_str(),
            actual.iter().map(|provider| provider.as_str()).collect::<Vec<_>>().join(", ")
        ),
    };

    assert_eq!(describe(&honoured), "ran on CoreML");
    assert_eq!(describe(&downgraded), "CUDA could not open every model; ran on CPU");
    assert_eq!(describe(&nothing_ran), "nothing was executed");

    // Comparable and printable, which is what a front end holding one in its own state and a bug report both need —
    // and the `Debug` renders both halves, so a logged report cannot make the downgrade invisible again.
    assert_ne!(honoured, downgraded);
    assert_eq!(honoured.clone(), honoured);
    let rendered = format!("{downgraded:?}");
    assert!(rendered.contains("Cuda") && rendered.contains("Cpu"), "{rendered}");

    // And the bundle the two halves travel in, destructured the way a caller that wants one half takes it — the `..`
    // being what keeps a field added by a later slice source-compatible here.
    let picture = Picture::new(
        "/pictures/holiday.jpg",
        std::sync::Arc::new(image::DynamicImage::ImageRgb8(image::RgbImage::new(64, 48))),
        "cafebabecafebabe",
    );
    let enhanced = Enhanced { picture, providers: downgraded.clone() };

    assert_eq!(enhanced.providers, downgraded);
    assert_eq!(enhanced.picture.dimensions(), (64, 48));

    let Enhanced { picture, .. } = enhanced;
    assert_eq!(picture.identity().len(), 16, "the result's identity is the same shape it always was");
}

#[test]
fn a_binary_can_get_itself_a_log_through_the_public_surface_alone() {
    // Built the way a `main` builds it: the name comes from the library rather than being retyped by each front end,
    // which is the whole reason it is published. Two front ends with two literals would be two configuration
    // directories, two model installs, two caches and two locks.
    assert_eq!(APP_NAME, "io.vinicius.opai");
    assert!(!APP_NAME.is_empty());

    // Asking where the log is creates nothing and installs nothing, which is what lets a settings screen show the
    // path whether or not this process is the one writing there. Checked against a name of this test's own, so the
    // assertion that nothing was created is about this call.
    let name = "opai-public-api-log-test";
    let path = logging::log_path(name).expect("the log path must be resolvable");

    assert!(path.ends_with(std::path::Path::new("logs").join("opai.log")));
    assert!(!path.exists(), "asking where the log is created it");
    assert!(!path.parent().unwrap().exists(), "asking where the log is created its directory");

    // `opai.log` whatever the application is called: it is what a user is asked to attach.
    assert_eq!(path.file_name().unwrap(), "opai.log");
    assert!(path.to_string_lossy().contains(name), "the path is not under the name it was asked for");

    // A name that cannot be a directory is refused rather than resolved, and the refusal is a variant a front end can
    // match from out here.
    let refused = logging::log_path("../escape").unwrap_err();
    assert!(matches!(refused, LogError::Fs(_)), "got {refused:?}");
    assert!(!refused.to_string().is_empty());

    // Every variant a caller has to be able to tell apart. `AlreadyInstalled` is the one that is not about the disk:
    // it says this caller did not install the sink, and the one installed first is still writing — so a front end
    // seeing it carries on rather than retrying.
    let already = LogError::AlreadyInstalled;
    assert!(matches!(already, LogError::AlreadyInstalled));
    assert!(already.to_string().contains("already installed"));

    // The entry point itself, pinned through a function pointer so that a change to any part of its signature stops
    // compiling here. It takes the name and the application and hands back the path it opened — no guard to hold, no
    // handle to keep, nothing to close.
    let init: fn(&str, &str) -> Result<std::path::PathBuf, LogError> = logging::init;
    let where_is: fn(&str) -> Result<std::path::PathBuf, LogError> = logging::log_path;
    let _ = (init, where_is);

    // And the identity a binary names itself with is the same value the single-instance refusal names a holder with,
    // which is what stops the log field and the refusal message from drifting — one identity, whether it is one of
    // this project's three or an embedder's own.
    for app in [GUI, CLI, PERF, "com.example.Photos"] {
        assert_eq!(Holder { app: app.to_string(), pid: 1 }.app, app);
    }

    // `init` itself is deliberately *not* called here: it installs a subscriber for the life of the process and may
    // be called once, so driving it belongs in a test that owns its process. `tests/logging_session.rs` is that test.
}

/// The third entry point, assembled the way a front end wiring its Autopilot toggle assembles one.
///
/// [`Opai::suggest`] is pinned through a function pointer for the reason
/// [`Opai::execute`] is above — running it needs the pinned ONNX Runtime and a multi-hundred-megabyte
/// download, and this suite is hermetic — so the pointer fixes its exact signature, its `async`-ness included, and
/// everything that does not need a model is exercised for real.
///
/// What the types spell that prose could not: a suggestion names a **family** and carries only what the image
/// determined, so there is no variant and no precision here for a caller to read or to have to supply; and the
/// result is a struct rather than a `Vec`, so "this photograph needs nothing" and "I could not tell" are two values
/// rather than one.
#[tokio::test]
async fn an_analysis_can_be_read_through_the_public_surface_alone() {
    // Every arm, named in a construction — which is what fails to compile if one is renamed or stops being
    // reachable from the crate root.
    let denoise = Suggestion::Denoise;
    let face_recovery = Suggestion::FaceRecovery;
    let colorization = Suggestion::Colorization;
    let light = Suggestion::LightAdjustment;
    let colour = Suggestion::ColorBalance;
    let sharpen = Suggestion::Sharpen;
    let upscale = Suggestion::Upscale { scale: Scale::clamped(4.0) };

    // The family is what a suggestion names, and it is the vocabulary the rest of the crate already speaks — so a
    // front end can map one to its own sidebar row without a second table of its own.
    assert_eq!(denoise.family(), Family::Denoise);
    assert_eq!(face_recovery.family(), Family::FaceRecovery);
    assert_eq!(colorization.family(), Family::Colorization);
    assert_eq!(light.family(), Family::LightAdjustment);
    assert_eq!(colour.family(), Family::ColorBalance);
    assert_eq!(sharpen.family(), Family::Sharpen);
    assert_eq!(upscale.family(), Family::Upscale);

    // The scale is reachable by matching the arm, exactly as an operation's typed value is.
    let Suggestion::Upscale { scale } = upscale else {
        panic!("the arm built is not the arm matched");
    };
    assert_eq!(scale.get(), 4.0);

    // No model variant and no precision anywhere in the value: what to run is the user's standing preference, and a
    // front end holds it. Building the operation is the front end's own step, and this is what it has to add.
    let accepted: Operation = Upscale::tokyo(FloatPrecision::Fp32, scale);
    assert_eq!(accepted.family(), upscale.family());

    // The result, constructed and destructured the way a caller takes the half it wants — the `..` being what keeps
    // a field added by a later slice source-compatible here.
    let concluded = Suggestions { suggested: vec![face_recovery, upscale], incomplete: None };
    assert_eq!(concluded.suggested.len(), 2);
    assert!(concluded.incomplete.is_none());

    let Suggestions { suggested, .. } = concluded;
    assert_eq!(
        suggested[0],
        Suggestion::FaceRecovery,
        "suggestions arrive in `Family::ALL` order, the one family order this library names"
    );

    // The distinction the struct exists for, read back from outside the crate: a photograph that needs nothing and
    // one whose signals could not be read are two values, not one empty list.
    let nothing_needed = Suggestions { suggested: Vec::new(), incomplete: None };
    let could_not_tell = Suggestions { suggested: Vec::new(), incomplete: Some(InferenceError::Cancelled) };

    assert!(nothing_needed.suggested.is_empty() && nothing_needed.incomplete.is_none());
    assert!(could_not_tell.suggested.is_empty());
    assert!(
        could_not_tell.incomplete.is_some_and(|error| matches!(error, InferenceError::Cancelled)),
        "the reason a signal could not be read is readable from outside the crate"
    );

    // The entry point itself, pinned so that a change to any part of its signature stops compiling here — including
    // that it takes the same `ExecuteOptions` the detection path does rather than a third near-identical type.
    type Analyse<'a> = std::pin::Pin<Box<dyn Future<Output = Result<Suggestions, InferenceError>> + Send + 'a>>;
    type Suggest = for<'a> fn(&'a Opai, &'a Picture, Option<&'a [Family]>, Option<ExecuteOptions>) -> Analyse<'a>;
    let suggest: Suggest = |opai, source, families, options| Box::pin(opai.suggest(source, families, options));
    let _ = suggest;

    // The picture the call above would be given, and the options it would carry — named here rather than only
    // inside the pointer so that a change to what it accepts on the way in stops compiling too.
    let source = Picture::new(
        "/pictures/portrait.jpg",
        std::sync::Arc::new(image::DynamicImage::ImageRgb8(image::RgbImage::new(64, 48))),
        "cafebabecafebabe",
    );
    assert_eq!(source.dimensions(), (64, 48));

    let cancel = CancellationToken::new();
    let options = ExecuteOptions { cancel: cancel.clone(), ..Default::default() };
    assert_eq!(options.provider, ExecutionProvider::Auto);
    assert!(
        options.cache,
        "a repeated analysis is served rather than executed unless a caller turns the store off"
    );

    cancel.cancel();
}
