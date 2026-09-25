//! A refused initialization, and the per-user claim on the configuration directory.

use super::*;

/// What [`snapshot`] records for one file.
///
/// Two shapes because a locked file's bytes are unreadable through another handle on Windows; its length
/// is what remains observable, and since nothing ever writes into it, length is the whole of any change.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FileState {
    /// Every other file, compared byte for byte.
    Contents(Vec<u8>),
    /// The lock file, compared by size because its bytes are unreadable under a live holder.
    Length(u64),
}

/// Every file beneath `dir`, path and contents, in a stable order.
///
/// Compares contents, not just names: a refused initialization must not touch the directory at all,
/// including the holder record of the process actually running. [`FileState`] says why the lock file is an
/// exception.
fn snapshot(dir: &Path) -> Vec<(PathBuf, FileState)> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];

    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current).expect("the directory is readable") {
            let path = entry.expect("the entry is readable").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let state = if path.file_name() == Some(OsStr::new(LOCK_FILE_NAME)) {
                    FileState::Length(path.metadata().expect("the entry is there").len())
                } else {
                    FileState::Contents(std::fs::read(&path).expect("the file is readable"))
                };
                files.push((path.strip_prefix(dir).expect("beneath dir").to_path_buf(), state));
            }
        }
    }

    files.sort();
    files
}

#[tokio::test]
async fn a_refused_initialization_transfers_nothing_and_leaves_the_directory_untouched() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");
    std::fs::create_dir_all(&app_dir).unwrap();

    // Another process of this user, holding the claim.
    let mut held = child_holding(&app_dir);
    let before = snapshot(&app_dir);

    // `Opai::initialize`'s own order: resolve, claim, then plan and install. `Opai::install` taking the
    // claim by value is what makes an unreached install impossible here.
    let error = instance::claim(&app_dir, GUI).unwrap_err();
    assert!(matches!(error, InitError::AlreadyRunning { .. }), "got {error:?}");

    assert_eq!(snapshot(&app_dir), before, "a refused initialization changed the configuration directory");
    assert_eq!(server.requests(), 0, "a refused initialization spent a transfer");

    let _ = held.kill();
    let _ = held.wait();
}

#[tokio::test]
async fn a_refused_initialization_reports_no_plan_because_nothing_was_probed() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");
    std::fs::create_dir_all(&app_dir).unwrap();

    let mut held = child_holding(&app_dir);
    let (on_plan, seen) = recording_plan();

    // The claim is taken before the hardware is probed, so a refusal here has nothing to report.
    let refused = (|| -> Result<Plan, InitError> {
        let _claim = instance::claim(&app_dir, GUI)?;
        let plan = Opai::select_at(&server.base_url, gpu::adapters(), "linux", "x86_64")?;
        Opai::report_plan(&plan, on_plan.as_ref());
        Ok(plan)
    })();

    let error = refused.unwrap_err();
    assert!(matches!(error, InitError::AlreadyRunning { .. }), "got {error:?}");
    assert!(seen.lock().unwrap().is_empty(), "a refused initialization reported a plan it never made");
    assert_eq!(server.requests(), 0, "a refused initialization spent a transfer");

    let _ = held.kill();
    let _ = held.wait();
}

#[tokio::test]
async fn the_claim_is_given_up_only_when_the_last_handle_is() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");

    let (opai, _) = Opai::install(
        "opai-test",
        app_dir.clone(),
        claimed(&app_dir),
        runtime_only(&server),
        ModelTrust::Published,
        None,
    )
    .await
    .unwrap();
    let clone = opai.clone();

    // Dropping one clone (e.g. `app.manage()`'s) releases nothing.
    drop(opai);
    assert!(child_attempt(&app_dir).starts_with(REFUSED), "dropping one clone gave up the claim");

    drop(clone);
    assert!(child_attempt(&app_dir).starts_with(ACQUIRED), "the last handle did not give up the claim");
}

#[test]
fn what_reads_no_shared_state_is_not_subject_to_the_claim() {
    // `--version` and the catalogue must work while another process holds the claim, which is why the
    // claim lives inside `initialize` rather than at the top of `main`.
    //
    // True today because both are free functions never resolving the configuration directory — pinned
    // because that's a fact about what they read, not their signatures.
    let dir = tempfile::tempdir().unwrap();
    let mut held = child_holding(dir.path());

    assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    assert!(!catalogue().is_empty(), "the catalogue was not published while another process held the claim");

    assert!(child_attempt(dir.path()).starts_with(REFUSED), "the child was not holding the claim");

    let _ = held.kill();
    let _ = held.wait();
}

