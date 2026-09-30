//! What graphics adapters this machine has, and which NVIDIA libraries they can use.

// The probe and the classification are deliberately separate. The classifiers are pure functions of an adapter, so
// every branch below is covered against constructed values on a runner with no GPU at all — which is every runner
// this project has.

use std::fmt;
use std::sync::OnceLock;

use rust_sak::sysinfo::{self, CudaInfo, GpuInfo, SysinfoError};

use crate::deps::artifact::{CUDA_MIN_COMPUTE_CAPABILITY, CUDA_MIN_DRIVER, TENSORRT_MIN_COMPUTE_CAPABILITY};

/// The machine's graphics adapters, probed once.
///
/// A probe that fails is memoised as an empty list, so a machine whose adapters cannot be enumerated initializes on
/// the CPU rather than failing — including one on an operating system `rust-sak` has no GPU backend for, which is a
/// `SysinfoError` and not an empty result.
pub(crate) fn adapters() -> &'static [GpuInfo] {
    // `OnceLock` rather than a probe per question, matching the Go original's `sync.OnceValues` and for the same
    // reason: more than one caller wants the answer, and short of hot-plugging an eGPU it cannot change while the
    // process runs.
    static ADAPTERS: OnceLock<Vec<GpuInfo>> = OnceLock::new();

    ADAPTERS.get_or_init(|| probed(sysinfo::gpu_info()))
}

/// The list [`adapters`] memoises for `outcome`, warning about a probe that failed.
pub(crate) fn probed(outcome: Result<Vec<GpuInfo>, SysinfoError>) -> Vec<GpuInfo> {
    // Split out so the branch is driven rather than described. The memoised answer is a process-wide `OnceLock` and
    // the failure is memoised with it, so the first caller in a test binary decides it for every other — a failing
    // probe cannot be staged in a suite that shares a process. Taking the probe's answer as a parameter is what lets a
    // test hand this an `Err` and assert on the line the formatter actually wrote, so that deleting or rewording the
    // record below fails that test instead of leaving it passing against a message nothing writes any more.
    outcome.unwrap_or_else(|error| {
        // `warn`: nothing failed that a user will see, and every GPU provider is about to be reported unsupported on
        // a machine that may well have a card. Written once per process, since the answer is memoised — the failure
        // included.
        tracing::warn!(%error, "the graphics adapters could not be enumerated; continuing as a machine with none");
        Vec::new()
    })
}

/// Whether `gpu` is an NVIDIA adapter, which is what CUDA and cuDNN are installed for: its vendor or its name says
/// `NVIDIA`, and either is enough.
///
/// A statement about the hardware, not about the driver: a card whose driver is missing or too old still matches, and
/// the failure surfaces later as a session that cannot be built on that provider.
pub(crate) fn is_nvidia(gpu: &GpuInfo) -> bool {
    // DXGI and IOKit report a vendor; Linux takes it from `pci.ids`, which a machine may not have installed — and there
    // the model name still says `NVIDIA` even when the vendor field is empty.
    let vendor_is_nvidia = gpu.vendor.as_deref().is_some_and(|vendor| vendor.eq_ignore_ascii_case("nvidia"));
    vendor_is_nvidia || contains_ignore_case(&gpu.name, "nvidia")
}

/// What the NVIDIA driver offers CUDA, probed once — and only on a machine [`adapters`] says has an NVIDIA card.
///
/// The probe initialises the CUDA driver, which is not free and wakes the discrete GPU on a hybrid laptop, so a machine
/// with no NVIDIA adapter never pays for it.
pub(crate) fn cuda() -> Option<&'static CudaInfo> {
    static CUDA: OnceLock<Option<CudaInfo>> = OnceLock::new();

    CUDA.get_or_init(|| adapters().iter().any(is_nvidia).then(sysinfo::cuda_info).flatten())
        .as_ref()
}

/// Whether the pinned CUDA release can run on this machine, and if not, why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CudaSupport {
    /// The driver and the device CUDA runs on both meet the pinned release's minimums.
    Supported,
    /// No CUDA driver answered: none is installed, or it is too old to report a version.
    NoDriver,
    /// The driver answered but reported no device it can run on.
    NoDevice { driver: (u32, u32) },
    /// The driver supports an older CUDA than the pinned release needs.
    DriverTooOld { driver: (u32, u32) },
    /// The device CUDA runs on is older than the pinned release supports.
    DeviceTooOld { name: String, compute_capability: (u32, u32) },
}

