//! Installing each dependency into its own directory, and what it reports while it does.

use super::*;

#[tokio::test]
async fn a_four_dependency_install_gives_each_one_its_own_directory_and_manifest() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");

    let (opai, _) = Opai::install(
        "opai-test",
        app_dir.clone(),
        claimed(&app_dir),
        every_dependency(&server),
        ModelTrust::Published,
        None,
    )
    .await
    .unwrap();

    for dir in all_dirs() {
        let installed = app_dir.join(deps::manifest::from_slash(dir));
        assert!(installed.join(fixture_library()).is_file(), "{dir} has no files");
        assert!(installed.join(deps::manifest::MANIFEST_NAME).is_file(), "{dir} has no manifest");
    }
    assert!(opai.providers().cuda && opai.providers().tensorrt);
}

#[tokio::test]
async fn each_dependency_reports_its_own_progress_ending_on_exactly_one() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");
    let (on_progress, seen) = recording();

    Opai::install(
        "opai-test",
        app_dir.clone(),
        claimed(&app_dir),
        every_dependency(&server),
        ModelTrust::Published,
        on_progress,
    )
    .await
    .unwrap();

    let reports = seen.lock().unwrap().clone();
    let order = [Dependency::Runtime, Dependency::Cuda, Dependency::Cudnn, Dependency::TensorRt];

    for dependency in &order {
        let mine: Vec<&Progress> = reports.iter().filter(|r| &r.dependency == dependency).collect();

        assert!(!mine.is_empty(), "{} reported nothing", dependency.as_str());
        assert!(mine.iter().any(|r| r.phase == Phase::Downloading), "{}", dependency.as_str());
        assert!(mine.iter().any(|r| r.phase == Phase::Extracting), "{}", dependency.as_str());
        assert!(mine.windows(2).all(|w| w[0].fraction <= w[1].fraction), "{}", dependency.as_str());
        assert_eq!(mine.last().unwrap().fraction, 1.0, "{} did not finish on 1.0", dependency.as_str());
    }

    // Sequential, in dependency order, so which dependency is installing is never ambiguous.
    let first_seen: Vec<Dependency> = reports.iter().fold(Vec::new(), |mut seen, report| {
        if seen.last() != Some(&report.dependency) {
            seen.push(report.dependency.clone());
        }
        seen
    });
    assert_eq!(first_seen, order, "the dependencies interleaved or were installed out of order");
}

#[tokio::test]
async fn the_whole_plan_is_reported_once_and_before_the_first_byte_of_it_moves() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");

    // One sink for both callbacks, since the ordering *between* them is what's under test.
    #[derive(Debug, PartialEq)]
    enum Event {
        Plan(Vec<Dependency>),
        Progress(Dependency),
    }

    let seen: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));

    let sink = Arc::clone(&seen);
    let on_plan: OnPlan = Arc::new(move |rows: &[PlannedDependency]| {
        sink.lock().unwrap().push(Event::Plan(rows.iter().map(|row| row.dependency.clone()).collect()));
    });

    let sink = Arc::clone(&seen);
    let on_progress: OnProgress = Arc::new(move |report: &Progress| {
        sink.lock().unwrap().push(Event::Progress(report.dependency.clone()));
    });

    let plan = every_dependency(&server);
    Opai::report_plan(&plan, Some(&on_plan));
    Opai::install("opai-test", app_dir.clone(), claimed(&app_dir), plan, ModelTrust::Published, Some(on_progress))
        .await
        .unwrap();

    let events = seen.lock().unwrap();
    let order = vec![Dependency::Runtime, Dependency::Cuda, Dependency::Cudnn, Dependency::TensorRt];

    let plans: Vec<&Event> = events.iter().filter(|event| matches!(event, Event::Plan(_))).collect();
    assert_eq!(plans.len(), 1, "the plan was reported {} times", plans.len());

    assert_eq!(events.first(), Some(&Event::Plan(order.clone())), "a progress report preceded the plan");
    assert!(events.len() > 1, "the install reported no progress at all");
}

