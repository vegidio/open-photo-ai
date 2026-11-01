//! Which dependencies a machine is given, and what the plan reports before anything moves.

use super::*;

/// What a current driver reports for an RTX 4090, Ada Lovelace.
fn rtx_4090() -> CudaInfo {
    cuda_device("NVIDIA GeForce RTX 4090", (8, 9))
}

/// A Turing card sold without the RTX brand, which the old name rule kept off TensorRT.
fn gtx_1660() -> GpuInfo {
    adapter("NVIDIA GeForce GTX 1660 SUPER", "NVIDIA")
}

/// What a current driver reports for [`gtx_1660`].
fn gtx_1660_cuda() -> CudaInfo {
    cuda_device("NVIDIA GeForce GTX 1660 SUPER", (7, 5))
}

#[test]
fn a_device_that_qualifies_for_cuda_and_tensorrt_installs_everything() {
    // Cards the Go rule's `rtx 20|30|40|50` substrings missed sit beside the ones it caught: the rule is the driver
    // and the device, never the name.
    for (name, compute_capability) in [
        ("NVIDIA GeForce RTX 4090", (8, 9)),
        ("GA102GL [RTX A6000]", (8, 6)),
        ("NVIDIA GeForce GTX 1660 SUPER", (7, 5)),
        ("Tesla T4", (7, 5)),
        ("NVIDIA H100 80GB HBM3", (9, 0)),
    ] {
        let cuda = cuda_device(name, compute_capability);
        let (chosen, providers) = selected(&[adapter(name, "NVIDIA")], Some(&cuda));

        assert_eq!(chosen, EVERYTHING, "{name}");
        assert!(providers.cuda && providers.tensorrt, "{name}");
    }
}

#[test]
fn a_device_too_old_for_cuda_installs_only_the_runtime_and_says_which_and_why() {
    // The machines seen in production failing `cublasCreate` with `CUBLAS_STATUS_ARCH_MISMATCH` on every model.
    for (name, compute_capability) in [("NVIDIA GeForce GTX 1060", (6, 1)), ("NVIDIA GeForce 930MX", (5, 0))] {
        let cuda = cuda_device(name, compute_capability);
        let (log, (chosen, providers)) =
            logging::records_of_blocking("info", || selected(&[adapter(name, "NVIDIA")], Some(&cuda)));

        assert_eq!(chosen, [Dependency::Runtime], "{name}");
        assert!(!providers.cuda && !providers.tensorrt, "{name}");

        let refused = log
            .lines()
            .find(|line| line.contains("the pinned CUDA release cannot run here"))
            .unwrap_or_else(|| panic!("the refusal was not recorded for {name}:\n{log}"));
        assert!(refused.contains("level=INFO"), "{refused}");
        assert!(refused.contains(name), "the record does not name the device: {refused}");
        let (major, minor) = compute_capability;
        assert!(
            refused.contains(&format!("compute capability {major}.{minor}, and 7.5 is required")),
            "the record does not say why: {refused}"
        );
        assert!(!log.contains("the pinned TensorRT release cannot run here"), "{log}");
    }
}

#[test]
fn a_driver_too_old_for_the_published_cuda_installs_only_the_runtime_and_names_both_versions() {
    let rtx = [adapter("NVIDIA GeForce RTX 4090", "NVIDIA")];
    let old_driver = CudaInfo { driver_version: (12, 8), ..rtx_4090() };

    let (log, (chosen, providers)) = logging::records_of_blocking("info", || selected(&rtx, Some(&old_driver)));

    assert_eq!(chosen, [Dependency::Runtime]);
    assert!(!providers.cuda && !providers.tensorrt);
    assert!(log.contains("the driver supports CUDA 12.8, and 13.0 is required"), "{log}");
}

#[test]
fn no_driver_or_no_device_installs_only_the_runtime_and_says_so() {
    let rtx = [adapter("NVIDIA GeForce RTX 4090", "NVIDIA")];
    let no_device = CudaInfo { driver_version: (13, 0), devices: Vec::new() };

    for (cuda, reason) in
        [(None, "no CUDA driver answered"), (Some(&no_device), "the CUDA 13.0 driver reports no device")]
    {
        let (log, (chosen, providers)) = logging::records_of_blocking("info", || selected(&rtx, cuda));

        assert_eq!(chosen, [Dependency::Runtime], "{cuda:?}");
        assert!(!providers.cuda && !providers.tensorrt, "{cuda:?}");
        assert!(log.contains(reason), "{log}");
    }
}

#[test]
fn a_machine_with_no_nvidia_adapter_installs_only_the_runtime_and_records_nothing() {
    for adapters in [vec![adapter("AMD Radeon RX 7900 XTX", "AMD")], vec![adapter("Apple M2 Max", "Apple")], vec![]] {
        let (log, (chosen, providers)) = logging::records_of_blocking("info", || selected(&adapters, None));

        assert_eq!(chosen, [Dependency::Runtime], "{adapters:?}");
        assert!(!providers.cuda && !providers.tensorrt, "{adapters:?}");
        assert!(providers.cpu, "the CPU is always available");
        assert!(!log.contains("cannot run here"), "the ordinary machine is not a refusal:\n{log}");
    }
}