impl fmt::Display for CudaSupport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let version = |(major, minor): (u32, u32)| format!("{major}.{minor}");

        match self {
            Self::Supported => f.write_str("supported"),
            Self::NoDriver => f.write_str("no CUDA driver answered"),
            Self::NoDevice { driver } => write!(f, "the CUDA {} driver reports no usable device", version(*driver)),
            Self::DriverTooOld { driver } => write!(
                f,
                "the driver supports CUDA {}, and the pinned release needs {}",
                version(*driver),
                version(CUDA_MIN_DRIVER)
            ),
            Self::DeviceTooOld { name, compute_capability } => write!(
                f,
                "{name} has compute capability {}, and the pinned release needs {}",
                version(*compute_capability),
                version(CUDA_MIN_COMPUTE_CAPABILITY)
            ),
        }
    }
}

/// Whether the pinned CUDA release can run with what the driver reported.
///
/// The device judged is CUDA's device 0, because that is the one the CUDA and TensorRT providers are pinned to open. A
/// machine whose second card would qualify still fails every session on the first, so it is not offered CUDA either.
pub(crate) fn cuda_support(cuda: Option<&CudaInfo>) -> CudaSupport {
    let Some(cuda) = cuda else {
        return CudaSupport::NoDriver;
    };

    if cuda.driver_version < CUDA_MIN_DRIVER {
        return CudaSupport::DriverTooOld { driver: cuda.driver_version };
    }

    match cuda.devices.first() {
        None => CudaSupport::NoDevice { driver: cuda.driver_version },
        Some(device) if device.compute_capability < CUDA_MIN_COMPUTE_CAPABILITY => {
            CudaSupport::DeviceTooOld { name: device.name.clone(), compute_capability: device.compute_capability }
        }
        Some(_) => CudaSupport::Supported,
    }
}

/// Whether the pinned TensorRT release can build engines on this machine, and if not, why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TensorRtSupport {
    /// The device the providers open meets the pinned release's minimum.
    Supported,
    /// The driver reported no device at all, so there is nothing to build for.
    NoDevice,
    /// The device the providers open is older than the pinned release supports.
    DeviceTooOld { name: String, compute_capability: (u32, u32) },
}

impl fmt::Display for TensorRtSupport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Supported => f.write_str("supported"),
            Self::NoDevice => f.write_str("the driver reports no usable device"),
            Self::DeviceTooOld { name, compute_capability: (major, minor) } => {
                let (need_major, need_minor) = TENSORRT_MIN_COMPUTE_CAPABILITY;
                write!(
                    f,
                    "{name} has compute capability {major}.{minor}, and the pinned release needs {need_major}.{need_minor}"
                )
            }
        }
    }
}

/// Whether the pinned TensorRT release can build engines with what the driver reported.
///
/// Read off the compute capability of CUDA's device 0 — the one the TensorRT provider is pinned to open — rather than off
/// the adapter's name. The name was a proxy that only held for the RTX brand: it missed every TensorRT-capable card
/// sold without it (the GTX 16-series, the data-centre parts such as the T4, A100 and H100, the Jetson modules), and
/// said nothing about which card the provider would actually open on a machine with two.
///
/// Only asked where [`cuda_support`] already passed, since TensorRT installs beside CUDA and never without it. That is
/// also what holds TensorRT to a driver new enough to run it.
pub(crate) fn tensorrt_support(cuda: Option<&CudaInfo>) -> TensorRtSupport {
    match cuda.and_then(|cuda| cuda.devices.first()) {
        None => TensorRtSupport::NoDevice,
        Some(device) if device.compute_capability < TENSORRT_MIN_COMPUTE_CAPABILITY => {
            TensorRtSupport::DeviceTooOld { name: device.name.clone(), compute_capability: device.compute_capability }
        }
        Some(_) => TensorRtSupport::Supported,
    }
}

/// Case-insensitive substring test, without allocating a lowercased copy of either side.
///
/// ASCII case is folded on both sides; every caller here passes a literal as `needle`.
fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

/// An adapter as a backend that reports a vendor describes it — DXGI on Windows, IOKit on macOS.
#[cfg(test)]
pub(crate) fn adapter(name: &str, vendor: &str) -> GpuInfo {
    // Here rather than in each test module that wants one: the selection tests in `lib.rs` construct the same adapters
    // these classifier tests do, and a second copy is a second thing to keep in step with `GpuInfo`.
    GpuInfo { name: name.to_string(), vendor: Some(vendor.to_string()), memory: None }
}

/// An adapter with no vendor, as Linux reports one on a machine with no `pci.ids` database.
#[cfg(test)]
pub(crate) fn unnamed_vendor(name: &str) -> GpuInfo {
    GpuInfo { name: name.to_string(), vendor: None, memory: None }
}

