//! Installing each dependency into its own folder of the shared root, and what it reports while it does.

use super::*;

/// Installs `plan` for `opai-test` in a new application directory, reporting to `on_progress` and `on_plan`.
async fn install(
    plan: Plan,
    on_progress: Option<OnProgress>,
    on_plan: Option<OnPlan>,
) -> (tempfile::TempDir, Result<(Opai, setup::Installed), InitError>) {
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");
    let installed = Opai::install_reporting(
        "opai-test",
        app_dir.clone(),
        claimed(&app_dir),
        plan,
        ModelTrust::Published,
        on_progress,
        on_plan,
    )
    .await;
    (root, installed)
}

#[tokio::test]
async fn a_four_dependency_install_gives_each_one_its_own_folder_under_the_shared_root() {
    let server = HfServer::start();
    let (app, installed) = install(every_dependency(&server), None, None).await;
    let (opai, _) = installed.unwrap();

    for (which, folder) in EVERYTHING.iter().zip(["onnx/1.30.0", "cuda/13.3.0", "cudnn/9.23.1", "tensorrt/10.14.1"]) {
        let dir = installed_dir(&server, which);
        assert_eq!(dir, server.root().join(folder), "{}", which.as_str());
        assert_installed(&dir);
    }
    assert!(opai.providers().cuda && opai.providers().tensorrt);

    // Nothing of it is in the application's own directory, where models, caches and settings live.
    assert!(!app.path().join("opai-test/runtime").exists());
    assert!(!app.path().join("opai-test/libs").exists());
}

#[tokio::test]
async fn each_dependency_reports_its_own_progress_in_install_order_ending_on_exactly_one() {
    let server = HfServer::start();
    let (on_progress, seen) = recording();
    let (_app, installed) = install(every_dependency(&server), on_progress, None).await;
    installed.unwrap();

    let reports = seen.lock().unwrap().clone();
    for dependency in &EVERYTHING {
        let mine: Vec<&Progress> = reports.iter().filter(|r| &r.dependency == dependency).collect();

        assert!(!mine.is_empty(), "{} reported nothing", dependency.as_str());
        assert!(mine.iter().any(|r| r.phase == Phase::Downloading), "{}", dependency.as_str());
        assert!(mine.iter().any(|r| r.phase == Phase::Extracting), "{}", dependency.as_str());
        let downloads = mine.iter().take_while(|r| r.phase == Phase::Downloading).count();
        assert!(
            mine[downloads..].iter().all(|r| r.phase == Phase::Extracting),
            "{} reported a download after its extraction began",
            dependency.as_str()
        );
        assert!(
            mine[..downloads].iter().all(|r| r.total == Some(hf_server::FIXTURE.len() as u64)),
            "{}: a download report did not carry the archive's size",
            dependency.as_str()
        );
        assert!(mine.windows(2).all(|w| w[0].fraction <= w[1].fraction), "{}", dependency.as_str());
        assert_eq!(mine.last().unwrap().fraction, 1.0, "{} did not finish on 1.0", dependency.as_str());
    }

    // Sequential, in install order, so which dependency is installing is never ambiguous.
    let order: Vec<Dependency> = reports.iter().fold(Vec::new(), |mut seen, report| {
        if seen.last() != Some(&report.dependency) {
            seen.push(report.dependency.clone());
        }
        seen
    });
    assert_eq!(order, EVERYTHING, "the dependencies interleaved or were installed out of order");
}

