//! Which dependencies a machine is given, and what the plan reports before anything moves.

use super::*;

#[test]
fn a_geforce_rtx_machine_installs_everything() {
    let (names, providers) = selected(&[adapter("NVIDIA GeForce RTX 4090", "NVIDIA")], "linux", "x86_64");

    assert_eq!(names, vec!["onnx-runtime", "cuda", "cudnn", "tensorrt"]);
    assert!(providers.cuda && providers.tensorrt);
}

#[test]
fn a_professional_rtx_machine_installs_everything_too() {
    // Cards the Go rule's `rtx 20|30|40|50` substrings missed, all Turing or newer; spelled as Linux reads
    // them from `pci.ids`, vendor carried separately.
    for name in ["GA102GL [RTX A6000]", "AD102GL [RTX 5000 Ada Generation]", "TU102GL [Quadro RTX 6000/8000]"] {
        let (names, providers) = selected(&[adapter(name, "NVIDIA")], "linux", "x86_64");

        assert_eq!(names, vec!["onnx-runtime", "cuda", "cudnn", "tensorrt"], "{name}");
        assert!(providers.cuda && providers.tensorrt, "{name}");
    }
}

#[test]
fn a_non_rtx_nvidia_machine_installs_cuda_and_cudnn_but_not_tensorrt() {
    let (names, providers) = selected(&[adapter("NVIDIA GeForce GTX 1080 Ti", "NVIDIA")], "windows", "x86_64");

    assert_eq!(names, vec!["onnx-runtime", "cuda", "cudnn"]);
    assert!(providers.cuda);
    assert!(!providers.tensorrt, "a pre-Turing card was offered TensorRT");
}

#[test]
fn a_machine_with_no_nvidia_adapter_installs_only_the_runtime() {
    for adapters in [vec![adapter("AMD Radeon RX 7900 XTX", "AMD")], vec![adapter("Apple M2 Max", "Apple")], vec![]] {
        let (names, providers) = selected(&adapters, "linux", "x86_64");

        assert_eq!(names, vec!["onnx-runtime"], "{adapters:?}");
        assert!(!providers.cuda && !providers.tensorrt, "{adapters:?}");
    }
}

#[test]
fn an_nvidia_machine_on_an_unpublished_platform_is_offered_nothing_and_still_initializes() {
    // `windows_arm64`: runtime published, no NVIDIA archive. Hardware says CUDA; there's nothing to
    // install, so the provider is unsupported rather than the launch failed.
    let (names, providers) = selected(&[adapter("NVIDIA GeForce RTX 4090", "NVIDIA")], "windows", "aarch64");

    assert_eq!(names, vec!["onnx-runtime"]);
    assert!(!providers.cuda, "a provider with no published archive was claimed");
    assert!(!providers.tensorrt);
    assert!(providers.cpu, "the CPU is always available");
}

#[test]
fn a_machine_with_no_nvidia_adapter_is_told_it_needs_the_runtime_alone() {
    for adapters in [vec![adapter("AMD Radeon RX 7900 XTX", "AMD")], vec![adapter("Apple M2 Max", "Apple")], vec![]] {
        let rows = planned(&adapters, "macos", "aarch64");

        assert_eq!(
            rows,
            vec![PlannedDependency {
                dependency: Dependency::Runtime,
                size: pinned_size(&ONNX_RUNTIME, "macos", "aarch64"),
            }],
            "{adapters:?}"
        );
    }
}

