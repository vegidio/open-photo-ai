//! Where what an execution provider compiles from a model is kept, and the two events that discard it.
//!
//! Slice 2 took the two directories as arguments and decided nothing about them; this is the slice that first
//! compiles an engine, so it is where they are. Under the configuration directory, beside `models/` and `runtime/`:
//!
//! ```text
//! <config>/engines/<artifact-id>/     what a provider compiled from that model
//! <config>/engines/.timing/           the one TensorRT timing cache, shared by every model
//! <config>/engines/.version           the runtime release those artifacts were compiled under
//! ```
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

use crate::config;
use crate::deps::manifest;
use crate::deps::model::descriptor::{ENGINES_DIR, engine_dir};
use crate::error::InitError;
use crate::models::ArtifactId;
use crate::providers::options::CachePaths;

/// The installation-wide TensorRT timing cache, shared by every model.
///
/// Spelled out rather than composed from [`ENGINES_DIR`], because a `const` cannot format — the test below is what
/// keeps it inside the tree, which is the half of its placement that matters.
const TIMING_DIR: &str = "engines/.timing";

/// The record of which ONNX Runtime release the tree was compiled under.
///
/// A file rather than an inference from "the runtime was just replaced", because the latter is not durable: a process
/// that died between replacing the runtime and discarding the engines would find a matching runtime on its next
/// start, skip the reinstall, and never discard them.
const VERSION_STAMP: &str = ".version";

/// Resolves and creates the two directories the execution providers are pointed at for `id`.
///
/// Both are created on demand through [`config::sub_dir`], so the first session built for a model is what brings its
/// directory into existence and a machine that has never run one carries nothing.
///
/// # Errors
///
/// Returns [`InitError::InvalidName`] if `name` cannot be a directory, [`InitError::Fs`] if either path could escape
/// its parent, and [`InitError::Io`] if either could not be created.
pub(crate) fn resolve(app_dir: &Path, name: &str, id: &ArtifactId) -> Result<CachePaths, InitError> {
    Ok(CachePaths {
        engine: config::sub_dir(app_dir, name, &engine_dir(id))?,
        timing: config::sub_dir(app_dir, name, TIMING_DIR)?,
    })
}

/// The engines tree itself, resolved and created.
///
/// # Errors
///
/// As [`resolve`].
fn engines_root(app_dir: &Path, name: &str) -> Result<PathBuf, InitError> {
    config::sub_dir(app_dir, name, ENGINES_DIR)
}