#[tokio::test]
async fn the_whole_plan_is_reported_once_and_before_the_first_byte_of_it_moves() {
    // One sink for both callbacks, since the ordering *between* them is what's under test.
    #[derive(Debug, PartialEq)]
    enum Event {
        Plan(Vec<Dependency>),
        Progress(Dependency),
    }

    let server = HfServer::start();
    let seen: Arc<Mutex<Vec<Event>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let on_plan: OnPlan = Arc::new(move |rows: &[PlannedDependency]| {
        sink.lock().unwrap().push(Event::Plan(rows.iter().map(|row| row.dependency.clone()).collect()));
    });
    let sink = Arc::clone(&seen);
    let on_progress: OnProgress = Arc::new(move |report: &Progress| {
        sink.lock().unwrap().push(Event::Progress(report.dependency.clone()));
    });

    let (_app, installed) = install(every_dependency(&server), Some(on_progress), Some(on_plan)).await;
    installed.unwrap();

    let events = seen.lock().unwrap();
    let plans = events.iter().filter(|event| matches!(event, Event::Plan(_))).count();
    assert_eq!(plans, 1, "the plan was reported {plans} times");
    assert_eq!(events.first(), Some(&Event::Plan(EVERYTHING.to_vec())), "a progress report preceded the plan");
    assert!(events.len() > 1, "the install reported no progress at all");
}

#[tokio::test]
async fn an_initialization_with_no_plan_recipient_installs_the_same() {
    let server = HfServer::start();
    let (_app, installed) = install(every_dependency(&server), None, None).await;
    let (opai, _) = installed.unwrap();

    for which in &EVERYTHING {
        assert_installed(&installed_dir(&server, which));
    }
    assert!(opai.providers().cuda && opai.providers().tensorrt);
}

#[tokio::test]
async fn a_second_launch_reports_the_same_plan_and_each_dependency_as_already_installed_with_no_request() {
    let server = HfServer::start();
    let (first_plan, first) = recording_plan();
    let (_app, installed) = install(every_dependency(&server), None, first_plan).await;
    installed.unwrap();

    let requests = server.requests();
    let (on_progress, seen) = recording();
    let (second_plan, second) = recording_plan();
    let (_app, installed) = install(every_dependency(&server), on_progress, second_plan).await;
    installed.unwrap();

    let reports = seen.lock().unwrap().clone();
    let named: Vec<Dependency> = reports.iter().map(|report| report.dependency.clone()).collect();
    assert_eq!(named, EVERYTHING, "an installed dependency reported something other than once");
    for report in &reports {
        assert_eq!(report.phase, Phase::AlreadyInstalled, "{}", report.dependency.as_str());
        assert_eq!(report.fraction, 1.0, "{}", report.dependency.as_str());
        assert_eq!(report.bytes, 0, "{} claimed bytes it never transferred", report.dependency.as_str());
        assert_eq!(report.total, Some(hf_server::FIXTURE.len() as u64), "{}", report.dependency.as_str());
    }

    assert_eq!(server.requests(), requests, "a steady-state launch made a request");

    // The plan lists what's needed, not what's missing: a steady-state launch reports the same list, with the same
    // sizes, read from what the first launch recorded.
    let (first, second) = (first.lock().unwrap().clone(), second.lock().unwrap().clone());
    assert_eq!(
        second, first,
        "a steady-state launch reported a different plan than the launch that installed it"
    );
}

#[tokio::test]
async fn a_second_launch_with_no_network_completes_without_waiting() {
    let server = HfServer::start();
    let (_app, installed) = install(every_dependency(&server), None, None).await;
    installed.unwrap();

    // Every folder on a port nothing listens on: a request would be refused and retried with backoff for seconds.
    let started = std::time::Instant::now();
    let (on_progress, seen) = recording();
    let (_app, installed) = install(server.offline_plan(&EVERYTHING), on_progress, None).await;
    let (opai, _) = installed.unwrap();

    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "the launch waited: {:?}",
        started.elapsed()
    );
    assert!(seen.lock().unwrap().iter().all(|report| report.phase == Phase::AlreadyInstalled));
    assert!(opai.providers().cuda && opai.providers().tensorrt);
}

