//! Getting a named model's files onto disk and proving they are the published ones.
//!
//! Nothing here opens a graph. This is the seam between `models/`, which knows what a model file is *called*, and
//! `deps/`, which knows how to put a verified file on disk — so code that runs inference takes the weights as already
//! present rather than as something it has to fetch.
//!
//! A model's expected SHA-256 is resolved from the first of three sources that answers, and a fourth outcome that
//! refuses:
//!
//! 1. the live Hugging Face tree listing, where a file's Git LFS object id *is* the SHA-256 of its contents;
//! 2. the copy cached from the last successful read of it, written into the models directory;
//! 3. the listing compiled into this binary, covering every model this build can name that was published when it was
//!    built;
//! 4. otherwise [`InitError::UnpublishedModel`], naming the artifact, with **nothing transferred**.
//!
//! There is no path here that places a model file on disk without checking it.
//!
//! The listing is read **lazily** — the first install that needs it, never during [`crate::Opai::initialize`] — and
//! memoized for the process, so a chain of installs costs one round trip and `initialize` has no network dependency.
//!
//! Each artifact installs into a directory of its own, `models/<artifact-id>/`.

// The reference implementation has a fifth step — compose the URL anyway and install the file with an empty expected
// hash — and reaches it from an ordinary timeout. Step 3 removes the case for every model that existed when the binary
// was built, so refusing the remainder costs only a model published *afterwards* on a machine that cannot reach the
// listing; and such a model is not published for that binary in any useful sense, so the URL would 404 regardless.

pub(crate) mod descriptor;
pub(crate) mod manifest;

#[cfg(test)]
pub(crate) mod generate;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config;
use crate::deps::model::descriptor::{MODEL_BASE_URL, MODELS_DIR, descriptor_at};
use crate::deps::model::manifest::{Listing, TREE_URL};
use crate::deps::{Installing, ModelTrust};
use crate::error::InitError;
use crate::models::ArtifactId;
use crate::progress::Reporter;

/// Installs every published file backing `id` and returns the directory they landed in.
///
/// Resolves the published listing — lazily, and at most once per process — then installs through the same pipeline the
/// release archives use. See the module documentation for the three sources and the refusal.
///
/// # Errors
///
/// Returns [`InitError::UnpublishedModel`] when no source has a hash for `id`, with nothing transferred. Otherwise as
/// [`crate::deps::install::install`]: [`InitError::Download`] if a transfer fails, [`InitError::HashMismatch`] if a
/// file's bytes are not the published ones, and [`InitError::Io`] or [`InitError::Fs`] for the bookkeeping and the
/// directories around them.
pub(crate) async fn install(
    app_dir: &Path,
    name: &str,
    id: &ArtifactId,
    trust: ModelTrust,
    installing: &Installing,
) -> Result<PathBuf, InitError> {
    // The models directory itself, which is where the cached listing lives; see `manifest::CACHE_NAME`.
    let models_dir = config::sub_dir(app_dir, name, MODELS_DIR)?;
    let listing = Listing::resolve(TREE_URL, &models_dir).await;

    install_from(MODEL_BASE_URL, &listing, app_dir, name, id, trust, installing).await
}

