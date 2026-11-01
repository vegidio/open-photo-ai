use std::time::{Duration, SystemTime};

use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::deps::hf_server::{HfServer, MODEL};
use crate::logging;
use crate::models::test_support::{artifact, kyoto};
use crate::models::{FloatPrecision, UpscaleVariant};
use crate::progress::{self, Progress};

/// The artifact most tests here install, and the file it is published as.
const KYOTO: &str = "up_kyoto_4x_fp32";

fn graph() -> String {
    format!("{KYOTO}.onnx")
}

/// The other half of an 8x Kyoto, for the tests about one model leaving another alone.
fn kyoto_2x() -> ArtifactId {
    artifact(UpscaleVariant::Kyoto(FloatPrecision::Fp32), 2.0)
}

/// A server publishing the 4x Kyoto graph, a models folder of its own on it, and an application directory.
struct Env {
    server: HfServer,
    folder: ModelFolder,
    _root: TempDir,
    app_dir: PathBuf,
}

fn env() -> Env {
    let server = HfServer::start();
    server.put_model(&graph(), MODEL);
    let folder = ModelFolder::at(server.models());
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");
    Env { server, folder, _root: root, app_dir }
}

impl Env {
    async fn acquire(&self, id: &ArtifactId, trust: ModelTrust) -> Result<PathBuf, InitError> {
        self.acquire_from(&self.folder, id, trust).await
    }

    async fn acquire_from(
        &self,
        folder: &ModelFolder,
        id: &ArtifactId,
        trust: ModelTrust,
    ) -> Result<PathBuf, InitError> {
        install(folder, &self.app_dir, "opai-test", id, trust, &Installing::default()).await
    }

    fn model_dir(&self, id: &ArtifactId) -> PathBuf {
        self.app_dir.join(model_dir(id))
    }

    fn engine_dir(&self, id: &ArtifactId) -> PathBuf {
        self.app_dir.join(engine_dir(id))
    }

    /// Put a compiled file in `id`'s engine folder, as a provider's build does.
    fn compile(&self, id: &ArtifactId) -> PathBuf {
        let engine = self.engine_dir(id).join("coreml").join("model.mlmodelc");
        std::fs::create_dir_all(engine.parent().unwrap()).unwrap();
        std::fs::write(&engine, b"compiled").unwrap();
        engine
    }

    /// Put `bytes` where the 4x Kyoto graph installs, with no record, as an operator copying a file in by hand does.
    fn place(&self, bytes: &[u8]) -> PathBuf {
        let dir = self.model_dir(&kyoto());
        std::fs::create_dir_all(&dir).unwrap();
        let graph = dir.join(graph());
        std::fs::write(&graph, bytes).unwrap();
        graph
    }
}

#[tokio::test]
async fn a_model_installs_its_published_file_into_a_directory_of_its_own() {
    let env = env();
    let id = kyoto();
    let (on_progress, seen) = progress::recording();

    let dir = install(
        &env.folder,
        &env.app_dir,
        "opai-test",
        &id,
        ModelTrust::Published,
        &Installing { on_progress: Some(on_progress), ..Installing::default() },
    )
    .await
    .unwrap();

    assert_eq!(dir, env.app_dir.join("models").join(KYOTO));
    assert_eq!(std::fs::read(dir.join(graph())).unwrap(), MODEL);
    assert_eq!(env.server.downloads().len(), 1);

    let reports: Vec<Progress> = seen.lock().unwrap().clone();
    assert!(!reports.is_empty(), "the install reported nothing");
    assert!(reports.iter().all(|report| report.dependency == crate::Dependency::Model(id.clone())));
    assert!(reports.iter().all(|report| report.total == Some(MODEL.len() as u64)));
    let fractions: Vec<f64> = reports.iter().map(|report| report.fraction).collect();
    assert!(fractions.is_sorted(), "{fractions:?}");
    assert_eq!(fractions.last(), Some(&1.0));
}