/// What a current driver reports for one device of `compute_capability`.
#[cfg(test)]
pub(crate) fn cuda_device(name: &str, compute_capability: (u32, u32)) -> CudaInfo {
    CudaInfo {
        driver_version: CUDA_MIN_DRIVER,
        devices: vec![rust_sak::sysinfo::CudaDevice { name: name.to_string(), compute_capability }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two spellings one NVIDIA card has: the way DXGI reports it, and the way Linux takes it from `pci.ids` —
    /// the model bracketed after a codename, with the vendor carried separately.
    ///
    /// Both are fed through [`is_nvidia`] and asserted to agree, because a rule anchored on a leading vendor word or on
    /// a `GeForce`/`Quadro` prefix would pass on one platform and fail on the other.
    const NVIDIA_SPELLINGS: &[(&str, &str)] = &[
        ("NVIDIA GeForce RTX 4090", "AD102 [GeForce RTX 4090]"),
        ("NVIDIA RTX A6000", "GA102GL [RTX A6000]"),
        ("NVIDIA GeForce GTX 1660 SUPER", "TU116 [GeForce GTX 1660 SUPER]"),
        ("NVIDIA GeForce GTX 1080 Ti", "GP102 [GeForce GTX 1080 Ti]"),
        ("NVIDIA A100-SXM4-80GB", "GA100 [A100 SXM4 80GB]"),
    ];

    #[test]
    fn every_nvidia_card_is_recognised_however_its_platform_spells_it() {
        for (dxgi, pci_ids) in NVIDIA_SPELLINGS {
            // The Linux spelling carries the vendor in its own field, so the name alone never says "NVIDIA".
            for gpu in [adapter(dxgi, "NVIDIA"), adapter(pci_ids, "NVIDIA")] {
                assert!(is_nvidia(&gpu), "{} was not recognised as NVIDIA", gpu.name);
            }
        }
    }

    #[test]
    fn a_non_nvidia_adapter_is_not_nvidia() {
        for gpu in [
            adapter("AMD Radeon RX 7900 XTX", "AMD"),
            adapter("Navi 31 [Radeon RX 7900 XT/7900 XTX]", "AMD"),
            adapter("Apple M2 Max", "Apple"),
            adapter("Intel(R) UHD Graphics 630", "Intel"),
        ] {
            assert!(!is_nvidia(&gpu), "{} was recognised as NVIDIA", gpu.name);
        }
    }

    #[test]
    fn an_nvidia_adapter_with_no_vendor_is_still_nvidia() {
        // Linux with no `pci.ids` database: the vendor field is empty and the model name is the only evidence.
        assert!(is_nvidia(&unnamed_vendor("NVIDIA GeForce RTX 4090")));
    }

    #[test]
    fn an_unnamed_vendor_on_a_name_that_does_not_say_nvidia_is_not_claimed() {
        // The `pci.ids` spelling with the vendor lost too: nothing left identifies the manufacturer, so the driver is
        // never asked. Better than guessing from "RTX" alone, which is a brand another vendor could use.
        assert!(!is_nvidia(&unnamed_vendor("GA102GL [RTX A6000]")));
    }

    #[test]
    fn a_machine_with_no_adapters_has_no_nvidia_card() {
        let none: &[GpuInfo] = &[];

        assert!(!none.iter().any(is_nvidia));
    }

    #[test]
    fn tensorrt_builds_for_turing_and_newer_whatever_the_card_is_called() {
        // The cards the RTX-name rule missed come first: Turing and newer, sold without the brand.
        for (name, compute_capability) in [
            ("NVIDIA GeForce GTX 1660 SUPER", (7, 5)),
            ("NVIDIA GeForce GTX 1650", (7, 5)),
            ("Tesla T4", (7, 5)),
            ("NVIDIA A100-SXM4-80GB", (8, 0)),
            ("NVIDIA L40S", (8, 9)),
            ("NVIDIA H100 80GB HBM3", (9, 0)),
            ("Orin (nvgpu)", (8, 7)),
            ("NVIDIA GeForce RTX 2060", (7, 5)),
            ("NVIDIA RTX A6000", (8, 6)),
            ("NVIDIA GeForce RTX 5090", (12, 0)),
        ] {
            let cuda = cuda_device(name, compute_capability);
            assert_eq!(tensorrt_support(Some(&cuda)), TensorRtSupport::Supported, "{name}");
        }
    }

    #[test]
    fn tensorrt_is_refused_below_turing_and_says_which_card() {
        for (name, compute_capability) in [("Tesla V100-SXM2-16GB", (7, 0)), ("NVIDIA TITAN V", (7, 0))] {
            let support = tensorrt_support(Some(&cuda_device(name, compute_capability)));

            assert_eq!(support, TensorRtSupport::DeviceTooOld { name: name.to_string(), compute_capability });
            assert_eq!(
                support.to_string(),
                format!("{name} has compute capability 7.0, and the pinned release needs 7.5")
            );
        }
    }

    #[test]
    fn tensorrt_is_judged_on_device_zero_which_the_provider_opens() {
        let mut cuda = cuda_device("Tesla V100-SXM2-16GB", (7, 0));
        cuda.devices.push(rust_sak::sysinfo::CudaDevice {
            name: "NVIDIA GeForce RTX 4090".to_string(),
            compute_capability: (8, 9),
        });

        assert!(matches!(tensorrt_support(Some(&cuda)), TensorRtSupport::DeviceTooOld { .. }));

        cuda.devices.reverse();
        assert_eq!(tensorrt_support(Some(&cuda)), TensorRtSupport::Supported);
    }

    #[test]
    fn tensorrt_is_refused_with_no_device_to_build_for() {
        assert_eq!(tensorrt_support(None), TensorRtSupport::NoDevice);

        let empty = CudaInfo { driver_version: CUDA_MIN_DRIVER, devices: Vec::new() };
        assert_eq!(tensorrt_support(Some(&empty)), TensorRtSupport::NoDevice);
    }

    #[test]
    fn tensorrt_never_asks_for_less_than_cuda() {
        // TensorRT only installs beside CUDA, so a minimum below CUDA's would be a promise the plan can never keep.
        assert!(TENSORRT_MIN_COMPUTE_CAPABILITY >= CUDA_MIN_COMPUTE_CAPABILITY);
    }

    #[test]
    fn the_vendor_match_ignores_case_as_every_backend_spells_it_differently() {
        for vendor in ["NVIDIA", "nvidia", "NVidia"] {
            assert!(is_nvidia(&adapter("GA102 [GeForce RTX 3090]", vendor)), "{vendor}");
        }
    }

    #[test]
    fn cuda_runs_on_turing_and_newer_under_a_current_driver() {
        for (name, compute_capability) in [
            ("NVIDIA GeForce GTX 1660 SUPER", (7, 5)),
            ("NVIDIA GeForce RTX 2060", (7, 5)),
            ("NVIDIA GeForce RTX 4090", (8, 9)),
            ("NVIDIA GeForce RTX 5090", (12, 0)),
        ] {
            assert_eq!(cuda_support(Some(&cuda_device(name, compute_capability))), CudaSupport::Supported, "{name}");
        }
    }

    #[test]
    fn cuda_is_refused_on_the_architectures_cuda_13_dropped() {
        // The three cards seen failing `cublasCreate` with `CUBLAS_STATUS_ARCH_MISMATCH` in production, plus Volta.
        for (name, compute_capability) in [
            ("NVIDIA GeForce 930MX", (5, 0)),
            ("NVIDIA GeForce GTX 1050 Ti", (6, 1)),
            ("NVIDIA GeForce GTX 1060", (6, 1)),
            ("Tesla V100-SXM2-16GB", (7, 0)),
        ] {
            let support = cuda_support(Some(&cuda_device(name, compute_capability)));

            assert_eq!(support, CudaSupport::DeviceTooOld { name: name.to_string(), compute_capability }, "{name}");
            assert!(support.to_string().contains(name), "the reason does not name the card: {support}");
        }
    }

    #[test]
    fn cuda_is_refused_under_a_driver_older_than_the_pinned_release() {
        let cuda = CudaInfo { driver_version: (12, 8), ..cuda_device("NVIDIA GeForce RTX 4090", (8, 9)) };

        let support = cuda_support(Some(&cuda));

        assert_eq!(support, CudaSupport::DriverTooOld { driver: (12, 8) });
        assert_eq!(support.to_string(), "the driver supports CUDA 12.8, and the pinned release needs 13.0");
    }

    #[test]
    fn cuda_is_refused_with_no_driver_or_no_device() {
        assert_eq!(cuda_support(None), CudaSupport::NoDriver);

        let empty = CudaInfo { driver_version: (13, 0), devices: Vec::new() };
        assert_eq!(cuda_support(Some(&empty)), CudaSupport::NoDevice { driver: (13, 0) });
    }

    #[test]
    fn cuda_is_judged_on_device_zero_which_the_providers_open() {
        // A new card behind an old one still fails every session: the providers are pinned to `device_id` 0.
        let mut cuda = cuda_device("NVIDIA GeForce GTX 1060", (6, 1));
        cuda.devices.push(rust_sak::sysinfo::CudaDevice {
            name: "NVIDIA GeForce RTX 4090".to_string(),
            compute_capability: (8, 9),
        });

        assert!(matches!(cuda_support(Some(&cuda)), CudaSupport::DeviceTooOld { .. }));

        cuda.devices.reverse();
        assert_eq!(cuda_support(Some(&cuda)), CudaSupport::Supported);
    }

    #[test]
    fn probing_the_running_machine_answers_rather_than_failing() {
        // Whatever this runner has — and every CI runner has no NVIDIA card — the probe must produce a list rather
        // than an error, since a machine whose adapters cannot be enumerated initializes on the CPU.
        let first = adapters();
        assert!(std::ptr::eq(first, adapters()), "the probe ran twice");
    }
}
