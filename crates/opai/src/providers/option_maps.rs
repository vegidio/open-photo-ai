//! Every model's provider options and session settings, as `rust-sak` builds them, for every session the catalogue
//! opens.
//!
//! The maps were checked key for key against this crate's own plan before that plan was deleted, and recorded in
//! `testdata/option_maps.txt`. That snapshot is what holds every model to them now: a drifted key would only show as a
//! slower model. Set `OPAI_UPDATE_OPTION_MAPS=1` to rewrite it after a deliberate change, and read the diff.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rust_sak::ai::{self, CachePaths, ExecutionMode, ExecutionProvider, Tuning};

use crate::models::operation::sealed::DataModel as _;
use crate::models::test_support::{every_analysis, every_operation};
use crate::models::{ArtifactId, FloatPrecision, OsakaPrecision, Scale, Upscale, UpscaleVariant};
use crate::pipeline::test_support::NoBackend;

const ACCELERATORS: [ExecutionProvider; 4] = [
    ExecutionProvider::TensorRt,
    ExecutionProvider::Cuda,
    ExecutionProvider::CoreMl,
    ExecutionProvider::WebGpu,
];

/// The snapshot, beside this file.
fn snapshot_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/providers/testdata/option_maps.txt")
}

/// Every distinct session the catalogue builds: each artifact with the tuning it is opened under.
fn every_session() -> Vec<(ArtifactId, Tuning)> {
    let mut sessions: Vec<(ArtifactId, Tuning)> = Vec::new();
    let mut push = |artifact: &ArtifactId, tuning: &Tuning| {
        if !sessions.iter().any(|(seen, with)| seen == artifact && with == tuning) {
            sessions.push((artifact.clone(), tuning.clone()));
        }
    };

    for operation in every_operation() {
        let pipeline = operation.pipeline::<NoBackend>().expect("every operation reaches a pipeline");
        for (artifact, tuning) in pipeline.sessions() {
            push(artifact, tuning);
        }
    }
    for analysis in every_analysis() {
        let pipeline = analysis.pipeline::<NoBackend>().expect("every analysis reaches a pipeline");
        for (artifact, tuning) in pipeline.sessions() {
            push(artifact, tuning);
        }
    }

    sessions.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));
    sessions
}

/// Fixed paths, one engine folder per model and one shared timing folder, built without joining so they display alike
/// on every platform. Where they really are is `sessions::caches`'s, and tested there.
fn paths_for(artifact: &ArtifactId) -> CachePaths {
    let engine = PathBuf::from(format!("/engines/{artifact}"));
    CachePaths { tensorrt: engine.clone(), timing: PathBuf::from("/engines/.timing"), coreml: engine }
}

/// `provider`'s options for `tuning`, against [`paths_for`] `artifact` and a card whose memory is unknown.
fn options(provider: ExecutionProvider, tuning: &Tuning, artifact: &ArtifactId) -> BTreeMap<String, String> {
    ai::provider_options(provider, tuning, &paths_for(artifact), None)
}

/// The session settings `rust-sak` applies for `tuning`, as its builder calls take them: parallel execution, the
/// memory planner, and the disabled optimizers joined by `;` with empty names dropped.
fn session_settings(tuning: &Tuning) -> (bool, bool, String) {
    let names: Vec<&str> =
        tuning.disabled_optimizers.iter().map(String::as_str).filter(|name| !name.is_empty()).collect();
    (tuning.execution_mode == ExecutionMode::Parallel, !tuning.disable_mem_pattern, names.join(";"))
}

/// Every map and setting as one line each, in the order the snapshot keeps them.
fn snapshot_lines() -> Vec<String> {
    let mut lines = Vec::new();
    for (artifact, tuning) in every_session() {
        for provider in ACCELERATORS {
            for (key, value) in options(provider, &tuning, &artifact) {
                lines.push(format!("{artifact} {provider} {key}={}", value.replace('\n', "\\n")));
            }
        }
        let (parallel, mem_pattern, disabled) = session_settings(&tuning);
        lines.push(format!(
            "{artifact} session parallel={parallel} mem_pattern={mem_pattern} disabled_optimizers={disabled}"
        ));
    }
    lines
}

#[test]
fn the_spread_reaches_the_whole_catalogue() {
    let artifacts: BTreeSet<String> =
        every_session().into_iter().map(|(artifact, _)| artifact.as_str().to_string()).collect();
    assert!(artifacts.len() > 30, "the spread missed most of the catalogue: {artifacts:?}");
}