/// Empties the engines tree where it was compiled under a runtime release other than `tag`, and records `tag`.
///
/// The wholesale invalidation of the two: what a provider compiles is valid only for the runtime that built it, and a
/// stale artifact is at best a wasted rebuild and at worst loaded against a runtime that cannot use it. It belongs to
/// initialization because that is where the runtime version in force is decided.
///
/// A launch whose stamp already names `tag` discards nothing, which is the point — these artifacts cost minutes to
/// rebuild. A machine that has never compiled anything writes the stamp over an empty directory and completes. No
/// model's weights are under this tree, so none is touched.
///
/// # Errors
///
/// Returns [`InitError::Io`] if the tree cannot be emptied or the stamp cannot be written, and otherwise as
/// [`resolve`].
pub(crate) fn invalidate_for_runtime(app_dir: &Path, name: &str, tag: &str) -> Result<(), InitError> {
    let engines = engines_root(app_dir, name)?;
    let stamp = engines.join(VERSION_STAMP);

    // Unreadable, absent and malformed all read the same way — as "compiled under something else" — which costs a
    // rebuild rather than trusting a stamp nothing wrote.
    let recorded = std::fs::read_to_string(&stamp).ok();
    if recorded.as_deref() == Some(tag) {
        return Ok(());
    }

    // Counted before the tree goes, and the stamp itself does not count: a launch that empties an empty tree — a
    // machine that has never compiled anything, or one whose stamp alone was stale — has discarded nothing and has
    // nothing to account for. The record exists for the minutes a rebuild costs, not for the bookkeeping.
    let discarded = std::fs::read_dir(&engines)
        .map(|entries| entries.flatten().filter(|entry| entry.file_name() != VERSION_STAMP).count())
        .unwrap_or_default();

    // The stamp lives in the tree it describes, so it goes with everything else and is written back afterwards. That
    // ordering is what makes an interruption in between read as "compiled under something else" on the next launch.
    manifest::empty_dir(&engines).map_err(InitError::io(&engines))?;

    if discarded > 0 {
        // `info`, naming the tag that replaced the one they were built under — which is the whole of the answer to
        // "why did this launch spend four minutes compiling engines it had already compiled".
        tracing::info!(
            discarded,
            tag,
            previous = recorded.as_deref().unwrap_or("unrecorded"),
            "discarded every compiled engine: the inference runtime they were built under has been replaced"
        );
    }

    std::fs::write(&stamp, tag).map_err(InitError::io(&stamp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging;
    use crate::models::test_support::{kyoto, tokyo};
    use tempfile::TempDir;

    /// The application name and directory every test here resolves against.
    const NAME: &str = "opai-test";

    /// An application directory to resolve beneath, and the temporary root keeping it alive.
    fn app() -> (TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let app_dir = root.path().join(NAME);

        (root, app_dir)
    }

    #[test]
    fn two_artifacts_are_pointed_at_two_directories() {
        // What a provider compiled from one model's weights must never be attributed to or reused against another's,
        // which is a property of the directories rather than of anything a provider is asked to do.
        let (_root, app_dir) = app();

        let first = resolve(&app_dir, NAME, &kyoto()).unwrap();
        let second = resolve(&app_dir, NAME, &tokyo()).unwrap();

        assert_ne!(first.engine, second.engine);
        assert_eq!(first.engine, app_dir.join(ENGINES_DIR).join(kyoto().as_str()));
        assert!(first.engine.is_dir() && second.engine.is_dir(), "a resolved directory was not created");
    }

    #[test]
    fn both_models_are_pointed_at_one_timing_cache_inside_the_engines_tree() {
        // Shared because measured kernel timings carry between graphs, and inside the tree because a runtime change
        // has to take it with everything else — a timing cache records tactics for one TensorRT version.
        let (_root, app_dir) = app();

        let first = resolve(&app_dir, NAME, &kyoto()).unwrap();
        let second = resolve(&app_dir, NAME, &tokyo()).unwrap();

        assert_eq!(first.timing, second.timing);
        assert!(
            first.timing.starts_with(app_dir.join(ENGINES_DIR)),
            "{} escaped the tree",
            first.timing.display()
        );
        assert!(first.timing.is_dir(), "the timing directory was not created");
        assert!(TIMING_DIR.starts_with(&format!("{ENGINES_DIR}/")), "{TIMING_DIR} is not inside {ENGINES_DIR}");
    }

    #[test]
    fn resolving_a_second_time_creates_nothing_and_disturbs_nothing() {
        // The session builder resolves these on every build that misses the cache, so resolution has to be something
        // an engine compiled minutes ago survives.
        let (_root, app_dir) = app();
        let id = kyoto();

        let paths = resolve(&app_dir, NAME, &id).unwrap();
        let compiled = paths.engine.join("engine.bin");
        std::fs::write(&compiled, b"what a provider compiled").unwrap();

        let again = resolve(&app_dir, NAME, &id).unwrap();

        assert_eq!(again, paths);
        assert!(compiled.is_file(), "resolving a second time discarded what was compiled");
    }

    #[test]
    fn a_bumped_runtime_release_empties_the_whole_tree() {
        let (_root, app_dir) = app();
        let paths = resolve(&app_dir, NAME, &kyoto()).unwrap();
        let engine = paths.engine.join("engine.bin");
        let timing = paths.timing.join("timings.bin");
        std::fs::write(&engine, b"compiled under the old runtime").unwrap();
        std::fs::write(&timing, b"measured under the old runtime").unwrap();

        invalidate_for_runtime(&app_dir, NAME, "runtime/1.26.0").unwrap();
        invalidate_for_runtime(&app_dir, NAME, "runtime/1.27.0").unwrap();

        assert!(!engine.exists(), "a compiled engine survived a runtime bump");
        assert!(!timing.exists(), "the shared timing cache survived a runtime bump");
        assert!(!paths.engine.exists(), "the model's directory was kept rather than discarded with the tree");
        let stamp = app_dir.join(ENGINES_DIR).join(VERSION_STAMP);
        assert_eq!(std::fs::read_to_string(stamp).unwrap(), "runtime/1.27.0");
    }

    #[test]
    fn a_launch_under_the_recorded_release_discards_nothing() {
        // The common case, and the one that must cost nothing: these artifacts take minutes to rebuild.
        let (_root, app_dir) = app();

        // Stamped first, then compiled: the order a machine actually reaches this state in, since the launch that
        // records the release precedes every session built under it.
        invalidate_for_runtime(&app_dir, NAME, "runtime/1.26.0").unwrap();
        let paths = resolve(&app_dir, NAME, &kyoto()).unwrap();
        let engine = paths.engine.join("engine.bin");
        std::fs::write(&engine, b"compiled under the recorded runtime").unwrap();

        invalidate_for_runtime(&app_dir, NAME, "runtime/1.26.0").unwrap();

        assert!(engine.is_file(), "a launch with the same runtime discarded a compiled engine");
    }

    #[test]
    fn a_machine_that_has_never_compiled_anything_records_the_release_and_completes() {
        // A first run: there is no tree, no stamp and nothing to discard. Emptying a directory that is already empty
        // is what "absent reads as compiled under something else" costs here.
        let (_root, app_dir) = app();

        invalidate_for_runtime(&app_dir, NAME, "runtime/1.26.0").unwrap();

        let stamp = app_dir.join(ENGINES_DIR).join(VERSION_STAMP);
        assert_eq!(std::fs::read_to_string(stamp).unwrap(), "runtime/1.26.0");
    }

    #[test]
    fn discarding_the_compiled_artifacts_leaves_every_installed_model_in_place() {
        // The weights are under `models/`, which is why they are a different tree: a runtime bump costs a rebuild,
        // never a re-download.
        let (_root, app_dir) = app();
        let models = config::sub_dir(&app_dir, NAME, "models/up_kyoto_4x_fp32").unwrap();
        let weights = models.join("up_kyoto_4x_fp32.onnx");
        std::fs::write(&weights, b"the published graph").unwrap();
        resolve(&app_dir, NAME, &kyoto()).unwrap();

        invalidate_for_runtime(&app_dir, NAME, "runtime/1.27.0").unwrap();

        assert!(weights.is_file(), "a runtime bump removed an installed model");
    }

    #[test]
    fn a_bumped_runtime_release_says_what_it_discarded_and_what_replaced_it() {
        // Alongside the filesystem effect, not instead of it: the record exists because a user pays minutes for the
        // rebuild it causes, and nothing on screen says why.
        let (_root, app_dir) = app();
        let paths = resolve(&app_dir, NAME, &kyoto()).unwrap();
        std::fs::write(paths.engine.join("engine.bin"), b"compiled under the old runtime").unwrap();
        invalidate_for_runtime(&app_dir, NAME, "runtime/1.26.0").unwrap();
        resolve(&app_dir, NAME, &kyoto()).unwrap();
        std::fs::write(paths.engine.join("engine.bin"), b"compiled under the old runtime").unwrap();

        let (log, ()) = logging::records_of_blocking("info", || {
            invalidate_for_runtime(&app_dir, NAME, "runtime/1.27.0").unwrap();
        });

        let discarded = log
            .lines()
            .find(|line| line.contains("the inference runtime they were built under has been replaced"))
            .unwrap_or_else(|| panic!("the discard was not recorded:\n{log}"));

        assert!(discarded.contains("level=INFO"), "{discarded}");
        assert!(
            discarded.contains("tag=runtime/1.27.0"),
            "the record does not name what replaced it: {discarded}"
        );
        assert!(discarded.contains("previous=runtime/1.26.0"), "{discarded}");
        assert!(!paths.engine.exists(), "a compiled engine survived a runtime bump");
    }

    #[test]
    fn a_launch_that_discarded_nothing_writes_no_record() {
        // Two ways to reach an empty tree, and neither is work about to be redone: a machine that has never compiled
        // anything, and a launch under the release already recorded.
        let (_root, first) = app();
        let (fresh, ()) = logging::records_of_blocking("info", || {
            invalidate_for_runtime(&first, NAME, "runtime/1.26.0").unwrap();
        });
        assert!(
            !fresh.contains("has been replaced"),
            "a first launch claimed to have discarded engines:\n{fresh}"
        );

        let (_root, second) = app();
        invalidate_for_runtime(&second, NAME, "runtime/1.26.0").unwrap();
        let paths = resolve(&second, NAME, &kyoto()).unwrap();
        std::fs::write(paths.engine.join("engine.bin"), b"compiled under the recorded runtime").unwrap();

        let (unchanged, ()) = logging::records_of_blocking("info", || {
            invalidate_for_runtime(&second, NAME, "runtime/1.26.0").unwrap();
        });
        assert!(!unchanged.contains("has been replaced"), "{unchanged}");
    }
}
