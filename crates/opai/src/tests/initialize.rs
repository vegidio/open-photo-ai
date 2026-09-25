//! Initializing the library and the handle it hands back.

use super::*;

#[tokio::test]
async fn initialize_installs_the_runtime_and_hands_back_a_handle() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");
    let (on_progress, seen) = recording();

    let (opai, _) = Opai::install(
        "opai-test",
        app_dir.clone(),
        claimed(&app_dir),
        runtime_only(&server),
        ModelTrust::Published,
        on_progress,
    )
    .await
    .unwrap();

    let runtime = app_dir.join("runtime");
    assert_eq!(opai.config_dir(), app_dir);
    test_server::assert_extracted(&runtime, "onnx-runtime_test.7z");
    assert!(runtime.join(deps::manifest::MANIFEST_NAME).is_file());

    let reports = seen.lock().unwrap().clone();
    assert!(reports.iter().all(|r| r.dependency == Dependency::Runtime));
    assert!(reports.iter().any(|r| r.phase == Phase::Downloading));
    assert!(reports.iter().any(|r| r.phase == Phase::Extracting));
    assert_eq!(reports.last().unwrap().fraction, 1.0);
}

#[tokio::test]
async fn an_initialization_says_what_it_did_on_the_path_that_runs_at_every_launch() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-records-test");

    // The real `instance::claim`, not the suite's `claimed` helper — the claim's own record is under test.
    std::fs::create_dir_all(&app_dir).unwrap();

    let (log, opai) = logging::records_of("debug", || async {
        let claim = instance::claim(&app_dir, CLI).unwrap();
        let (opai, _) = Opai::install(
            "opai-records-test",
            app_dir.clone(),
            claim,
            runtime_only(&server),
            ModelTrust::Published,
            None,
        )
        .await
        .unwrap();
        opai
    })
    .await;

    // Asserted against whatever mode was actually resolved, not `Disk`: this record exists to make a
    // degraded run visible, so demanding the healthy outcome would fail on the machine it's for.
    let (expected_mode, expected_level) = match opai.cache_mode() {
        CacheMode::Disk => ("mode=disk", "level=INFO"),
        CacheMode::Memory => ("mode=memory", "level=WARN"),
        CacheMode::None => ("mode=none", "level=WARN"),
    };

    // Through the formatter's own quoting, since a Windows path escapes differently than a POSIX one.
    let cache_path = logging::format::as_field_value(&app_dir.join(cache::CACHE_DIR).display().to_string());
    let cache_record = log
        .lines()
        .find(|line| line.contains(&format!("path={cache_path}")))
        .unwrap_or_else(|| panic!("the cache's decision was not recorded:\n{log}"));

    assert!(
        cache_record.contains(expected_mode),
        "the record disagrees with {:?}: {cache_record}",
        opai.cache_mode()
    );
    assert!(
        cache_record.contains(expected_level),
        "a {:?} cache was not reported at the level that makes it visible: {cache_record}",
        opai.cache_mode()
    );

    assert!(log.contains(r#"msg="single-instance claim taken""#), "the claim was not recorded:\n{log}");
    assert!(log.contains("level=DEBUG"), "{log}");

    // Every record carries the application, so two sessions can be told apart in one file.
    for line in log.lines().filter(|line| line.contains("msg=")) {
        assert!(line.contains(" app=cli "), "a record does not name its application: {line}");
    }
}

#[tokio::test]
async fn initialization_opens_with_a_record_naming_what_it_is_about() {
    // `Opai::initialize` resolves the platform's directory and loads a real runtime, neither hermetically testable —
    // so the opening record is checked by emitting it the same way, against the same formatter. The closing records
    // are the real ones, driven through `Opai::recorded` below.
    let (log, ()) = logging::records_of("debug", || async {
        tracing::info!(
            name = "opai-records-test",
            config_dir = "/tmp/opai-records-test",
            onnx = ONNX_RUNTIME.tag,
            os = std::env::consts::OS,
            arch = std::env::consts::ARCH,
            "initializing"
        );
    })
    .await;

    let opening = log.lines().find(|line| line.contains(r#"msg=initializing"#)).expect(&log);
    assert!(opening.contains("name=opai-records-test"), "{opening}");
    assert!(
        opening.contains(&format!("onnx={}", ONNX_RUNTIME.tag)),
        "the pinned runtime tag is missing: {opening}"
    );
    assert!(opening.contains(&format!("os={}", std::env::consts::OS)), "{opening}");
    assert!(opening.contains(&format!("arch={}", std::env::consts::ARCH)), "{opening}");

    // One `app=` per line, since it's the formatter's own field.
    for line in log.lines().filter(|line| line.contains("msg=")) {
        assert_eq!(line.matches(" app=").count(), 1, "the application is named more than once: {line}");
    }
}

#[tokio::test]
async fn a_finished_initialization_says_how_long_it_took_and_what_the_machine_supports() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-records-test");

    let (log, outcome) = logging::records_of("info", || async {
        Opai::recorded("opai-records-test", async {
            let (opai, _) = Opai::install(
                "opai-records-test",
                app_dir.clone(),
                claimed(&app_dir),
                runtime_only(&server),
                ModelTrust::Published,
                None,
            )
            .await?;
            Ok(opai)
        })
        .await
    })
    .await;
    outcome.unwrap();

    let closing = log.lines().find(|line| line.contains("msg=initialized")).expect(&log);
    assert!(closing.contains("level=INFO"), "{closing}");
    assert!(closing.contains("name=opai-records-test"), "{closing}");
    assert!(
        closing.contains("providers="),
        "what the machine can run is not on the closing record: {closing}"
    );
    assert!(closing.contains("duration="), "the closing record does not say how long it took: {closing}");
}

#[tokio::test]
async fn a_failed_initialization_is_recorded_once_with_its_duration_and_reason() {
    // The real `Opai::initialize`: the name is refused before any directory is resolved, so this touches nothing.
    let (log, outcome) = logging::records_of("info", || Opai::initialize("not/a/name", None)).await;

    assert!(matches!(outcome, Err(InitError::InvalidName { .. })), "{:?}", outcome.err());

    let warnings: Vec<&str> = log.lines().filter(|line| line.contains("level=WARN")).collect();
    assert_eq!(warnings.len(), 1, "a failure was not recorded exactly once:\n{log}");

    let failed = warnings[0];
    assert!(failed.contains(r#"msg="initialization failed""#), "{failed}");
    assert!(failed.contains(r#"name="not/a/name""#) || failed.contains("name=not/a/name"), "{failed}");
    assert!(failed.contains("duration="), "the failure does not say how long it ran: {failed}");
    assert!(
        failed.contains("is not usable as a directory"),
        "the failure does not give its reason: {failed}"
    );
    assert!(!log.contains("msg=initialized"), "a failed initialization was recorded as finished:\n{log}");
}

#[tokio::test]
async fn a_refusal_because_another_process_holds_the_claim_is_not_a_failure() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");
    std::fs::create_dir_all(&app_dir).unwrap();

    let mut held = child_holding(&app_dir);

    // `Opai::initialize`'s own order: claim, then install — so the refusal reaches the recorder exactly as it would.
    let (log, outcome) = logging::records_of("info", || async {
        Opai::recorded("opai-test", async {
            let claim = instance::claim(&app_dir, GUI)?;
            let (opai, _) =
                Opai::install("opai-test", app_dir.clone(), claim, runtime_only(&server), ModelTrust::Published, None)
                    .await?;
            Ok(opai)
        })
        .await
    })
    .await;

    let _ = held.kill();
    let _ = held.wait();

    assert!(matches!(outcome, Err(InitError::AlreadyRunning { .. })), "{:?}", outcome.err());

    let refused = log
        .lines()
        .find(|line| line.contains("another process holds the claim"))
        .unwrap_or_else(|| panic!("the refusal was not recorded:\n{log}"));
    assert!(refused.contains("level=INFO"), "{refused}");
    assert!(refused.contains("holder="), "the refusal does not name the holder it read: {refused}");
    assert!(
        !log.contains("level=WARN") && !log.contains("level=ERROR"),
        "a refusal was recorded as a fault:\n{log}"
    );
}

#[tokio::test]
async fn a_stopped_initialization_says_why_and_is_not_a_failure() {
    let (log, outcome) =
        logging::records_of("info", || Opai::recorded("opai-test", async { Err(InitError::Cancelled) })).await;

    assert!(matches!(outcome, Err(InitError::Cancelled)), "{:?}", outcome.err());

    let stopped = log.lines().find(|line| line.contains(r#"msg="initialization stopped""#)).expect(&log);
    assert!(stopped.contains("level=INFO"), "{stopped}");
    assert!(stopped.contains("reason=shutdown"), "{stopped}");
    assert!(stopped.contains("duration="), "{stopped}");
    assert!(!log.contains("level=WARN"), "a shutdown was recorded as a failure:\n{log}");
}

#[tokio::test]
async fn initialization_reads_the_model_listing_on_no_path_of_its_own() {
    // The listing is read no earlier than the first install that needs it. Asserted on what's left behind:
    // a resolution creates the models directory and caches a copy, so neither existing means nothing read it.
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

    let models = app_dir.join("models");
    assert!(!models.exists(), "initialization created the models directory, so something resolved a listing");
    assert!(
        !models.join(deps::model::manifest::CACHE_NAME).exists(),
        "initialization cached a model listing"
    );

    assert!(opai.providers().cpu);
}

#[tokio::test]
async fn initialization_opens_the_run_cache_on_disk_and_reports_it() {
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

    assert_eq!(opai.cache_mode(), CacheMode::Disk);
    assert!(app_dir.join(cache::CACHE_DIR).is_dir(), "the cache has no directory of its own");
}

#[tokio::test]
async fn an_initialization_whose_cache_cannot_be_opened_still_succeeds_and_says_it_is_degraded() {
    // Losing the cache costs speed and nothing else, so a launch must not fail over it.
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");

    // A file where the directory would go, so the store can't open there.
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::write(app_dir.join(cache::CACHE_DIR), b"not a directory").unwrap();

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

    assert_eq!(opai.cache_mode(), CacheMode::Memory, "a locked cache directory was not reported as degraded");
    assert!(opai.providers().cpu);
    assert!(app_dir.join("runtime").join(deps::manifest::MANIFEST_NAME).is_file());
}

#[tokio::test]
async fn what_backs_the_cache_is_reported_and_is_the_same_for_every_clone_of_the_handle() {
    // No setter: the mode belongs to the shared state, not the handle, so two components sharing one
    // application can't disagree about it.
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

    assert_eq!(opai.clone().cache_mode(), opai.cache_mode());
    // Named through a function pointer, pinning that it takes nothing.
    let reporter: fn(&Opai) -> CacheMode = Opai::cache_mode;
    assert_eq!(reporter(&opai), CacheMode::Disk);
}

#[tokio::test]
async fn the_handle_is_shared_rather_than_copied() {
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

    assert_eq!(opai.config_dir(), clone.config_dir());
    assert_eq!(opai.providers(), clone.providers());
    assert!(Arc::ptr_eq(&opai.inner, &clone.inner), "cloning re-initialized the state");
}

#[tokio::test]
async fn a_sub_directory_is_resolved_under_the_application_directory() {
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

    assert_eq!(opai.sub_dir("models").unwrap(), app_dir.join("models"));
    assert!(app_dir.join("models").is_dir());
    assert!(opai.sub_dir("../escape").is_err());
}

#[tokio::test]
async fn the_handle_reports_what_the_plan_it_installed_unlocks() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");
    let plan = runtime_only(&server);

    // Asserting the report is derived from the plan actually installed, not that two hand-maintained
    // things agree.
    let (opai, _) =
        Opai::install("opai-test", app_dir.clone(), claimed(&app_dir), plan.clone(), ModelTrust::Published, None)
            .await
            .unwrap();

    let report = opai.providers();
    assert_eq!(report, plan.providers(), "the handle reports something other than what its plan derives");
    assert!(report.cpu, "the CPU is always available");
    assert_eq!(report.coreml, cfg!(target_os = "macos"));
    assert!(!report.cuda && !report.tensorrt);
}

#[tokio::test]
async fn an_unpublished_gpu_platform_reports_the_provider_unsupported_and_still_initializes() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");
    let adapters = [adapter("NVIDIA GeForce RTX 4090", "NVIDIA")];
    let plan = Opai::select_at("http://example.invalid", &adapters, "windows", "aarch64").unwrap();

    assert!(plan.gpu.is_empty(), "something was selected for a platform nothing is published for");

    let (on_progress, seen) = recording();
    let (opai, _) = Opai::install(
        "opai-test",
        app_dir.clone(),
        claimed(&app_dir),
        runtime_only(&server),
        ModelTrust::Published,
        on_progress,
    )
    .await
    .unwrap();

    assert!(!opai.providers().cuda);
    assert!(!opai.providers().tensorrt);
    assert!(opai.providers().cpu);
    assert!(
        seen.lock().unwrap().iter().all(|report| report.dependency == Dependency::Runtime),
        "an unoffered provider was downloaded anyway"
    );
    for dir in &all_dirs()[1..] {
        assert!(!app_dir.join(dir).exists(), "{dir} was created for a provider that is not offered");
    }
}

#[test]
fn init_options_set_one_field_without_naming_the_others() {
    let options = InitOptions { app: Some(CLI.to_string()), ..Default::default() };

    assert_eq!(options.app.as_deref(), Some("cli"));
    assert!(options.on_progress.is_none());

    assert_eq!(options.models, ModelTrust::Published);
    let debugging = InitOptions { models: ModelTrust::LocalFiles, ..Default::default() };
    assert_eq!(debugging.models, ModelTrust::LocalFiles);
    assert!(debugging.app.is_none(), "setting the declaration moved the identity");

    let defaults = InitOptions::default();
    assert!(defaults.app.is_none() && defaults.on_progress.is_none() && defaults.on_plan.is_none());
    assert_eq!(defaults.models, ModelTrust::Published, "the default stopped being to verify every model");

    // Written by hand so a registered handler prints as a `bool` instead of failing.
    assert_eq!(
        format!("{defaults:?}"),
        r#"InitOptions { app: None, models: Published, on_progress: false, on_plan: false }"#
    );
}

#[tokio::test]
async fn releasing_the_sessions_says_what_went() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-release-test");

    let (opai, _) = Opai::install(
        "opai-release-test",
        app_dir.clone(),
        claimed(&app_dir),
        runtime_only(&server),
        ModelTrust::Published,
        None,
    )
    .await
    .unwrap();

    let (log, ()) = logging::records_of("info", || async { opai.release_sessions() }).await;

    let released = log
        .lines()
        .find(|line| line.contains(r#"msg="released resident sessions""#))
        .unwrap_or_else(|| panic!("the release was not recorded:\n{log}"));

    assert!(released.contains("released=0"), "the record does not say how many went: {released}");
    assert!(released.contains("reason=requested"), "the record does not say why: {released}");
    assert!(released.contains(r#"artifacts="""#), "the record does not say what went: {released}");
}

#[tokio::test]
async fn a_finished_initialization_is_one_span_holding_its_installs() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-spans-test");

    let (_, observed, outcome) = logging::traced_of("info", || async {
        Opai::recorded("opai-spans-test", async {
            let (opai, _) = Opai::install(
                "opai-spans-test",
                app_dir.clone(),
                claimed(&app_dir),
                runtime_only(&server),
                ModelTrust::Published,
                None,
            )
            .await?;
            Ok(opai)
        })
        .await
    })
    .await;
    outcome.unwrap();

    let initialize = observed.only("initialize");
    assert_eq!(initialize.field("outcome"), Some("finished"));
    assert_eq!(initialize.field("name"), Some("opai-spans-test"));
    assert_eq!(initialize.field("error"), None);
    assert!(initialize.closed);

    let install = observed.only("install");
    assert_eq!(install.parent, Some(initialize.index), "the install is not the initialization's child");
    assert_eq!(install.field("dependency"), Some("onnx-runtime"));
    assert_eq!(install.field("outcome"), Some("finished"));

    // The closing record is in the initialization's trace.
    let closing = observed.event("initialized").expect("the closing record did not reach the collector");
    assert_eq!(closing.span, Some(initialize.index));
}

#[tokio::test]
async fn a_failed_initialization_marks_its_span_failed() {
    let (_, observed, outcome) = logging::traced_of("info", || Opai::initialize("not/a/name", None)).await;
    assert!(matches!(outcome, Err(InitError::InvalidName { .. })), "{:?}", outcome.err());

    let initialize = observed.only("initialize");
    assert_eq!(initialize.field("outcome"), Some("failed"));
    assert!(
        initialize.field("error").is_some_and(|error| error.contains("is not usable as a directory")),
        "{initialize:?}"
    );
}

#[tokio::test]
async fn a_refused_initialization_says_it_did_not_happen_under_its_own_kind() {
    use crate::telemetry::metrics::FAILURES;

    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");
    std::fs::create_dir_all(&app_dir).unwrap();

    let mut held = child_holding(&app_dir);

    let refusals = || FAILURES.value(&[("unit", "initialize"), ("kind", "already_running")]);
    let before = refusals();
    let (log, observed, outcome) = logging::traced_of("info", || async {
        Opai::recorded("opai-test", async {
            let claim = instance::claim(&app_dir, GUI)?;
            let (opai, _) =
                Opai::install("opai-test", app_dir.clone(), claim, runtime_only(&server), ModelTrust::Published, None)
                    .await?;
            Ok(opai)
        })
        .await
    })
    .await;

    let _ = held.kill();
    let _ = held.wait();

    assert!(matches!(outcome, Err(InitError::AlreadyRunning { .. })), "{:?}", outcome.err());
    assert!(!log.contains("level=WARN"), "{log}");

    let initialize = observed.only("initialize");
    assert_eq!(initialize.field("outcome"), Some("failed"));
    assert!(initialize.field("error").is_some(), "{initialize:?}");
    // `>=`: another test in this process may be refused by a held claim too.
    assert!(refusals() > before, "the refusal was not counted under its kind");
    assert!(observed.named("install").is_empty(), "a refused initialization installed something");
}