#[test]
fn an_rtx_machine_is_told_it_needs_all_four_in_install_order_with_their_published_sizes() {
    // The multi-row case, which can't be produced on a developer's own macOS machine — driven from a
    // synthetic adapter list instead.
    let rows = planned(&[adapter("NVIDIA GeForce RTX 4090", "NVIDIA")], "linux", "x86_64");

    assert_eq!(
        rows,
        vec![
            PlannedDependency { dependency: Dependency::Runtime, size: pinned_size(&ONNX_RUNTIME, "linux", "x86_64") },
            PlannedDependency { dependency: Dependency::Cuda, size: pinned_size(&CUDA, "linux", "x86_64") },
            PlannedDependency { dependency: Dependency::Cudnn, size: pinned_size(&CUDNN, "linux", "x86_64") },
            PlannedDependency { dependency: Dependency::TensorRt, size: pinned_size(&TENSORRT, "linux", "x86_64") },
        ]
    );

    // The reported order is the install order, not two lists that happen to agree.
    let plan = Opai::select_at("http://example.invalid", &[adapter("RTX A6000", "NVIDIA")], "linux", "x86_64").unwrap();
    let installed: Vec<Dependency> = plan.all().map(|descriptor| descriptor.progress.clone()).collect();
    let reported: Vec<Dependency> = plan.rows().into_iter().map(|row| row.dependency).collect();
    assert_eq!(installed, reported);
}

#[test]
fn a_non_rtx_nvidia_machine_is_told_it_needs_the_runtime_cuda_and_cudnn() {
    let rows = planned(&[adapter("NVIDIA GeForce GTX 1080 Ti", "NVIDIA")], "windows", "x86_64");

    let named: Vec<&Dependency> = rows.iter().map(|row| &row.dependency).collect();
    assert_eq!(named, vec![&Dependency::Runtime, &Dependency::Cuda, &Dependency::Cudnn]);
    assert!(rows.iter().all(|row| row.size > 0), "a row carried no published size: {rows:?}");
}

#[test]
fn a_platform_with_no_published_runtime_fails_the_selection() {
    // The one case still an error: the application cannot run without the runtime.
    let error = Opai::select_at("http://example.invalid", &[], "macos", "x86_64").unwrap_err();

    assert!(
        matches!(error, InitError::UnsupportedPlatform { dependency: "onnx-runtime", .. }),
        "got {error:?}"
    );
}

#[test]
fn the_running_platform_selects_without_error() {
    let plan = Opai::select(gpu::adapters(), std::env::consts::OS, std::env::consts::ARCH).unwrap();

    assert_eq!(plan.runtime.name, "onnx-runtime");
    assert!(plan.providers().cpu);
}

#[test]
fn a_plan_reports_a_provider_exactly_where_one_of_its_rows_unlocks_it() {
    let server_less = "http://example.invalid";
    let machines = [
        vec![adapter("NVIDIA GeForce RTX 4090", "NVIDIA")],
        vec![adapter("NVIDIA GeForce GTX 1080 Ti", "NVIDIA")],
        vec![adapter("AMD Radeon RX 7900 XTX", "AMD")],
        vec![],
    ];

    for adapters in machines {
        let plan = Opai::select_at(server_less, &adapters, "linux", "x86_64").unwrap();
        let report = plan.providers();

        let unlocks = |provider| plan.gpu.iter().any(|descriptor| descriptor.provides == Some(provider));
        assert_eq!(report.cuda, unlocks(Provider::Cuda), "{adapters:?}");
        assert_eq!(report.tensorrt, unlocks(Provider::TensorRt), "{adapters:?}");
    }
}

#[test]
fn a_plan_built_with_a_gpu_row_always_reports_that_rows_provider() {
    let runtime = Descriptor::from_release_at("http://example.invalid", &ONNX_RUNTIME, "linux", "x86_64").unwrap();
    let row = |release| Descriptor::from_release_at("http://example.invalid", release, "linux", "x86_64").unwrap();

    for (release, provider) in [(&CUDA, Provider::Cuda), (&TENSORRT, Provider::TensorRt)] {
        let plan = Plan { runtime: runtime.clone(), gpu: vec![row(release)] };

        assert_eq!(plan.providers(), SupportedProviders::detect().with(provider), "{}", release.name);
    }

    // cuDNN alone unlocks nothing: it's a prerequisite of CUDA, not a provider.
    let plan = Plan { runtime, gpu: vec![row(&CUDNN)] };
    assert_eq!(plan.providers(), SupportedProviders::detect());
}