#[tokio::test]
async fn a_model_published_as_a_graph_and_its_weights_installs_both_and_nothing_else() {
    let env = env();
    env.server.put_model(&format!("{KYOTO}.onnx.data"), b"the external weights");
    env.server.put_model("up_kyoto_4x_fp32_v2.onnx", b"a model whose name starts the same");

    let dir = env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();

    assert_eq!(std::fs::read(dir.join(format!("{KYOTO}.onnx.data"))).unwrap(), b"the external weights");
    assert!(dir.join(graph()).is_file());
    assert!(!dir.join("up_kyoto_4x_fp32_v2.onnx").exists());
    assert_eq!(env.server.downloads().len(), 2);
}

#[tokio::test]
async fn the_listing_is_read_once_for_several_installs() {
    let env = env();
    env.server.put_model(&format!("{}.onnx", kyoto_2x()), b"the 2x graph");

    env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();
    env.acquire(&kyoto_2x(), ModelTrust::Published).await.unwrap();
    env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();

    assert_eq!(env.server.model_listings(), 1, "every install after the first used what was already read");
}

#[tokio::test]
async fn nothing_of_the_listing_is_left_on_disk_after_a_run() {
    let env = env();
    env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();

    // Every file the run left behind is the model's own or the installer's (its marker, and its lock beside the
    // folder), so no copy of the listing is there for a later run to read.
    let mut left = Vec::new();
    let mut pending = vec![env.app_dir.clone()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                left.push(path.strip_prefix(&env.app_dir).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    left.sort();

    let dir = model_dir(&kyoto());
    let expected = [format!("{dir}/.installed"), format!("{dir}.lock"), format!("{dir}/{}", graph())];
    assert!(left.iter().all(|path| expected.contains(path)), "{left:?}");
    assert!(left.contains(&format!("{dir}/{}", graph())), "{left:?}");
}

#[tokio::test]
async fn an_artifact_the_listing_does_not_name_is_refused_with_nothing_transferred() {
    let env = env();
    let unpublished = artifact(UpscaleVariant::Kyoto(FloatPrecision::Fp16), 4.0);

    let (log, outcome) =
        logging::records_of("info", || async { env.acquire(&unpublished, ModelTrust::Published).await }).await;

    match outcome {
        Err(InitError::UnpublishedModel { artifact }) => assert_eq!(artifact, unpublished.as_str()),
        other => panic!("expected UnpublishedModel, got {other:?}"),
    }
    assert!(env.server.downloads().is_empty());

    let refused = log
        .lines()
        .find(|line| line.contains("no published source names this model"))
        .unwrap_or_else(|| panic!("the refusal was not recorded:\n{log}"));
    assert!(refused.contains("level=WARN"), "{refused}");
    assert!(refused.contains(&format!("artifact={unpublished}")), "{refused}");
    assert!(
        !log.contains(r#"msg="installing a dependency""#),
        "a refused model was installed anyway:\n{log}"
    );
}

#[tokio::test]
async fn a_served_file_whose_bytes_are_wrong_fails_as_a_mismatch() {
    let env = env();
    env.server.corrupt_model(&graph(), &vec![b'x'; MODEL.len()]);

    match env.acquire(&kyoto(), ModelTrust::Published).await {
        Err(InitError::HashMismatch { url, expected, actual }) => {
            assert!(url.ends_with(&format!("/resolve/main/models/{KYOTO}.onnx")), "{url}");
            assert_eq!(expected, rust_sak::crypto::sha256_bytes(MODEL));
            assert_eq!(actual, rust_sak::crypto::sha256_bytes(&vec![b'x'; MODEL.len()]));
        }
        other => panic!("expected HashMismatch, got {other:?}"),
    }
    assert!(!env.model_dir(&kyoto()).join(graph()).exists(), "the mismatched file was left in place");
}

#[tokio::test]
async fn a_second_install_of_an_intact_model_transfers_nothing_and_keeps_what_was_compiled() {
    let env = env();
    env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();
    let engine = env.compile(&kyoto());
    let requests = env.server.requests();

    env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();

    assert_eq!(env.server.requests(), requests, "an intact model made a request");
    assert!(engine.exists());
}

#[tokio::test]
async fn a_model_whose_files_were_removed_is_installed_again() {
    let env = env();
    let dir = env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();
    std::fs::remove_file(dir.join(graph())).unwrap();

    env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();

    assert_eq!(std::fs::read(dir.join(graph())).unwrap(), MODEL);
    assert_eq!(env.server.downloads().len(), 2);
}

#[tokio::test]
async fn a_republished_model_is_replaced_and_only_its_own_engine_is_discarded() {
    let env = env();
    env.server.put_model(&format!("{}.onnx", kyoto_2x()), b"the 2x graph");
    env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();
    env.acquire(&kyoto_2x(), ModelTrust::Published).await.unwrap();
    let own = env.compile(&kyoto());
    let other = env.compile(&kyoto_2x());
    let timing = env.app_dir.join("engines/.timing/cache.bin");
    std::fs::create_dir_all(timing.parent().unwrap()).unwrap();
    std::fs::write(&timing, b"timings").unwrap();

    // A new process, which reads the listing again and finds the 4x graph republished.
    env.server.put_model(&graph(), b"the 4x graph, republished");
    let folder = ModelFolder::at(env.server.models());
    let dir = env.acquire_from(&folder, &kyoto(), ModelTrust::Published).await.unwrap();
    env.acquire_from(&folder, &kyoto_2x(), ModelTrust::Published).await.unwrap();

    assert_eq!(std::fs::read(dir.join(graph())).unwrap(), b"the 4x graph, republished");
    assert!(!own.exists(), "what was compiled from the old weights survived");
    assert!(env.engine_dir(&kyoto()).is_dir(), "the engine folder itself is kept");
    assert!(other.exists(), "another model's engine was discarded");
    assert!(timing.exists(), "the shared timing cache was discarded");
}

#[tokio::test]
async fn a_model_from_an_earlier_release_is_adopted_with_nothing_transferred() {
    let env = env();
    let graph = env.place(MODEL);
    std::fs::write(graph.with_file_name(".manifest.json"), br#"{"schema":1}"#).unwrap();

    let dir = env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();

    assert!(env.server.downloads().is_empty(), "an adopted model was downloaded");
    assert!(!dir.join(".manifest.json").exists(), "the earlier release's record survived");
    let requests = env.server.requests();
    env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();
    assert_eq!(env.server.requests(), requests, "the next acquisition hashes and asks nothing");
}

#[tokio::test]
async fn a_file_from_an_earlier_release_that_no_longer_matches_is_downloaded_again() {
    let env = env();
    env.place(&vec![b'o'; MODEL.len()]);
    let engine = env.compile(&kyoto());

    let dir = env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();

    assert_eq!(std::fs::read(dir.join(graph())).unwrap(), MODEL);
    assert_eq!(env.server.downloads().len(), 1);
    assert!(!engine.exists(), "what was compiled from the previous weights survived");
}

#[tokio::test]
async fn an_installed_model_is_used_as_it_is_when_the_listing_cannot_be_read() {
    let env = env();
    let dir = env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();
    let engine = env.compile(&kyoto());

    let offline = ModelFolder::at(HfServer::offline_models());
    assert_eq!(env.acquire_from(&offline, &kyoto(), ModelTrust::Published).await.unwrap(), dir);
    assert!(engine.exists());
}

#[tokio::test]
async fn a_model_that_is_not_installed_fails_when_the_listing_cannot_be_read() {
    let env = env();
    let offline = ModelFolder::at(HfServer::offline_models());

    match env.acquire_from(&offline, &kyoto(), ModelTrust::Published).await {
        Err(error @ InitError::UnlistedModel { .. }) => {
            let message = error.to_string();
            assert!(message.contains(KYOTO) && message.contains("listing could not be read"), "{message}");
        }
        other => panic!("expected UnlistedModel, got {other:?}"),
    }

    // A model from an earlier release has nothing to verify it against either.
    env.place(MODEL);
    assert!(matches!(
        env.acquire_from(&offline, &kyoto(), ModelTrust::Published).await,
        Err(InitError::UnlistedModel { .. })
    ));
}

#[tokio::test]
async fn after_a_failed_read_an_installed_model_does_not_wait_on_the_listing_again() {
    let env = env();
    env.server.put_model(&format!("{}.onnx", kyoto_2x()), b"the 2x graph");
    // Installed by an earlier run, with its own listing.
    env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();
    env.acquire(&kyoto_2x(), ModelTrust::Published).await.unwrap();

    env.server.set_api_down(true);
    let folder = ModelFolder::at(env.server.models());
    env.acquire_from(&folder, &kyoto(), ModelTrust::Published).await.unwrap();
    let requests = env.server.requests();
    env.acquire_from(&folder, &kyoto_2x(), ModelTrust::Published).await.unwrap();

    assert_eq!(env.server.requests(), requests, "the second installed model asked for the listing again");
}

#[tokio::test]
async fn a_stopped_install_keeps_its_partial_and_the_next_one_finishes_it() {
    let env = env();
    env.server.stall_model(&graph(), 32 * 1024);
    let cancel = CancellationToken::new();
    let installing = Installing { cancel: cancel.clone(), ..Installing::default() };

    let id = kyoto();
    let part = env.model_dir(&id).join(".staging").join(format!("{}.part", graph()));
    let stopping = {
        let part = part.clone();
        async move {
            while std::fs::metadata(&part).map_or(0, |meta| meta.len()) == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            cancel.cancel();
        }
    };
    let (outcome, ()) = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(
            install(&env.folder, &env.app_dir, "opai-test", &id, ModelTrust::Published, &installing),
            stopping
        )
    })
    .await
    .expect("the stop ended the install");

    assert!(matches!(outcome, Err(InitError::Stopped)), "{outcome:?}");
    assert!(std::fs::metadata(&part).unwrap().len() > 0, "nothing was kept to resume onto");

    env.server.unstall_model(&graph());
    let dir = env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();
    assert_eq!(std::fs::read(dir.join(graph())).unwrap(), MODEL);
}

#[tokio::test]
async fn a_model_install_is_recorded_against_the_artifact_id() {
    // One `grep` over `opai.log` for an artifact returns its install as well as its session build and every run that
    // used it.
    let env = env();
    let id = kyoto();

    let (log, ()) = logging::records_of("info", || async {
        env.acquire(&id, ModelTrust::Published).await.unwrap();
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

// ---------------------------------------------------------------------------------------------------------------------
// The records of a run that uses installed models without the listing.
// ---------------------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn installed_models_used_without_the_listing_are_recorded_once_per_run() {
    let env = env();
    env.server.put_model(&format!("{}.onnx", kyoto_2x()), b"the 2x graph");
    env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();
    env.acquire(&kyoto_2x(), ModelTrust::Published).await.unwrap();

    let offline = ModelFolder::at(HfServer::offline_models());
    let (log, ()) = logging::records_of("info", || async {
        env.acquire_from(&offline, &kyoto(), ModelTrust::Published).await.unwrap();
        env.acquire_from(&offline, &kyoto_2x(), ModelTrust::Published).await.unwrap();
    })
    .await;

    let warned = logging::records(
        &log,
        "the published model listing could not be read; installed models are used as they are, without checking for \
         a newer version",
    );
    assert_eq!(warned.len(), 1, "once per run, not once per model:\n{log}");
    assert!(warned[0].contains("level=WARN"), "{}", warned[0]);
}

#[tokio::test]
async fn a_listing_read_successfully_is_recorded_with_its_entries_at_raised_verbosity() {
    let env = env();
    env.server.put_model(&format!("{}.onnx", kyoto_2x()), b"the 2x graph");

    let (log, ()) = logging::records_of("debug", || async {
        env.acquire(&kyoto(), ModelTrust::Published).await.unwrap();
    })
    .await;

    let read = logging::records(&log, "read the published model listing");
    assert_eq!(read.len(), 1, "{log}");
    assert!(read[0].contains("level=DEBUG") && read[0].contains("entries=2"), "{}", read[0]);
}

// ---------------------------------------------------------------------------------------------------------------------
// A process that declares it trusts the model files already on disk.
// ---------------------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn a_model_whose_every_file_is_present_is_used_as_it_is_under_a_declaration_of_trust() {
    let env = env();
    let graph = env.place(b"a re-export, not yet published");

    let (log, outcome) =
        logging::records_of("info", || async { env.acquire(&kyoto(), ModelTrust::LocalFiles).await }).await;

    outcome.unwrap();
    assert_eq!(std::fs::read(&graph).unwrap(), b"a re-export, not yet published");
    assert!(env.server.downloads().is_empty());
    let skipped = log
        .lines()
        .find(|line| line.contains("verification skipped"))
        .unwrap_or_else(|| panic!("the skipped verification was not recorded:\n{log}"));
    assert!(skipped.contains("level=WARN") && skipped.contains(&format!("dependency={KYOTO}")), "{skipped}");
}

#[tokio::test]
async fn a_trusted_model_is_recorded_on_every_acquisition_and_reinstalls_nothing() {
    let env = env();
    env.place(b"a re-export");
    env.acquire(&kyoto(), ModelTrust::LocalFiles).await.unwrap();
    let engine = env.compile(&kyoto());

    let (log, ()) = logging::records_of("info", || async {
        env.acquire(&kyoto(), ModelTrust::LocalFiles).await.unwrap();
    })
    .await;

    assert!(log.contains("verification skipped"), "a second acquisition said nothing:\n{log}");
    assert!(
        !log.contains(r#"msg="installing a dependency""#),
        "an unchanged trusted model reinstalled:\n{log}"
    );
    assert!(engine.exists(), "an unmoved stamp discarded what was compiled");
}

#[tokio::test]
async fn replacing_a_trusted_model_file_discards_what_was_compiled_from_it_once() {
    let env = env();
    let graph = env.place(b"the first export");
    env.acquire(&kyoto(), ModelTrust::LocalFiles).await.unwrap();
    let engine = env.compile(&kyoto());

    std::fs::write(&graph, b"the second export, which is a different length entirely").unwrap();
    let file = std::fs::File::options().write(true).open(&graph).unwrap();
    file.set_modified(SystemTime::now() + Duration::from_secs(60)).unwrap();
    drop(file);
    env.acquire(&kyoto(), ModelTrust::LocalFiles).await.unwrap();
    assert!(!engine.exists(), "a moved stamp kept what was compiled from the previous file");

    let engine = env.compile(&kyoto());
    env.acquire(&kyoto(), ModelTrust::LocalFiles).await.unwrap();
    assert!(engine.exists(), "once");
    assert_eq!(std::fs::read(&graph).unwrap(), b"the second export, which is a different length entirely");
    assert!(env.server.downloads().is_empty());
}

#[tokio::test]
async fn a_model_with_a_file_missing_is_installed_and_verified_under_a_declaration_of_trust() {
    let env = env();
    env.server.put_model(&format!("{KYOTO}.onnx.data"), b"the external weights");
    env.place(b"a re-exported graph");

    let dir = env.acquire(&kyoto(), ModelTrust::LocalFiles).await.unwrap();

    assert_eq!(
        std::fs::read(dir.join(graph())).unwrap(),
        MODEL,
        "the partial model was completed from the listing"
    );
    assert_eq!(std::fs::read(dir.join(format!("{KYOTO}.onnx.data"))).unwrap(), b"the external weights");
}

#[tokio::test]
async fn a_model_with_nothing_on_disk_is_downloaded_and_verified_under_a_declaration_of_trust() {
    let env = env();
    env.server.corrupt_model(&graph(), &vec![b'x'; MODEL.len()]);

    assert!(
        matches!(env.acquire(&kyoto(), ModelTrust::LocalFiles).await, Err(InitError::HashMismatch { .. })),
        "trust changed something about bytes the application obtained itself"
    );
}

#[tokio::test]
async fn trust_does_not_escape_the_process_that_declared_it() {
    let env = env();
    let graph = env.place(b"a re-export, not yet published");
    env.acquire(&kyoto(), ModelTrust::LocalFiles).await.unwrap();

    // A later process, without the declaration.
    let folder = ModelFolder::at(env.server.models());
    env.acquire_from(&folder, &kyoto(), ModelTrust::Published).await.unwrap();

    assert_eq!(std::fs::read(&graph).unwrap(), MODEL, "the unverified file was kept");
    assert_eq!(env.server.downloads().len(), 1);
}
