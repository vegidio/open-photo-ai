//! What graphics adapters this machine has and what its NVIDIA driver reports, each probed once.
//!
//! Which NVIDIA libraries those answers qualify for is `rust-sak`'s `ai::gpu::check_with`, judged in
//! [`crate::setup::select`]; what is here is the memoised probing, and [`is_nvidia`] for the records that name a card.

use std::sync::OnceLock;

use rust_sak::sysinfo::{self, CudaInfo, GpuInfo, SysinfoError};

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

/// The total memory of CUDA's device 0, the one the CUDA and TensorRT providers are pinned to open, in bytes — or
/// `None` on a machine with no CUDA device, or whose driver did not say.
///
/// The card's whole framebuffer, as the driver reports it, not what is free: that changes with every session held
/// and every other program on the card, so it would size a build for a moment rather than for the device.
pub(crate) fn cuda_device_memory() -> Option<u64> {
    cuda()?.devices.first()?.total_memory
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
        driver_version: crate::deps::folders::CUDA_REQUIREMENT.min_driver,
        devices: vec![rust_sak::sysinfo::CudaDevice { name: name.to_string(), compute_capability, total_memory: None }],
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
    fn the_vendor_match_ignores_case_as_every_backend_spells_it_differently() {
        for vendor in ["NVIDIA", "nvidia", "NVidia"] {
            assert!(is_nvidia(&adapter("GA102 [GeForce RTX 3090]", vendor)), "{vendor}");
        }
    }

    #[test]
    fn probing_the_running_machine_answers_rather_than_failing() {
        // Whatever this runner has — and every CI runner has no NVIDIA card — the probe must produce a list rather
        // than an error, since a machine whose adapters cannot be enumerated initializes on the CPU.
        let first = adapters();
        assert!(std::ptr::eq(first, adapters()), "the probe ran twice");
    }
}