#[test]
fn every_models_options_match_the_snapshot() {
    let current = snapshot_lines().join("\n") + "\n";
    let path = snapshot_path();

    if std::env::var_os("OPAI_UPDATE_OPTION_MAPS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &current).unwrap();
        return;
    }

    let recorded = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!("{} could not be read ({error}); set OPAI_UPDATE_OPTION_MAPS=1 to write it", path.display())
    });
    let recorded: Vec<&str> = recorded.lines().collect();
    let current: Vec<&str> = current.lines().collect();
    let missing: Vec<&&str> = recorded.iter().filter(|line| !current.contains(line)).collect();
    let added: Vec<&&str> = current.iter().filter(|line| !recorded.contains(line)).collect();
    assert!(
        missing.is_empty() && added.is_empty(),
        "the option maps drifted\nno longer:\n{missing:#?}\nnow:\n{added:#?}"
    );
}

/// An upscale at 4x, the scale every one of these models is published at.
fn upscale(variant: UpscaleVariant) -> Upscale {
    Upscale::new(variant, Scale::new(4.0).expect("4.0 is in range"))
}

/// One CoreML option of `upscale`'s tuning.
fn coreml(upscale: &Upscale, key: &str) -> String {
    let artifact = crate::models::test_support::kyoto();
    options(ExecutionProvider::CoreMl, &upscale.profile(), &artifact)[key].clone()
}

#[test]
fn kyoto_and_saitama_at_fp16_reach_the_cpu_and_the_neural_engine_and_at_fp32_take_the_default() {
    // A measured declaration read through the option map it ends up in, so the tuning and the key it configures are
    // checked together. The two RRDBNets agreeing is a fact about their graphs: same op mix, same answer.
    for variant in [UpscaleVariant::Kyoto, UpscaleVariant::Saitama] {
        let fp16 = upscale(variant(FloatPrecision::Fp16));
        let fp32 = upscale(variant(FloatPrecision::Fp32));
        assert_eq!(
            coreml(&fp16, "MLComputeUnits"),
            "CPUAndNeuralEngine",
            "{fp16:?} did not reach the Neural Engine"
        );
        assert_eq!(coreml(&fp32, "MLComputeUnits"), "ALL", "{fp32:?} was given the FP16 answer");
    }
}

#[test]
fn tokyo_and_osaka_reach_the_cpu_and_the_gpu_at_both_precisions() {
    // The exclusion rather than the preference: Tokyo's window attention and Osaka's VAE both lose on the Neural
    // Engine, so both precisions have to arrive at `CPUAndGPU`.
    for precision in FloatPrecision::ALL {
        assert_eq!(coreml(&upscale(UpscaleVariant::Tokyo(precision)), "MLComputeUnits"), "CPUAndGPU");
    }
    for precision in OsakaPrecision::ALL {
        assert_eq!(coreml(&upscale(UpscaleVariant::Osaka(precision)), "MLComputeUnits"), "CPUAndGPU");
    }
}

#[test]
fn osakas_builder_level_reaches_tensorrt_as_three_while_another_models_reaches_it_as_five() {
    // That Osaka's declaration *arrives*, not merely that the overlay works: on a 12,940-node graph level 5 is 87
    // seconds of engine build for no runtime difference.
    let artifact = crate::models::test_support::kyoto();
    for precision in OsakaPrecision::ALL {
        let tensorrt =
            options(ExecutionProvider::TensorRt, &upscale(UpscaleVariant::Osaka(precision)).profile(), &artifact);
        assert_eq!(tensorrt["trt_builder_optimization_level"], "3", "at {precision:?}");
        // The two absences that are decisions rather than omissions, and the override names one key only.
        assert_eq!(tensorrt["trt_fp16_enable"], "0", "at {precision:?}");
        assert_eq!(tensorrt["trt_int8_enable"], "0", "at {precision:?}");
        assert_eq!(tensorrt["trt_engine_hw_compatible"], "0", "at {precision:?}");
    }
    let kyoto = upscale(UpscaleVariant::Kyoto(FloatPrecision::Fp16)).profile();
    assert_eq!(options(ExecutionProvider::TensorRt, &kyoto, &artifact)["trt_builder_optimization_level"], "5");
}

#[test]
fn osakas_and_tokyos_session_settings_are_what_their_graphs_need() {
    // Osaka's is correctness rather than tuning: without `SimplifiedLayerNormFusion` off the transformer does not load
    // at all. Session settings apply on every provider, the CPU included.
    let (parallel, mem_pattern, disabled) =
        session_settings(&upscale(UpscaleVariant::Osaka(OsakaPrecision::Fp16)).profile());
    assert!(!parallel && !mem_pattern);
    assert_eq!(disabled, "ReshapeFusion;SimplifiedLayerNormFusion");

    let (parallel, ..) = session_settings(&upscale(UpscaleVariant::Tokyo(FloatPrecision::Fp16)).profile());
    assert!(!parallel, "Tokyo measured 18-27% from running sequentially");
}