#[test]
fn a_plan_installs_the_runtime_first_and_the_gpu_libraries_in_table_order() {
    let plan =
        Opai::select_at("http://example.invalid", &[adapter("NVIDIA GeForce RTX 4090", "NVIDIA")], "linux", "x86_64")
            .unwrap();

    let names: Vec<&str> = plan.all().map(|descriptor| descriptor.name.as_str()).collect();
    assert_eq!(names, vec!["onnx-runtime", "cuda", "cudnn", "tensorrt"]);
    assert_eq!(plan.all().next().map(|descriptor| descriptor.name.as_str()), Some(plan.runtime.name.as_str()));
}

#[test]
fn a_card_too_old_for_the_pinned_tensorrt_says_so_rather_than_looking_like_a_failed_install() {
    // From outside, CUDA-without-TensorRT looks identical to a failed install — this is the line that tells
    // them apart, at `info` since the outcome is correct rather than degraded.
    let (log, ()) = logging::records_of_blocking("info", || {
        let plan = Opai::select_at(
            "http://example.invalid",
            &[adapter("NVIDIA GeForce GTX 1080 Ti", "NVIDIA")],
            "windows",
            "x86_64",
        )
        .unwrap();

        assert!(plan.providers().cuda && !plan.providers().tensorrt);
    });

    let too_old = log
        .lines()
        .find(|line| line.contains("no adapter is new enough for the pinned TensorRT release"))
        .unwrap_or_else(|| panic!("the refusal was not recorded:\n{log}"));

    assert!(too_old.contains("level=INFO"), "{too_old}");
    assert!(too_old.contains("GTX 1080 Ti"), "the record does not name the card: {too_old}");

    // Neither an RTX machine nor one with no NVIDIA card should log anything here.
    for adapters in [vec![adapter("NVIDIA GeForce RTX 4090", "NVIDIA")], vec![], vec![adapter("Apple M3", "Apple")]] {
        let (quiet, ()) = logging::records_of_blocking("info", || {
            Opai::select_at("http://example.invalid", &adapters, "windows", "x86_64").unwrap();
        });
        assert!(!quiet.contains("no adapter is new enough"), "{adapters:?} recorded a refusal:\n{quiet}");
    }
}

#[test]
fn a_probe_that_fails_is_a_warning_and_a_machine_with_no_adapters() {
    // The probe is a process-wide `OnceLock`, so a failure can't be staged through `gpu::adapters` in a
    // test sharing the process — its answer goes to [`gpu::probed`] instead.
    let error = rust_sak::sysinfo::SysinfoError::UnsupportedPlatform { os: "haiku" };
    let (log, memoised) = logging::records_of_blocking("info", || gpu::probed(Err(error)));

    assert!(memoised.is_empty(), "a failed probe memoised adapters it never saw");

    let failed = log
        .lines()
        .find(|line| line.contains("the graphics adapters could not be enumerated"))
        .unwrap_or_else(|| panic!("{log}"));
    assert!(failed.contains("level=WARN"), "{failed}");
    assert!(failed.contains("error="), "the record does not say what the probe said: {failed}");

    // A successful probe logs nothing — the warning marks only the fallback.
    let (quiet, probed) = logging::records_of_blocking("debug", || gpu::probed(Ok(vec![adapter("Apple M3", "Apple")])));
    assert_eq!(probed.len(), 1);
    assert!(!quiet.contains("msg="), "a probe that answered wrote a record:\n{quiet}");

    // And what a failure memoises (an empty list) still selects a runnable, CPU-only plan.
    let plan = Opai::select_at("http://example.invalid", &memoised, "windows", "x86_64").unwrap();

    assert_eq!(plan.all().count(), 1, "a machine with no adapters was offered a GPU library");
    assert!(plan.providers().cpu);
    assert!(!plan.providers().cuda && !plan.providers().tensorrt);
}