#[tokio::test]
async fn an_initialization_with_no_plan_recipient_installs_the_same_and_reports_nothing() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");
    let (on_plan, seen) = recording_plan();

    let plan = every_dependency(&server);
    Opai::report_plan(&plan, None);
    assert!(seen.lock().unwrap().is_empty(), "a plan was reported to a recipient that was never registered");
    drop(on_plan);

    let (opai, _) = Opai::install("opai-test", app_dir.clone(), claimed(&app_dir), plan, ModelTrust::Published, None)
        .await
        .unwrap();

    for dir in all_dirs() {
        let installed = app_dir.join(deps::manifest::from_slash(dir));
        assert!(installed.join(fixture_library()).is_file(), "{dir} has no files");
        assert!(installed.join(deps::manifest::MANIFEST_NAME).is_file(), "{dir} has no manifest");
    }
    assert!(opai.providers().cuda && opai.providers().tensorrt);
}

#[tokio::test]
async fn a_second_launch_reports_the_whole_plan_again_and_each_dependency_as_already_in_place() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");

    let (first_plan, first) = recording_plan();
    let plan = every_dependency(&server);
    Opai::report_plan(&plan, first_plan.as_ref());
    Opai::install("opai-test", app_dir.clone(), claimed(&app_dir), plan, ModelTrust::Published, None)
        .await
        .unwrap();

    let requests = server.requests();
    let (on_progress, seen) = recording();
    let (second_plan, second) = recording_plan();

    let plan = every_dependency(&server);
    Opai::report_plan(&plan, second_plan.as_ref());
    Opai::install("opai-test", app_dir.clone(), claimed(&app_dir), plan, ModelTrust::Published, on_progress)
        .await
        .unwrap();

    let reports = seen.lock().unwrap().clone();
    let order = [Dependency::Runtime, Dependency::Cuda, Dependency::Cudnn, Dependency::TensorRt];

    let named: Vec<Dependency> = reports.iter().map(|report| report.dependency.clone()).collect();
    assert_eq!(named, order.to_vec(), "a skipped dependency reported something other than once");

    for report in &reports {
        assert_eq!(report.phase, Phase::AlreadyInstalled, "{}", report.dependency.as_str());
        assert_eq!(report.fraction, 1.0, "{}", report.dependency.as_str());
        assert_eq!(report.bytes, 0, "{} claimed bytes it never transferred", report.dependency.as_str());
        assert!(report.total.is_some_and(|total| total > 0), "{}", report.dependency.as_str());
    }

    assert_eq!(server.requests(), requests, "a steady-state launch transferred something");

    // The plan lists what's needed, not what's missing: a steady-state launch reports the same list, with
    // the same sizes, as the launch that installed everything. Nothing here reads the disk.
    let (first, second) = (first.lock().unwrap().clone(), second.lock().unwrap().clone());
    assert_eq!(
        second, first,
        "a steady-state launch reported a different plan than the launch that installed it"
    );

    let rows = second.first().expect("the steady-state launch reported no plan");
    assert!(rows.iter().all(|row| row.size > 0), "a row carried no published size: {rows:?}");

    assert_eq!(rows.iter().map(|row| row.dependency.clone()).collect::<Vec<_>>(), named);
}

#[tokio::test]
async fn an_already_installed_dependency_reports_before_a_later_one_that_does_have_work() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");

    // Runtime-only state: what a machine looks like after a new adapter appears, or an interrupted launch.
    Opai::install(
        "opai-test",
        app_dir.clone(),
        claimed(&app_dir),
        runtime_only(&server),
        ModelTrust::Published,
        None,
    )
    .await
    .unwrap();

    let (on_progress, seen) = recording();
    Opai::install(
        "opai-test",
        app_dir.clone(),
        claimed(&app_dir),
        every_dependency(&server),
        ModelTrust::Published,
        on_progress,
    )
    .await
    .unwrap();

    let reports = seen.lock().unwrap().clone();

    // The runtime had nothing to do, and says so at the point it would otherwise install.
    let first = reports.first().expect("the install reported nothing");
    assert_eq!(first.dependency, Dependency::Runtime);
    assert_eq!(first.phase, Phase::AlreadyInstalled);

    let downloading = reports
        .iter()
        .position(|report| report.phase == Phase::Downloading)
        .expect("no GPU library was downloaded");
    assert_eq!(reports[downloading].dependency, Dependency::Cuda);
    assert!(downloading > 0, "a download preceded the runtime\'s already-in-place report");

    let runtime: Vec<&Progress> = reports.iter().filter(|r| r.dependency == Dependency::Runtime).collect();
    assert_eq!(runtime.len(), 1, "the runtime reported {} times: {runtime:?}", runtime.len());
}

