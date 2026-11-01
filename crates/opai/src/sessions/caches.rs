//! Where what an execution provider compiles from a model is kept, and the two events that discard it.
//!
//! Under the configuration directory, beside `models/`:
//!
//! ```text
//! <config>/engines/<artifact-id>/tensorrt/   the TensorRT engine compiled from that model
//! <config>/engines/<artifact-id>/coreml/     the CoreML model compiled from it
//! <config>/engines/.timing/                  the one TensorRT timing cache, shared by every model
//! <config>/engines/.version                  the runtime release and the layout those were written under
//! ```
//!
//! One folder per provider inside a model's own: each provider owns its directory, so one can be cleared or inspected
//! without touching the other, and two providers' file names can never collide. `rust-sak` creates a provider's folder
//! when a rung that attaches it is tried, so a model only ever run on the CPU leaves none behind.
//!
//! Not inside `models/<artifact-id>/`, which was the first idea and does not survive contact with the install
//! pipeline: it finishes by recording **every file under the directory** as something it downloaded, and a
//! replacement removes exactly the files the *previous* record named — so an engine compiled after that record was
//! written would survive the one replacement that must not leave it.
//!
//! `.timing` sits **inside** `engines/` rather than beside it, which is what gives it the right lifetime at both
//! ends: replacing one model's weights clears that model's subdirectory and the shared cache survives, while a
//! runtime change empties the tree wholesale and takes the timing cache with it — which is correct, because a timing
//! cache records tactics for one TensorRT version and TensorRT comes from the runtime.
//!
//! One function resolves a model's engine directory and every caller goes through it: the descriptor that records it
//! as derived from the weights, the installer that clears it on a replacement, and the session builder that points a
//! provider at it. They cannot disagree about which directory it is. That function lives beside `model_dir` in
//! `deps/model/descriptor.rs` rather than here, because `deps/` needs it too and sits below this layer — the layout
//! decisions above are still this module's.

use std::path::{Path, PathBuf};

use rust_sak::ai::CachePaths;

use crate::config;
use crate::deps::manifest;
use crate::deps::model::descriptor::{ENGINES_DIR, engine_dir};
use crate::error::InitError;
use crate::models::ArtifactId;

/// The installation-wide TensorRT timing cache, shared by every model.
///
/// Spelled out rather than composed from [`ENGINES_DIR`], because a `const` cannot format — the test below is what
/// keeps it inside the tree, which is the half of its placement that matters.
const TIMING_DIR: &str = "engines/.timing";

/// The record of which ONNX Runtime release, and which layout, the tree was written under.
///
/// A file rather than an inference from "the runtime was just replaced", because the latter is not durable: a process
/// that died between replacing the runtime and discarding the engines would find a matching runtime on its next
/// start, skip the reinstall, and never discard them.
const VERSION_STAMP: &str = ".version";

/// The layout this build writes the tree in, as the stamp records it after the runtime's version.
///
/// `2` is a folder per provider inside each model's. A stamp naming no layout is the flat `engines/<artifact-id>/` of
/// earlier releases, which nothing reads any more. A later layout change is a bump here.
const LAYOUT: &str = "layout=2";

/// The stamp a tree written under `runtime_version` in this build's layout carries.
fn stamp_for(runtime_version: &str) -> String {
    format!("{runtime_version};{LAYOUT}")
}

/// The folders the execution providers are pointed at for `id`: its TensorRT and CoreML folders inside its own, and
/// the timing cache every model shares. Computes the paths only; see the module documentation for who creates them.
///
/// # Errors
///
/// Returns [`InitError::InvalidName`] if `name` cannot be a directory, [`InitError::Fs`] if the tree could escape its
/// parent, and [`InitError::Io`] if the tree itself could not be created.
pub(crate) fn resolve(app_dir: &Path, name: &str, id: &ArtifactId) -> Result<CachePaths, InitError> {
    // The tree is validated and created through the one rule every configuration path goes through; what is inside it
    // is named by an artifact id, which cannot escape it.
    engines_root(app_dir, name)?;

    Ok(CachePaths { timing: app_dir.join(TIMING_DIR), ..CachePaths::under(&app_dir.join(engine_dir(id))) })
}

/// The engines tree itself, resolved and created.
///
/// # Errors
///
/// As [`resolve`].
fn engines_root(app_dir: &Path, name: &str) -> Result<PathBuf, InitError> {
    config::sub_dir(app_dir, name, ENGINES_DIR)
}