/// The testable half of [`install`]: the same install against an explicit base URL and an explicit listing.
///
/// # Errors
///
/// As [`install`].
pub(crate) async fn install_from(
    base_url: &str,
    listing: &Listing,
    app_dir: &Path,
    name: &str,
    id: &ArtifactId,
    trust: ModelTrust,
    installing: &Installing,
) -> Result<PathBuf, InitError> {
    // The listing is a parameter rather than resolved here because `Listing::resolve` memoizes for the *process* — the
    // suite has to be able to drive each of the three sources, and a memoized resolution would hand every test
    // whatever the first one resolved.
    //
    // Before any directory is created and before a byte moves: an artifact no source names has no URL worth
    // requesting, so the refusal costs nothing and says which model it was.
    //
    // **A refusal, and the divergence is deliberate.** The reference *warns* here and then downloads the unknown
    // model unverified; `model-install`'s specification already fixes this project's refusal instead. The level is
    // still a `warn`: the refusal is returned to the caller, and whether it is fatal is the caller's to decide (see
    // `logging` for the rule).
    let Some(dependency) = descriptor_at(base_url, listing, id, app_dir, trust) else {
        tracing::warn!(
            artifact = %id,
            "no published source names this model; refusing to install it rather than transferring it unverified"
        );

        return Err(InitError::UnpublishedModel { artifact: id.as_str().to_string() });
    };

    let dir = config::sub_dir(app_dir, name, &dependency.dir)?;

    // One reporter per model, as one per dependency for the archives: two installs sharing a reporter interleave into
    // a figure no front end could present. Built from the descriptor, so the expansion the bar assumes and the branch
    // the pipeline takes are read off the same value rather than by two callers that each remember to.
    let reporter = Arc::new(Reporter::for_dependency(&dependency, installing.on_progress.clone()));

    // No install records of its own. The shared pipeline already writes the three — installing, ready, failed — and
    // a model's descriptor carries the **artifact id** as its name, so they come out as
    // `dependency=up_kyoto_4x_fp16` without anything here restating them. That is what makes one `grep` over
    // `opai.log` for an artifact return its install, its session build and every run that used it; a second set here
    // would be two answers to "when did this model install" separated by one call.
    //
    // **The outcome is discarded, and that is the model path's answer rather than an oversight.** A dependency whose
    // install was already current says so through its reporter, because a front end drawing a component list cannot
    // otherwise tell a finished row from one not yet reached. A model has no such list: it is installed the first time
    // an operation needs it, `model-install` fixes that an install already current reports nothing, and no consumer
    // wants otherwise.
    let _ = crate::deps::install::install(&dir, &dependency, reporter, &installing.cancel).await?;

    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps::install::TRANSFER_RETRIES;
    use crate::deps::manifest;
    use crate::deps::test_server::{FIXTURE, Reply, TestServer, fixture_listing};
    use crate::logging;
    use crate::models::test_support::kyoto;
    use crate::models::{FloatPrecision, Operation, Scale, Upscale, UpscaleVariant};
    use crate::progress::{self, Progress};
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// The artifact every test here installs, and the file it is published as.
    const KYOTO: &str = "up_kyoto_4x_fp32";

    /// The listing that publishes the 4x Kyoto graph alone.
    fn kyoto_listing() -> Listing {
        fixture_listing(&[format!("{KYOTO}.onnx")])
    }

    /// An application directory to install into, and the temporary root it lives under.
    fn app() -> (TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let app_dir = root.path().join("opai-test");

        (root, app_dir)
    }

    /// [`super::install_from`] against `server`, reporting nothing.
    async fn install_from(
        server: &TestServer,
        listing: &Listing,
        app_dir: &Path,
        id: &ArtifactId,
        trust: ModelTrust,
    ) -> Result<PathBuf, InitError> {
        // Shadowing the function under test, as `deps/install.rs` does, rather than threading five constant arguments
        // through two dozen call sites: the base URL, the application name and an `Installing` nothing reads are noise
        // at every one of them, and the two tests that *do* want progress say so by calling `super::install_from` with
        // an `Installing` of their own.
        super::install_from(&server.base_url, listing, app_dir, "opai-test", id, trust, &Installing::default()).await
    }

    /// A callback that collects every report it is handed, returned with the collection.
    fn recording() -> (Option<progress::OnProgress>, Arc<Mutex<Vec<Progress>>>) {
        let (on_progress, seen) = progress::recording();
        (Some(on_progress), seen)
    }

    #[tokio::test]
    async fn a_model_installs_its_published_file_into_a_directory_of_its_own() {
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let id = kyoto();
        let (on_progress, seen) = recording();

        let dir = super::install_from(
            &server.base_url,
            &kyoto_listing(),
            &app_dir,
            "opai-test",
            &id,
            ModelTrust::Published,
            &Installing { on_progress, ..Installing::default() },
        )
        .await
        .unwrap();

        assert_eq!(dir, app_dir.join("models").join(KYOTO));
        let graph = dir.join(format!("{KYOTO}.onnx"));
        assert!(graph.is_file(), "the graph is not on disk");
        assert_eq!(std::fs::metadata(&graph).unwrap().len(), FIXTURE.len() as u64);
        assert!(manifest::read(&dir).is_some(), "the install left no record");

        let reports = seen.lock().unwrap().clone();
        assert!(!reports.is_empty(), "the install reported nothing");
        assert!(reports.iter().all(|report| report.dependency == crate::Dependency::Model(id.clone())));
        assert_eq!(reports.last().unwrap().fraction, 1.0);
    }

    #[tokio::test]
    async fn an_artifact_absent_from_every_source_fails_naming_it_with_no_transfer_attempted() {
        // The refusal that replaces the reference implementation's unverified install. Nothing is requested, because
        // an artifact no listing names has no URL worth requesting.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();

        let error = install_from(&server, &Listing::default(), &app_dir, &kyoto(), ModelTrust::Published)
            .await
            .unwrap_err();

        match error {
            InitError::UnpublishedModel { artifact } => assert_eq!(artifact, KYOTO),
            other => panic!("expected UnpublishedModel, got {other:?}"),
        }
        assert_eq!(server.requests(), 0, "a transfer was attempted for an artifact with no published hash");
        assert!(!app_dir.join("models").join(KYOTO).exists(), "a refused install created a directory");
    }

    #[tokio::test]
    async fn a_served_file_whose_bytes_are_wrong_fails_as_a_mismatch_rather_than_as_a_transfer_failure() {
        // The two have to be distinguishable: a transfer failure is worth retrying, and bytes that do not hash are
        // either a corrupted mirror or a listing that no longer describes what is served.
        let server = TestServer::start(vec![Reply::Corrupt]).await;
        let (_root, app_dir) = app();

        let error = install_from(&server, &kyoto_listing(), &app_dir, &kyoto(), ModelTrust::Published)
            .await
            .unwrap_err();

        match error {
            InitError::HashMismatch { url, expected, actual } => {
                assert!(url.ends_with(&format!("{KYOTO}.onnx")), "{url}");
                assert_eq!(expected, rust_sak::crypto::sha256_bytes(FIXTURE));
                assert_ne!(actual, expected);
            }
            other => panic!("expected HashMismatch, got {other:?}"),
        }

        let dir = app_dir.join("models").join(KYOTO);
        assert!(!dir.join(format!("{KYOTO}.onnx")).exists(), "bytes that failed verification were left in place");
        assert!(manifest::read(&dir).is_none(), "a failed install left a record");
    }

    #[tokio::test]
    async fn a_transfer_that_never_completes_fails_as_a_download_rather_than_as_a_mismatch() {
        // The other half of the same distinction: nothing arrived, so there is nothing to have hashed wrong.
        let server = TestServer::start(vec![Reply::Truncated(0); 8]).await;
        let (_root, app_dir) = app();

        let error = install_from(&server, &kyoto_listing(), &app_dir, &kyoto(), ModelTrust::Published)
            .await
            .unwrap_err();

        assert!(matches!(error, InitError::Download { .. }), "got {error:?}");
    }

    #[tokio::test]
    async fn a_second_install_of_an_intact_model_transfers_nothing() {
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let listing = kyoto_listing();

        install_from(&server, &listing, &app_dir, &kyoto(), ModelTrust::Published).await.unwrap();
        let after_first = server.requests();

        let (on_progress, seen) = recording();
        let dir = super::install_from(
            &server.base_url,
            &listing,
            &app_dir,
            "opai-test",
            &kyoto(),
            ModelTrust::Published,
            &Installing { on_progress, ..Installing::default() },
        )
        .await
        .unwrap();

        assert_eq!(server.requests(), after_first, "the second install contacted the server");

        // **Nothing at all**, not even the already-in-place report a *dependency* makes on this path: a model is
        // installed the first time an operation needs it rather than drawn as a row of a component list, so there is
        // nobody to tell a finished row from an unreached one. The shared pipeline reports which of the two it was and
        // this caller discards it — see `deps::install::Outcome`.
        assert!(seen.lock().unwrap().is_empty(), "a model already installed reported something");
        assert!(dir.join(format!("{KYOTO}.onnx")).is_file());
    }

    #[tokio::test]
    async fn a_model_whose_files_were_removed_is_installed_again() {
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let listing = kyoto_listing();

        let dir = install_from(&server, &listing, &app_dir, &kyoto(), ModelTrust::Published).await.unwrap();
        let after_first = server.requests();
        std::fs::remove_file(dir.join(format!("{KYOTO}.onnx"))).unwrap();

        install_from(&server, &listing, &app_dir, &kyoto(), ModelTrust::Published).await.unwrap();

        assert!(server.requests() > after_first, "a missing graph was not fetched again");
        assert!(dir.join(format!("{KYOTO}.onnx")).is_file());
    }

    #[tokio::test]
    async fn an_install_interrupted_after_some_files_are_written_is_not_mistaken_for_complete() {
        // The first file arrives, every attempt at the second is cut short. The record is written only once every file
        // is in place, so the next attempt has to see a mid-install directory rather than a finished one.
        // One reply for the first file, then one for each attempt at the second — the initial request plus the
        // transfer's own retries — so the install gives up with the first file already written. Anything after that
        // falls through to the fixture, which is what the second install gets.
        let mut script = vec![Reply::Fixture];
        script.extend(std::iter::repeat_n(Reply::Truncated(FIXTURE.len() / 3), TRANSFER_RETRIES as usize + 1));
        let server = TestServer::start(script).await;
        let (_root, app_dir) = app();
        let listing = fixture_listing(&[format!("{KYOTO}.onnx"), format!("{KYOTO}.onnx.data")]);

        let interrupted = install_from(&server, &listing, &app_dir, &kyoto(), ModelTrust::Published).await;
        assert!(interrupted.is_err(), "a truncated transfer was accepted as an install");

        let dir = app_dir.join("models").join(KYOTO);
        assert!(dir.join(format!("{KYOTO}.onnx")).is_file(), "the first file did not survive the interruption");
        assert!(manifest::read(&dir).is_none(), "an unfinished install left a record");

        install_from(&server, &listing, &app_dir, &kyoto(), ModelTrust::Published).await.unwrap();

        assert!(dir.join(format!("{KYOTO}.onnx")).is_file());
        assert!(dir.join(format!("{KYOTO}.onnx.data")).is_file(), "the install did not run to completion");
        assert!(manifest::read(&dir).is_some());
    }

    #[tokio::test]
    async fn replacing_a_model_whose_published_files_changed_removes_only_that_models_previous_files() {
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let id = kyoto();

        // First published as a graph plus a weights blob...
        let split = fixture_listing(&[format!("{KYOTO}.onnx"), format!("{KYOTO}.onnx.data")]);
        let dir = install_from(&server, &split, &app_dir, &id, ModelTrust::Published).await.unwrap();
        assert!(dir.join(format!("{KYOTO}.onnx.data")).is_file());

        // ...and a second model installed beside it, which must not be touched by what follows.
        let other = Upscale::new(UpscaleVariant::Kyoto(FloatPrecision::Fp32), Scale::new(2.0).unwrap())
            .required_artifacts()
            .remove(0);
        let other_listing = fixture_listing(&[format!("{other}.onnx")]);
        let other_dir = install_from(&server, &other_listing, &app_dir, &other, ModelTrust::Published).await.unwrap();

        // Republished as a single file: the blob the previous version installed has to go.
        install_from(&server, &kyoto_listing(), &app_dir, &id, ModelTrust::Published).await.unwrap();

        assert!(dir.join(format!("{KYOTO}.onnx")).is_file());
        assert!(
            !dir.join(format!("{KYOTO}.onnx.data")).exists(),
            "the previous version's weights blob survived beside the new graph"
        );
        let recorded = manifest::read(&dir).unwrap();
        assert_eq!(
            recorded.files.iter().map(|file| file.path.as_str()).collect::<Vec<_>>(),
            vec![format!("{KYOTO}.onnx")],
            "the record names something other than what the new version installed"
        );

        // And the model beside it is untouched, which is what the directory-per-model layout is for.
        assert!(other_dir.join(format!("{other}.onnx")).is_file(), "another model's files were removed");
        assert!(manifest::read(&other_dir).is_some());
    }

    #[tokio::test]
    async fn two_models_installed_in_turn_both_remain_present() {
        // The two halves of one 8x Kyoto. Under the flat layout the reference uses, the second install would empty the
        // directory and delete the first — the other half of the same operation.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let required =
            Upscale::new(UpscaleVariant::Kyoto(FloatPrecision::Fp32), Scale::new(8.0).unwrap()).required_artifacts();
        assert_eq!(required.len(), 2, "an 8x Kyoto runs two passes");

        let mut dirs = Vec::new();
        for id in &required {
            let listing = fixture_listing(&[format!("{id}.onnx")]);
            dirs.push(install_from(&server, &listing, &app_dir, id, ModelTrust::Published).await.unwrap());
        }

        for (id, dir) in required.iter().zip(&dirs) {
            assert!(dir.join(format!("{id}.onnx")).is_file(), "{id} is not on disk after the other was installed");
            assert!(manifest::read(dir).is_some(), "{id} has no record");
        }
        assert_ne!(dirs[0], dirs[1], "the two models share a directory");
    }

    #[tokio::test]
    async fn a_model_published_as_two_files_installs_both_before_reporting_success() {
        // Both halves are transferred and each verified against its own hash; the record names both, so replacing the
        // model later removes both.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let listing = fixture_listing(&[format!("{KYOTO}.onnx"), format!("{KYOTO}.onnx.data")]);

        let dir = install_from(&server, &listing, &app_dir, &kyoto(), ModelTrust::Published).await.unwrap();

        assert!(dir.join(format!("{KYOTO}.onnx")).is_file(), "the graph is missing");
        assert!(dir.join(format!("{KYOTO}.onnx.data")).is_file(), "the weights blob is missing");

        let recorded = manifest::read(&dir).unwrap();
        let mut names: Vec<&str> = recorded.files.iter().map(|file| file.path.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, vec![format!("{KYOTO}.onnx"), format!("{KYOTO}.onnx.data")]);
    }

    /// Seeds `id`'s engine directory and the shared timing cache with what a provider would have compiled, and
    /// returns the two files.
    fn compiled(app_dir: &Path, id: &ArtifactId) -> (PathBuf, PathBuf) {
        // Through the same resolution the session builder points a provider at, so what these tests check is removed
        // is what would really have been there rather than a directory only this module believes in.
        let paths = crate::sessions::caches::resolve(app_dir, "opai-test", id).unwrap();
        let engine = paths.engine.join("engine.bin");
        let timing = paths.timing.join("timings.bin");
        std::fs::write(&engine, b"compiled from the weights installed then").unwrap();
        std::fs::write(&timing, b"measured across every model").unwrap();

        (engine, timing)
    }

    #[tokio::test]
    async fn replacing_a_models_published_files_discards_what_was_compiled_from_the_previous_ones() {
        // At best a stale engine is wasted disk; at worst it is reused against weights it was never built for, which
        // produces an image nothing reports as wrong.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let id = kyoto();

        let split = fixture_listing(&[format!("{KYOTO}.onnx"), format!("{KYOTO}.onnx.data")]);
        install_from(&server, &split, &app_dir, &id, ModelTrust::Published).await.unwrap();
        let (engine, _timing) = compiled(&app_dir, &id);

        // Republished as a single file, which is what makes this a replacement rather than a repeat.
        install_from(&server, &kyoto_listing(), &app_dir, &id, ModelTrust::Published).await.unwrap();

        assert!(!engine.exists(), "an engine compiled from the previous weights survived the replacement");
    }

    #[tokio::test]
    async fn an_install_that_is_already_current_leaves_what_was_compiled_in_place() {
        // The steady-state launch. These artifacts were compiled from exactly the weights that are on disk, and
        // discarding them would cost a rebuild of minutes for no reason.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let id = kyoto();
        let listing = kyoto_listing();

        install_from(&server, &listing, &app_dir, &id, ModelTrust::Published).await.unwrap();
        let (engine, timing) = compiled(&app_dir, &id);

        install_from(&server, &listing, &app_dir, &id, ModelTrust::Published).await.unwrap();

        assert!(engine.is_file(), "an install with nothing to do discarded a compiled engine");
        assert!(timing.is_file(), "an install with nothing to do discarded the shared timing cache");
    }

    #[tokio::test]
    async fn replacing_one_model_leaves_another_models_engine_and_the_shared_timing_cache_alone() {
        // The two reasons the engine directory is per-model and the timing cache is not: one model's compiled output
        // must never be attributed to another's weights, while measured kernel timings carry between graphs.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let id = kyoto();
        let other = Upscale::new(UpscaleVariant::Kyoto(FloatPrecision::Fp32), Scale::new(2.0).unwrap())
            .required_artifacts()
            .remove(0);

        let split = fixture_listing(&[format!("{KYOTO}.onnx"), format!("{KYOTO}.onnx.data")]);
        install_from(&server, &split, &app_dir, &id, ModelTrust::Published).await.unwrap();
        let (engine, timing) = compiled(&app_dir, &id);
        let (other_engine, _) = compiled(&app_dir, &other);

        install_from(&server, &kyoto_listing(), &app_dir, &id, ModelTrust::Published).await.unwrap();

        assert!(!engine.exists(), "the replaced model kept what was compiled from its previous weights");
        assert!(other_engine.is_file(), "replacing one model discarded another model's compiled output");
        assert!(timing.is_file(), "replacing one model discarded the timing cache every model shares");
    }

    #[tokio::test]
    async fn a_model_install_is_recorded_against_the_artifact_id() {
        // One `grep` over `opai.log` for an artifact returns its install as well as its session build and every run
        // that used it. The three records are the shared install pipeline's, and a model's descriptor carries the
        // artifact id as its name — so nothing here restates them.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let id = kyoto();

        let (log, ()) = logging::records_of("info", || async {
            install_from(&server, &kyoto_listing(), &app_dir, &id, ModelTrust::Published).await.unwrap();
        })
        .await;

        for message in ["installing a dependency", "the dependency is ready"] {
            let record = log
                .lines()
                .find(|line| line.contains(&format!("msg=\"{message}\"")))
                .unwrap_or_else(|| panic!("{message:?} was not recorded:\n{log}"));

            assert!(
                record.contains(&format!("dependency={id}")),
                "the record is not keyed on the artifact: {record}"
            );
        }
    }

    #[tokio::test]
    async fn a_model_no_source_names_is_refused_with_a_record_that_names_it() {
        // The divergence from the reference, in the log as well as in the behaviour: it warns and downloads the
        // unknown model unverified, and this refuses. So the record is an `error` on a path that fails rather than a
        // warning on one that continues.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let unpublished =
            Operation::Upscale(Upscale::new(UpscaleVariant::Kyoto(FloatPrecision::Fp16), Scale::new(4.0).unwrap()))
                .required_artifacts()
                .remove(0);

        let (log, outcome) = logging::records_of("info", || async {
            install_from(&server, &kyoto_listing(), &app_dir, &unpublished, ModelTrust::Published).await
        })
        .await;

        assert!(matches!(outcome, Err(InitError::UnpublishedModel { .. })), "{outcome:?}");

        let refused = log
            .lines()
            .find(|line| line.contains("no published source names this model"))
            .unwrap_or_else(|| panic!("the refusal was not recorded:\n{log}"));

        assert!(refused.contains("level=WARN"), "a refusal returned to the caller is not a warning: {refused}");
        assert!(refused.contains(&format!("artifact={unpublished}")), "the refusal does not name it: {refused}");

        // And nothing was transferred on its behalf, which is what the record is claiming.
        assert!(
            !log.contains(r#"msg="installing a dependency""#),
            "a refused model was installed anyway:\n{log}"
        );
    }

    // ---------------------------------------------------------------------------------------------------------
    // A process that declares it trusts the model files already on disk.
    // ---------------------------------------------------------------------------------------------------------

    /// Puts `contents` on disk where the 4x Kyoto graph installs to, as an operator hand-copying a re-export does.
    ///
    /// Creates the directory and nothing else — no record, which is exactly the state a hand-placed file is in, and
    /// the state the trusted path has to recognise.
    fn place(app_dir: &Path, contents: &[u8]) -> PathBuf {
        let dir = app_dir.join("models").join(KYOTO);
        std::fs::create_dir_all(&dir).unwrap();

        let graph = dir.join(format!("{KYOTO}.onnx"));
        std::fs::write(&graph, contents).unwrap();

        graph
    }

    fn bytes(path: &Path) -> Vec<u8> {
        std::fs::read(path).unwrap()
    }

    /// Installs the 4x Kyoto graph under `trust`, returning where it landed.
    async fn acquire(server: &TestServer, app_dir: &Path, trust: ModelTrust) -> Result<PathBuf, InitError> {
        install_from(server, &kyoto_listing(), app_dir, &kyoto(), trust).await
    }

    #[tokio::test]
    async fn a_model_whose_every_file_is_present_is_used_as_it_is_under_a_declaration_of_trust() {
        // The whole workflow: a model re-exported and not yet published cannot otherwise be run at all, because the
        // file under test does not match the published hash and is replaced by the published weights before anything
        // can measure it. These bytes are deliberately **not** the fixture's, so "whatever their contents" is what is
        // actually being checked rather than a coincidence.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let unpublished = b"not the published weights, and that is the point".to_vec();
        let graph = place(&app_dir, &unpublished);

        let dir = acquire(&server, &app_dir, ModelTrust::LocalFiles).await.unwrap();

        assert_eq!(dir, app_dir.join("models").join(KYOTO));
        assert_eq!(bytes(&graph), unpublished, "the file under test was replaced by the published weights");
        assert_eq!(server.requests(), 0, "a transfer was attempted for a model that was already complete on disk");
    }

    #[tokio::test]
    async fn a_model_with_one_file_missing_is_installed_and_verified_even_under_a_declaration_of_trust() {
        // The declaration says "what is here is what I want measured". It does not say "fetch me whatever is missing
        // and ask no questions" — so a model that is not complete is installed whole, under the ordinary rules, and
        // never ends up a mix of trusted and verified files.
        //
        // Two published files with only one of them on disk, which is the shape a split model has: a graph beside the
        // weights blob it is too large to carry.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let listing = fixture_listing(&[format!("{KYOTO}.onnx"), format!("{KYOTO}.onnx.data")]);
        place(&app_dir, b"only half of a model");

        let dir = install_from(&server, &listing, &app_dir, &kyoto(), ModelTrust::LocalFiles).await.unwrap();

        // Both halves transferred and verified, and the hand-placed half replaced by the published bytes.
        assert_eq!(server.requests(), 2, "an incomplete model was not installed from the published sources");
        assert_eq!(bytes(&dir.join(format!("{KYOTO}.onnx"))), FIXTURE);
        assert!(dir.join(format!("{KYOTO}.onnx.data")).is_file(), "the missing half was not installed");

        // And the record is a real one, naming the files — the ordinary install happened, so nothing about it is
        // trusted.
        let recorded = manifest::read(&dir).expect("an ordinary install leaves a record");
        assert_eq!(recorded.files.len(), 2, "an ordinary install recorded no files");
    }

    #[tokio::test]
    async fn a_model_with_nothing_on_disk_is_downloaded_and_verified_under_a_declaration_of_trust() {
        // The first launch of a debugging binary, which has declared trust and has nothing to trust yet. There is no
        // third behaviour here: it is exactly what any other process does.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();

        let dir = acquire(&server, &app_dir, ModelTrust::LocalFiles).await.unwrap();

        assert_eq!(server.requests(), 1);
        assert_eq!(bytes(&dir.join(format!("{KYOTO}.onnx"))), FIXTURE, "the published bytes were not installed");
        assert_eq!(manifest::read(&dir).expect("a record").files.len(), 1);
    }

    #[tokio::test]
    async fn a_downloaded_file_whose_hash_is_wrong_fails_under_a_declaration_of_trust_exactly_as_without_one() {
        // The deliberate divergence from the reference, which also ignores a mismatch on bytes it has just fetched.
        // Trust decides what is read off the **disk**; it never decides what is accepted off the network, because the
        // workflow it exists for is a file the operator placed by hand and bytes arriving over a network are the ones
        // an attacker can choose.
        let server = TestServer::start(vec![Reply::Corrupt]).await;
        let (_root, app_dir) = app();

        let error = acquire(&server, &app_dir, ModelTrust::LocalFiles).await.unwrap_err();

        assert!(
            matches!(error, InitError::HashMismatch { .. }),
            "a declaration of trust accepted bad bytes: {error:?}"
        );

        let dir = app_dir.join("models").join(KYOTO);
        assert!(!dir.join(format!("{KYOTO}.onnx")).exists(), "bytes that failed verification were left in place");
        assert!(manifest::read(&dir).is_none(), "a failed install left a record");
    }

    #[tokio::test]
    async fn trust_does_not_escape_the_process_that_declared_it() {
        // What stops one debugging session silently downgrading every later session on the machine. The record a
        // trusted install leaves names no files, and `Manifest::intact` reads an empty list as "not installed" — so a
        // process that did not declare trust finds the record not current, and installs from the published sources.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let unpublished = b"weights nobody published".to_vec();
        let graph = place(&app_dir, &unpublished);

        let dir = acquire(&server, &app_dir, ModelTrust::LocalFiles).await.unwrap();
        assert_eq!(bytes(&graph), unpublished);
        assert_eq!(server.requests(), 0);

        // The record asserts no installed file, which is the whole mechanism.
        let trusted = manifest::read(&dir).expect("a trusted install still leaves a record");
        assert!(trusted.files.is_empty(), "a trusted install claimed files were installed: {:?}", trusted.files);

        // And the next process, which declared nothing, reinstalls and verifies rather than accepting them.
        acquire(&server, &app_dir, ModelTrust::Published).await.unwrap();

        assert_eq!(server.requests(), 1, "an ordinary process accepted files that were never checked");
        assert_eq!(bytes(&graph), FIXTURE, "the unverified weights survived a process that did not trust them");
        assert_eq!(manifest::read(&dir).expect("a record").files.len(), 1);
    }

    #[tokio::test]
    async fn a_trusted_model_whose_files_have_not_changed_transfers_nothing_and_keeps_what_was_compiled_from_them() {
        // The iteration loop this exists for: re-exporting a model is something a developer does repeatedly, and
        // rebuilding an execution provider's compiled engine on every launch costs minutes each time. A launch that
        // changed nothing has to discard nothing, or the declaration is unusable for the work it was written for.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        place(&app_dir, b"a model under test");

        acquire(&server, &app_dir, ModelTrust::LocalFiles).await.unwrap();

        // What a provider compiled from those weights, seeded after the first acquisition the way a session build
        // would have left it.
        let engines = app_dir.join("engines").join(KYOTO);
        std::fs::create_dir_all(&engines).unwrap();
        let engine = engines.join("compiled.engine");
        std::fs::write(&engine, b"an engine built from those weights").unwrap();

        acquire(&server, &app_dir, ModelTrust::LocalFiles).await.unwrap();

        assert_eq!(server.requests(), 0, "a second acquisition of an unchanged trusted model transferred something");
        assert!(engine.is_file(), "an unchanged trusted model discarded what was compiled from it");
    }

    #[tokio::test]
    async fn replacing_a_trusted_model_file_moves_its_stamp_and_discards_what_was_compiled_from_it() {
        // The other half, and the one that matters most in this workflow: a compiled artifact reused against weights
        // it was never built for produces an image nothing reports as wrong, and re-exporting a model is what changes
        // weights most often.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        let graph = place(&app_dir, b"the first export");

        let dir = acquire(&server, &app_dir, ModelTrust::LocalFiles).await.unwrap();
        let first = manifest::read(&dir).expect("a record").fingerprint;

        let engines = app_dir.join("engines").join(KYOTO);
        std::fs::create_dir_all(&engines).unwrap();
        let engine = engines.join("compiled.engine");
        std::fs::write(&engine, b"an engine built from the first export").unwrap();

        // A re-export: different bytes, so a different size, and a modification time that has moved.
        std::fs::write(&graph, b"the second export, which is a different length entirely").unwrap();

        acquire(&server, &app_dir, ModelTrust::LocalFiles).await.unwrap();

        let second = manifest::read(&dir).expect("a record").fingerprint;
        assert_ne!(first, second, "replacing a trusted file did not move its stamp");
        assert!(!engine.exists(), "an engine compiled from the previous weights survived a re-export");
        // The replacement is used as it is, without being checked and without being fetched over.
        assert_eq!(bytes(&graph), b"the second export, which is a different length entirely");
        assert_eq!(server.requests(), 0);
    }

    #[tokio::test]
    async fn using_a_model_on_trust_is_recorded_at_a_level_a_users_ordinary_log_contains() {
        // An unverified model is the first thing to suspect in a report of wrong output, and finding out that it was
        // in force must not require reproducing the problem with the logging turned up. `warn`, naming the model, in
        // the file a user attaches without being asked.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        place(&app_dir, b"a model under test");

        let (log, ()) = logging::records_of("info", || async {
            acquire(&server, &app_dir, ModelTrust::LocalFiles).await.unwrap();
        })
        .await;

        let skipped = log
            .lines()
            .find(|line| line.contains("verification skipped"))
            .unwrap_or_else(|| panic!("the skipped verification was not recorded:\n{log}"));

        assert!(skipped.contains("level=WARN"), "the record is below the default level: {skipped}");
        assert!(
            skipped.contains(&format!("dependency={KYOTO}")),
            "the record does not name the model: {skipped}"
        );
    }

    #[tokio::test]
    async fn the_record_is_written_on_every_acquisition_rather_than_once() {
        // Per acquisition, not once per process, and specifically **including** the launch that found the stamp
        // unmoved and did no work at all — which is exactly the launch where it would otherwise go unsaid.
        let server = TestServer::start(vec![]).await;
        let (_root, app_dir) = app();
        place(&app_dir, b"a model under test");

        acquire(&server, &app_dir, ModelTrust::LocalFiles).await.unwrap();

        let (log, ()) = logging::records_of("info", || async {
            acquire(&server, &app_dir, ModelTrust::LocalFiles).await.unwrap();
        })
        .await;

        assert!(
            log.lines().any(|line| line.contains("verification skipped")),
            "a second acquisition of an unchanged trusted model said nothing:\n{log}"
        );
        // And it did no work, which is the thing it is saying nothing else about.
        assert!(
            !log.contains(r#"msg="installing a dependency""#),
            "an unchanged trusted model reinstalled:\n{log}"
        );
    }
}