#[tokio::test]
async fn a_bumped_pin_deletes_only_that_dependencys_files() {
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");

    Opai::install(
        "opai-test",
        app_dir.clone(),
        claimed(&app_dir),
        every_dependency(&server),
        ModelTrust::Published,
        None,
    )
    .await
    .unwrap();

    // A file only the previous CUDA version installed: the replacement must delete it, not leave two
    // versions' libraries for the loader to choose between.
    let cuda_dir = app_dir.join("libs").join("cuda");
    let stale = cuda_dir.join("libcudart.so.12");
    std::fs::write(&stale, b"the previous release").unwrap();
    let mut recorded = deps::manifest::read(&cuda_dir).unwrap();
    recorded.files.push(deps::manifest::File { path: "libcudart.so.12".to_string(), size: 20 });
    deps::manifest::write(&cuda_dir, &recorded).unwrap();

    // Only CUDA moves; the other three stay pinned as installed.
    let mut bumped = every_dependency(&server);
    bumped.gpu[0] = fixture(&server, &CUDA, "cuda/13.4.0");
    let (on_progress, seen) = recording();

    Opai::install("opai-test", app_dir.clone(), claimed(&app_dir), bumped, ModelTrust::Published, on_progress)
        .await
        .unwrap();

    assert!(!stale.exists(), "the previous CUDA version's file survived the bump");
    assert_eq!(deps::manifest::read(&cuda_dir).unwrap().version, "cuda/13.4.0");

    for (dir, version) in [
        ("runtime", "runtime/1.30.0"),
        ("libs/cudnn", "cudnn/9.23.1"),
        ("libs/tensorrt", "tensorrt/10.14.1"),
    ] {
        let installed = app_dir.join(deps::manifest::from_slash(dir));
        assert_eq!(deps::manifest::read(&installed).unwrap().version, version, "{dir} was reinstalled");
        assert!(installed.join(fixture_library()).is_file(), "{dir} lost its files");
    }

    let reports = seen.lock().unwrap().clone();
    assert!(
        reports.iter().any(|report| report.dependency == Dependency::Cuda),
        "the bumped dependency reported nothing"
    );
    assert!(
        reports
            .iter()
            .filter(|report| report.dependency != Dependency::Cuda)
            .all(|report| report.phase == Phase::AlreadyInstalled),
        "a dependency that was already installed reported work"
    );
}

#[tokio::test]
async fn a_failing_gpu_transfer_fails_initialization_with_an_error_naming_that_dependency() {
    // Once a GPU library is selected, a failure is fatal like the runtime's — falling back to CPU silently
    // would be a user-visible behaviour change belonging to the front ends, not this crate.
    let server = TestServer::start(vec![]).await;
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");

    // TensorRT pinned to a hash its bytes can't match.
    let mut plan = every_dependency(&server);
    plan.gpu[2].sources[0].sha256 = sha256_bytes(b"not the published archive");

    let error = Opai::install("opai-test", app_dir.clone(), claimed(&app_dir), plan, ModelTrust::Published, None)
        .await
        .unwrap_err();

    match error {
        InitError::HashMismatch { url, .. } => {
            assert!(url.contains("tensorrt"), "the error does not name the dependency that failed: {url}");
        }
        other => panic!("expected HashMismatch, got {other:?}"),
    }

    // Everything already installed is left in place for the next run.
    for dir in &all_dirs()[..3] {
        let installed = app_dir.join(deps::manifest::from_slash(dir));
        assert!(installed.join(deps::manifest::MANIFEST_NAME).is_file(), "{dir} was rolled back");
    }
}