/// Empties the engines tree where it was written under a runtime release other than `runtime_version`, or in a layout
/// other than this build's, and records both.
///
/// The wholesale invalidation of the two: what a provider compiles is valid only for the runtime that built it, and a
/// stale artifact is at best a wasted rebuild and at worst loaded against a runtime that cannot use it. Artifacts in a
/// layout nothing reads any more would never be reused and never removed by anything else. It belongs to
/// initialization because that is where the runtime version in force is decided.
///
/// A launch whose stamp already names both discards nothing, which is the point — these artifacts cost minutes to
/// rebuild. A machine that has never compiled anything writes the stamp over an empty directory and completes. No
/// model's weights are under this tree, so none is touched.
///
/// # Errors
///
/// Returns [`InitError::Io`] if the tree cannot be emptied or the stamp cannot be written, and otherwise as
/// [`resolve`].
pub(crate) fn invalidate_for_runtime(app_dir: &Path, name: &str, runtime_version: &str) -> Result<(), InitError> {
    let engines = engines_root(app_dir, name)?;
    let stamp = engines.join(VERSION_STAMP);
    let expected = stamp_for(runtime_version);

    // Unreadable, absent and malformed all read the same way — as "written under something else" — which costs a
    // rebuild rather than trusting a stamp nothing wrote. A stamp naming no layout is an earlier release's.
    let recorded = std::fs::read_to_string(&stamp).ok();
    if recorded.as_deref() == Some(expected.as_str()) {
        return Ok(());
    }

    // Counted before the tree goes, and the stamp itself does not count: a launch that empties an empty tree — a
    // machine that has never compiled anything, or one whose stamp alone was stale — has discarded nothing and has
    // nothing to account for. The record exists for the minutes a rebuild costs, not for the bookkeeping.
    let discarded = std::fs::read_dir(&engines)
        .map(|entries| entries.flatten().filter(|entry| entry.file_name() != VERSION_STAMP).count())
        .unwrap_or_default();

    // The stamp lives in the tree it describes, so it goes with everything else and is written back afterwards. That
    // ordering is what makes an interruption in between read as "written under something else" on the next launch.
    manifest::empty_dir(&engines).map_err(InitError::io(&engines))?;

    if discarded > 0 {
        // `info`, naming what replaced what they were built under — which is the whole of the answer to "why did this
        // launch spend four minutes compiling engines it had already compiled".
        tracing::info!(
            discarded,
            stamp = %expected,
            previous = recorded.as_deref().unwrap_or("unrecorded"),
            "discarded every compiled engine: the inference runtime they were built under, or the layout they were \
             kept in, has changed"
        );
    }

    std::fs::write(&stamp, &expected).map_err(InitError::io(&stamp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging;
    use crate::models::test_support::{kyoto, tokyo};
    use tempfile::TempDir;

    /// The application name and directory every test here resolves against.
    const NAME: &str = "opai-test";

    /// The runtime version every test here is stamped under.
    const RUNTIME: &str = "1.30.0";

    /// An application directory to resolve beneath, and the temporary root keeping it alive.
    fn app() -> (TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let app_dir = root.path().join(NAME);

        (root, app_dir)
    }

    /// `paths` with something compiled into each provider's folder and into the shared timing cache, as a machine that
    /// ran the model on both would have them. Returns the files written.
    fn compile_into(paths: &CachePaths) -> [PathBuf; 3] {
        let files = [
            paths.tensorrt.join("engine.bin"),
            paths.coreml.join("model.mlmodelc"),
            paths.timing.join("t.cache"),
        ];
        for file in &files {
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, b"compiled").unwrap();
        }
        files
    }

    /// What the stamp says now.
    fn stamp(app_dir: &Path) -> String {
        std::fs::read_to_string(app_dir.join(ENGINES_DIR).join(VERSION_STAMP)).unwrap()
    }

    #[test]
    fn each_provider_compiles_into_a_folder_of_its_own_inside_the_models() {
        let (_root, app_dir) = app();
        let paths = resolve(&app_dir, NAME, &kyoto()).unwrap();
        let model = app_dir.join(ENGINES_DIR).join(kyoto().as_str());

        assert_eq!(paths.tensorrt, model.join("tensorrt"));
        assert_eq!(paths.coreml, model.join("coreml"));
        assert_ne!(paths.tensorrt, paths.coreml);
        assert!(paths.tensorrt.starts_with(&model) && paths.coreml.starts_with(&model));
    }

    #[test]
    fn two_artifacts_are_pointed_at_two_directories() {
        // What a provider compiled from one model's weights must never be attributed to or reused against another's,
        // which is a property of the directories rather than of anything a provider is asked to do.
        let (_root, app_dir) = app();

        let first = resolve(&app_dir, NAME, &kyoto()).unwrap();
        let second = resolve(&app_dir, NAME, &tokyo()).unwrap();

        assert_ne!(first.tensorrt, second.tensorrt);
        assert_ne!(first.coreml, second.coreml);
    }

    #[test]
    fn both_models_are_pointed_at_one_timing_cache_inside_the_engines_tree_and_outside_either_model() {
        // Shared because measured kernel timings carry between graphs, and inside the tree because a runtime change
        // has to take it with everything else — a timing cache records tactics for one TensorRT version.
        let (_root, app_dir) = app();

        let first = resolve(&app_dir, NAME, &kyoto()).unwrap();
        let second = resolve(&app_dir, NAME, &tokyo()).unwrap();

        assert_eq!(first.timing, second.timing);
        assert_eq!(first.timing, app_dir.join(TIMING_DIR));
        assert!(
            first.timing.starts_with(app_dir.join(ENGINES_DIR)),
            "{} escaped the tree",
            first.timing.display()
        );
        for model in [kyoto(), tokyo()] {
            let own = app_dir.join(ENGINES_DIR).join(model.as_str());
            assert!(!first.timing.starts_with(&own), "the timing cache is inside {}", own.display());
        }
        assert!(TIMING_DIR.starts_with(&format!("{ENGINES_DIR}/")), "{TIMING_DIR} is not inside {ENGINES_DIR}");
    }

    #[test]
    fn resolving_creates_no_provider_folder_and_disturbs_nothing_compiled() {
        // The provider folders are `rust-sak`'s to create when a rung that compiles is tried, so a model only run on
        // the CPU leaves nothing; and the session builder resolves on every build that misses the cache, so
        // resolution has to be something an engine compiled minutes ago survives.
        let (_root, app_dir) = app();
        let paths = resolve(&app_dir, NAME, &kyoto()).unwrap();
        assert!(!paths.tensorrt.exists() && !paths.coreml.exists() && !paths.timing.exists());

        let compiled = compile_into(&paths);
        assert_eq!(resolve(&app_dir, NAME, &kyoto()).unwrap(), paths);
        assert!(
            compiled.iter().all(|file| file.is_file()),
            "resolving a second time discarded what was compiled"
        );
    }

    #[test]
    fn a_bumped_runtime_release_empties_the_whole_tree() {
        let (_root, app_dir) = app();
        invalidate_for_runtime(&app_dir, NAME, "1.26.0").unwrap();
        let paths = resolve(&app_dir, NAME, &kyoto()).unwrap();
        let compiled = compile_into(&paths);

        invalidate_for_runtime(&app_dir, NAME, "1.27.0").unwrap();

        assert!(compiled.iter().all(|file| !file.exists()), "something compiled survived a runtime bump");
        assert!(!app_dir.join(engine_dir(&kyoto())).exists(), "the model's directory was kept");
        assert_eq!(stamp(&app_dir), "1.27.0;layout=2");
    }

    #[test]
    fn a_stamp_naming_no_layout_discards_the_flat_engines_once() {
        // The stamp an earlier release wrote: the runtime's version alone, over engines kept flat in each model's
        // folder. Same runtime, so only the layout differs.
        let (_root, app_dir) = app();
        let flat = app_dir.join(engine_dir(&kyoto())).join("engine.bin");
        std::fs::create_dir_all(flat.parent().unwrap()).unwrap();
        std::fs::write(&flat, b"compiled in the flat layout").unwrap();
        let timing = app_dir.join(TIMING_DIR).join("t.cache");
        std::fs::create_dir_all(timing.parent().unwrap()).unwrap();
        std::fs::write(&timing, b"measured").unwrap();
        std::fs::write(app_dir.join(ENGINES_DIR).join(VERSION_STAMP), RUNTIME).unwrap();

        invalidate_for_runtime(&app_dir, NAME, RUNTIME).unwrap();
        assert!(!flat.exists(), "an engine in the flat layout survived");
        assert!(!timing.exists(), "the shared timing cache survived a layout change");
        assert_eq!(stamp(&app_dir), "1.30.0;layout=2");

        let compiled = compile_into(&resolve(&app_dir, NAME, &kyoto()).unwrap());
        invalidate_for_runtime(&app_dir, NAME, RUNTIME).unwrap();
        assert!(compiled.iter().all(|file| file.is_file()), "the second launch discarded again");
    }

    #[test]
    fn a_stamp_naming_another_layout_empties_the_whole_tree() {
        // Same runtime, a layout this build does not write: what a later layout bump, or a rollback, finds.
        let (_root, app_dir) = app();
        let compiled = compile_into(&resolve(&app_dir, NAME, &kyoto()).unwrap());
        std::fs::write(app_dir.join(ENGINES_DIR).join(VERSION_STAMP), format!("{RUNTIME};layout=1")).unwrap();

        invalidate_for_runtime(&app_dir, NAME, RUNTIME).unwrap();

        assert!(compiled.iter().all(|file| !file.exists()), "something compiled survived a layout change");
        assert_eq!(stamp(&app_dir), "1.30.0;layout=2");
    }

    #[test]
    fn a_launch_under_the_recorded_release_and_layout_discards_nothing() {
        // The common case, and the one that must cost nothing: these artifacts take minutes to rebuild.
        let (_root, app_dir) = app();
        invalidate_for_runtime(&app_dir, NAME, RUNTIME).unwrap();
        let compiled = compile_into(&resolve(&app_dir, NAME, &kyoto()).unwrap());

        invalidate_for_runtime(&app_dir, NAME, RUNTIME).unwrap();

        assert!(compiled.iter().all(|file| file.is_file()), "a launch with nothing changed discarded something");
    }

    #[test]
    fn a_machine_that_has_never_compiled_anything_records_the_release_and_layout_and_completes() {
        let (_root, app_dir) = app();

        invalidate_for_runtime(&app_dir, NAME, RUNTIME).unwrap();

        assert_eq!(stamp(&app_dir), "1.30.0;layout=2");
    }

    #[test]
    fn discarding_the_compiled_artifacts_leaves_every_installed_model_in_place() {
        // The weights are under `models/`, which is why they are a different tree: a runtime bump or a layout change
        // costs a rebuild, never a re-download.
        let (_root, app_dir) = app();
        let models = config::sub_dir(&app_dir, NAME, "models/up_kyoto_4x_fp32").unwrap();
        let weights = models.join("up_kyoto_4x_fp32.onnx");
        std::fs::write(&weights, b"the published graph").unwrap();
        compile_into(&resolve(&app_dir, NAME, &kyoto()).unwrap());

        std::fs::write(app_dir.join(ENGINES_DIR).join(VERSION_STAMP), RUNTIME).unwrap();
        invalidate_for_runtime(&app_dir, NAME, RUNTIME).unwrap();
        invalidate_for_runtime(&app_dir, NAME, "1.31.0").unwrap();

        assert!(weights.is_file(), "discarding the engines removed an installed model");
    }

    #[test]
    fn a_discard_says_what_it_discarded_and_what_replaced_it() {
        // Alongside the filesystem effect, not instead of it: the record exists because a user pays minutes for the
        // rebuild it causes, and nothing on screen says why.
        let (_root, app_dir) = app();
        compile_into(&resolve(&app_dir, NAME, &kyoto()).unwrap());
        std::fs::write(app_dir.join(ENGINES_DIR).join(VERSION_STAMP), RUNTIME).unwrap();

        let (log, ()) = logging::records_of_blocking("info", || {
            invalidate_for_runtime(&app_dir, NAME, RUNTIME).unwrap();
        });

        let discarded = log
            .lines()
            .find(|line| line.contains("discarded every compiled engine"))
            .unwrap_or_else(|| panic!("the discard was not recorded:\n{log}"));

        assert!(discarded.contains("level=INFO"), "{discarded}");
        assert!(discarded.contains("or the layout they were kept in, has changed"), "{discarded}");
        assert!(
            discarded.contains(r#"stamp="1.30.0;layout=2""#),
            "the record does not name what replaced it: {discarded}"
        );
        assert!(discarded.contains("previous=1.30.0"), "{discarded}");
    }

    #[test]
    fn a_launch_that_discarded_nothing_writes_no_record() {
        // Two ways to reach an empty tree, and neither is work about to be redone: a machine that has never compiled
        // anything, and a launch under the stamp already recorded.
        let (_root, first) = app();
        let (fresh, ()) = logging::records_of_blocking("info", || {
            invalidate_for_runtime(&first, NAME, RUNTIME).unwrap();
        });
        assert!(!fresh.contains("discarded every compiled engine"), "a first launch claimed a discard:\n{fresh}");

        let (_root, second) = app();
        invalidate_for_runtime(&second, NAME, RUNTIME).unwrap();
        compile_into(&resolve(&second, NAME, &kyoto()).unwrap());

        let (unchanged, ()) = logging::records_of_blocking("info", || {
            invalidate_for_runtime(&second, NAME, RUNTIME).unwrap();
        });
        assert!(!unchanged.contains("discarded every compiled engine"), "{unchanged}");
    }
}