#[tokio::test]
async fn two_initializations_in_one_process_under_one_name_both_succeed() {
    // A front end restarting its application layer after an error, not a second instance — the first
    // claim must not refuse the second.
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");

    let (first, _) = Opai::install(
        "opai-test",
        app_dir.clone(),
        claimed(&app_dir),
        runtime_only(&server),
        ModelTrust::Published,
        None,
    )
    .await
    .unwrap();
    let (second, _) = Opai::install(
        "opai-test",
        app_dir.clone(),
        claimed(&app_dir),
        runtime_only(&server),
        ModelTrust::Published,
        None,
    )
    .await
    .unwrap();

    assert_eq!(first.config_dir(), second.config_dir());
    assert!(
        !Arc::ptr_eq(&first.inner, &second.inner),
        "the second initialization handed back the first handle"
    );

    // One shared claim, given up only once both handles are.
    drop(first);
    assert!(child_attempt(&app_dir).starts_with(REFUSED));
    drop(second);
    assert!(child_attempt(&app_dir).starts_with(ACQUIRED));
}

#[tokio::test]
async fn two_application_names_in_one_process_both_initialize() {
    // Two names, two directories, no shared cache/models/log — neither contends with the other.
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let (one, two) = (root.path().join("opai-test-one"), root.path().join("opai-test-two"));

    let (first, _) =
        Opai::install("opai-test-one", one.clone(), claimed(&one), runtime_only(&server), ModelTrust::Published, None)
            .await
            .unwrap();
    let (second, _) =
        Opai::install("opai-test-two", two.clone(), claimed(&two), runtime_only(&server), ModelTrust::Published, None)
            .await
            .unwrap();

    assert_eq!(first.config_dir(), one);
    assert_eq!(second.config_dir(), two);
}

#[tokio::test]
async fn initialize_rejects_a_name_that_cannot_be_a_directory_before_touching_anything() {
    for name in ["", ".", "..", "a/b"] {
        let error = Opai::initialize(name, None).await.unwrap_err();
        match error {
            InitError::InvalidName { name: rejected, .. } => assert_eq!(rejected, name),
            other => panic!("expected InvalidName for {name:?}, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn initialize_rejects_an_identity_that_could_not_be_recorded_before_touching_anything() {
    // Refused before the claim, so nothing is recorded or transferred — a declared identity and a
    // defaulted one are both validated as directory segments, which permits a space the record can't carry.
    let declared = InitOptions { app: Some("my app".to_string()), ..Default::default() };
    let error = Opai::initialize(APP_NAME, Some(declared)).await.unwrap_err();
    match error {
        InitError::InvalidApp { app, .. } => assert_eq!(app, "my app"),
        other => panic!("expected InvalidApp, got {other:?}"),
    }
}

#[test]
fn an_undeclared_identity_is_claimed_and_recorded_under_the_name_it_initialized_under() {
    // `initialize`'s own prologue — resolve identity against name, then claim — driven against a test
    // directory, since the real call resolves the platform's directory and loads a runtime.
    let dir = tempfile::tempdir().unwrap();
    let name = "io.vinicius.opai-undeclared-identity-test";

    let app = app::identity(None, name).expect("a name that is also a legible identity");
    let claim = instance::claim(dir.path(), &app).expect("an unheld directory");

    assert_eq!(
        instance::tests::holder_of(dir.path()),
        Some(Holder { app: name.to_string(), pid: std::process::id() }),
        "an undeclared identity was recorded as something other than the name"
    );

    drop(claim);
}

#[test]
fn a_declared_identity_is_claimed_and_recorded_instead_of_the_name() {
    let dir = tempfile::tempdir().unwrap();
    let name = "io.vinicius.opai-declared-identity-test";

    let app = app::identity(Some(GUI.to_string()), name).expect("a declared identity");
    let claim = instance::claim(dir.path(), &app).expect("an unheld directory");

    assert_eq!(
        instance::tests::holder_of(dir.path()),
        Some(Holder { app: GUI.to_string(), pid: std::process::id() }),
        "a declared identity lost to the name"
    );

    drop(claim);
}