#[test]
fn a_machine_that_runs_both_or_neither_records_no_tensorrt_refusal() {
    // The TensorRT refusal marks CUDA-without-TensorRT only. With both published releases needing Turing, no machine
    // lands there today, so this pins that nothing else writes it.
    for (adapters, cuda) in [
        (vec![adapter("NVIDIA GeForce RTX 4090", "NVIDIA")], Some(rtx_4090())),
        (vec![gtx_1660()], Some(gtx_1660_cuda())),
        (
            vec![adapter("NVIDIA GeForce GTX 1060", "NVIDIA")],
            Some(cuda_device("NVIDIA GeForce GTX 1060", (6, 1))),
        ),
        (vec![], None),
    ] {
        let (quiet, _) = logging::records_of_blocking("info", || selected(&adapters, cuda.as_ref()));
        assert!(
            !quiet.contains("the pinned TensorRT release cannot run here"),
            "{adapters:?} recorded:\n{quiet}"
        );
    }
}

#[test]
fn a_probe_that_fails_is_a_warning_and_a_machine_with_no_adapters() {
    // The probe is a process-wide `OnceLock`, so a failure can't be staged through `gpu::adapters` in a test sharing
    // the process — its answer goes to [`gpu::probed`] instead.
    let error = rust_sak::sysinfo::SysinfoError::UnsupportedPlatform { os: "haiku" };
    let (log, memoised) = logging::records_of_blocking("info", || gpu::probed(Err(error)));

    assert!(memoised.is_empty(), "a failed probe memoised adapters it never saw");
    let failed = log
        .lines()
        .find(|line| line.contains("the graphics adapters could not be enumerated"))
        .unwrap_or_else(|| panic!("{log}"));
    assert!(failed.contains("level=WARN"), "{failed}");

    // What a failure memoises (an empty list) still selects a runnable, CPU-only machine rather than failing.
    let (chosen, providers) = selected(&memoised, None);
    assert_eq!(chosen, [Dependency::Runtime]);
    assert!(providers.cpu && !providers.cuda && !providers.tensorrt);
}

#[test]
fn the_running_machine_selects_the_runtime_first() {
    let plan = Plan::for_machine(PathBuf::from("/root"), gpu::adapters(), gpu::cuda());

    assert_eq!(plan.dependencies.first().map(|(which, _)| which), Some(&Dependency::Runtime));
    assert_eq!(plan.dependencies[0].1.kind, rust_sak::ai::Kind::Runtime);
}

/// The one plan an install of `plan` reported, and what the install returned.
async fn planned(plan: Plan) -> (Vec<PlannedDependency>, Result<setup::Installed, InitError>) {
    let root = tempfile::tempdir().unwrap();
    let app_dir = root.path().join("opai-test");
    let (on_plan, seen) = recording_plan();

    let installed = Opai::install_reporting(
        "opai-test",
        app_dir.clone(),
        claimed(&app_dir),
        plan,
        ModelTrust::Published,
        None,
        on_plan,
    )
    .await
    .map(|(_, installed)| installed);

    let mut reports = seen.lock().unwrap().clone();
    assert!(reports.len() <= 1, "the plan was reported {} times", reports.len());
    (reports.pop().unwrap_or_default(), installed)
}

#[tokio::test]
async fn a_machine_with_no_nvidia_adapter_is_told_it_needs_the_runtime_alone() {
    let server = HfServer::start();
    let (rows, _) = planned(runtime_only(&server)).await;

    assert_eq!(
        rows,
        [PlannedDependency { dependency: Dependency::Runtime, size: hf_server::FIXTURE.len() as u64 }]
    );
}

#[tokio::test]
async fn an_nvidia_machine_is_told_it_needs_all_four_in_install_order_with_their_published_sizes() {
    let server = HfServer::start();
    let (rows, installed) = planned(every_dependency(&server)).await;

    let named: Vec<Dependency> = rows.iter().map(|row| row.dependency.clone()).collect();
    assert_eq!(named, EVERYTHING);
    assert!(rows.iter().all(|row| row.size == hf_server::FIXTURE.len() as u64), "{rows:?}");

    let installed = installed.unwrap();
    assert!(installed.cuda && installed.tensorrt);
    assert_eq!(installed.runtime, installed_dir(&server, &Dependency::Runtime));
}

#[tokio::test]
async fn an_nvidia_machine_on_a_platform_the_libraries_are_not_published_for_is_offered_none_and_still_initializes() {
    // As on Windows arm64: the runtime is published, no NVIDIA archive is. Hardware says CUDA; there is nothing to
    // install, so the providers are unsupported rather than the launch failed, and the rows leave them out.
    let server = HfServer::start();
    for which in [Dependency::Cuda, Dependency::Cudnn, Dependency::TensorRt] {
        server.unpublish(&which);
    }
    let (rows, installed) = planned(every_dependency(&server)).await;

    assert_eq!(rows.iter().map(|row| row.dependency.clone()).collect::<Vec<_>>(), [Dependency::Runtime]);
    let installed = installed.unwrap();
    assert!(!installed.cuda && !installed.tensorrt);
    let providers = installed.providers();
    assert!(providers.cpu && !providers.cuda && !providers.tensorrt);
    assert!(!server.root().join("cuda").exists(), "nothing is created for an unpublished library");
}

#[tokio::test]
async fn a_platform_the_runtime_is_not_published_for_fails_naming_it_and_downloads_nothing() {
    let server = HfServer::start();
    server.unpublish(&Dependency::Runtime);
    let (rows, installed) = planned(runtime_only(&server)).await;

    let error = installed.unwrap_err();
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    assert!(
        matches!(error, InitError::UnsupportedPlatform { dependency: "ONNX Runtime", os: o, arch: a } if o == os && a == arch),
        "got {error:?}"
    );
    assert!(rows.is_empty(), "a plan was reported for an initialization that could not plan");
    assert!(server.downloads().is_empty());
}