#[tokio::test]
async fn an_already_installed_dependency_reports_before_a_later_one_that_does_have_work() {
    let server = HfServer::start();
    // Runtime-only state: what a machine looks like after a new adapter appears, or after an interrupted launch.
    let (_app, installed) = install(runtime_only(&server), None, None).await;
    installed.unwrap();

    let (on_progress, seen) = recording();
    let (_app, installed) = install(every_dependency(&server), on_progress, None).await;
    installed.unwrap();

    let reports = seen.lock().unwrap().clone();
    let first = reports.first().expect("the install reported nothing");
    assert_eq!(first.dependency, Dependency::Runtime);
    assert_eq!(first.phase, Phase::AlreadyInstalled);

    let downloading = reports
        .iter()
        .position(|report| report.phase == Phase::Downloading)
        .expect("no GPU library was downloaded");
    assert_eq!(reports[downloading].dependency, Dependency::Cuda);

    let runtime = reports.iter().filter(|r| r.dependency == Dependency::Runtime).count();
    assert_eq!(runtime, 1, "the runtime reported {runtime} times");
}

#[tokio::test]
async fn a_bumped_version_replaces_that_dependencys_folder_and_leaves_the_others() {
    let server = HfServer::start();
    let (_app, installed) = install(every_dependency(&server), None, None).await;
    installed.unwrap();
    let old_cuda = installed_dir(&server, &Dependency::Cuda);

    // Only CUDA moves; the other three stay as installed.
    let mut bumped = every_dependency(&server);
    bumped.dependencies[1].1.source = server.source_at(&Dependency::Cuda, "13.4.0");
    let (on_progress, seen) = recording();
    let (_app, installed) = install(bumped, on_progress, None).await;
    installed.unwrap();

    assert!(!old_cuda.exists(), "the previous CUDA version's folder survived the bump");
    assert_installed(&server.root().join("cuda/13.4.0"));
    for which in [Dependency::Runtime, Dependency::Cudnn, Dependency::TensorRt] {
        assert_installed(&installed_dir(&server, &which));
    }

    let reports = seen.lock().unwrap().clone();
    assert!(
        reports
            .iter()
            .any(|report| report.dependency == Dependency::Cuda && report.phase == Phase::Downloading)
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
async fn a_damaged_gpu_download_fails_initialization_naming_that_dependency() {
    // Once a GPU library is selected, a failure is fatal like the runtime's — falling back to CPU silently would be a
    // user-visible behaviour change belonging to the front ends, not this crate.
    let server = HfServer::start();
    server.corrupt(&Dependency::TensorRt);

    let (_app, installed) = install(every_dependency(&server), None, None).await;

    let Err(error) = installed else { panic!("the install must fail") };
    match error {
        InitError::Transfer { dependency, reason } => {
            assert_eq!(dependency, "NVIDIA TensorRT");
            assert!(reason.contains("damaged"), "{reason}");
        }
        other => panic!("expected Transfer, got {other:?}"),
    }

    // Everything already installed is left in place for the next run.
    for which in [Dependency::Runtime, Dependency::Cuda, Dependency::Cudnn] {
        assert_installed(&installed_dir(&server, &which));
    }
    assert!(!installed_dir(&server, &Dependency::TensorRt).join(".installed").exists());
}

#[tokio::test]
async fn each_dependency_is_a_unit_of_its_own_naming_its_folder_and_version() {
    let server = HfServer::start();
    let (_, observed, outcome) = logging::traced_of("info", || async {
        let (_app, installed) = install(every_dependency(&server), None, None).await;
        installed.map(|_| ())
    })
    .await;
    outcome.unwrap();

    let installs = observed.named("install");
    let recorded: Vec<(Option<&str>, Option<&str>)> =
        installs.iter().map(|span| (span.field("dependency"), span.field("version"))).collect();
    assert_eq!(
        recorded,
        [
            (Some("ONNX Runtime"), Some("onnx/1.30.0")),
            (Some("NVIDIA CUDA"), Some("cuda/13.3.0")),
            (Some("NVIDIA cuDNN"), Some("cudnn/9.23.1")),
            (Some("NVIDIA TensorRT"), Some("tensorrt/10.14.1")),
        ]
    );
    assert!(
        installs.iter().all(|span| span.field("outcome") == Some("finished") && span.closed),
        "{installs:?}"
    );
}
