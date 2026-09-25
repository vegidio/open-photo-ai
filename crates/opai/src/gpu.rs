//! What graphics adapters this machine has, and which NVIDIA libraries they can use.

// The probe and the classification are deliberately separate. The classifiers are pure functions of an adapter, so
// every branch below is covered against constructed values on a runner with no GPU at all — which is every runner
// this project has.

use std::sync::OnceLock;

use rust_sak::sysinfo::{self, GpuInfo, SysinfoError};

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

/// Whether `gpu` is an NVIDIA adapter TensorRT can build for: an RTX-branded card, of any product line, that
/// [`is_nvidia`] recognises.
///
/// It misses TensorRT-capable NVIDIA hardware carrying no RTX name at all — the GTX 16-series, the data-centre parts
/// (T4, A100, L40S, H100) and the Jetson modules. They stay on CUDA, which works.
pub(crate) fn is_tensorrt_capable(gpu: &GpuInfo) -> bool {
    // **The test is the RTX brand, not the Go original's `rtx 20|30|40|50`.** Those four substrings match the GeForce
    // naming and miss every professional card — `RTX A6000`, `RTX 5000 Ada Generation`, `RTX PRO 6000 Blackwell`,
    // `Quadro RTX 6000`, `TITAN RTX` — all of which are Turing or newer and which the Go rule offers CUDA only.
    //
    // It is not a loosening of the requirement, it *is* the requirement: the pinned TensorRT 10.14 needs compute
    // capability 7.5 or newer — Turing and up, Volta having been deprecated in 10.0 and removed in 10.5 — and NVIDIA
    // introduced the RTX brand with Turing. "Named RTX" and "at least Turing" are therefore the same set for
    // RTX-branded hardware, so one substring is more accurate than an enumeration and needs no edit when the next line
    // ships.
    //
    // A bare substring also reads correctly on all three backends, which name cards differently: DXGI reports
    // `NVIDIA RTX A6000`, while Linux takes the model from `pci.ids` and reports `GA102GL [RTX A6000]` with the vendor
    // carried separately. A rule anchored on a leading vendor word, or on a `GeForce`/`Quadro` prefix, would work on
    // one and not the other.
    //
    // The cards it misses are missed deliberately: each would need its own rule with its own over-match risk, and none
    // can be written without the hardware in hand to see what the probe reports for it.
    //
    // Unlike the Go original this shares `is_nvidia` rather than requiring an exact vendor match of its own. The
    // reference argues for that asymmetry nowhere, and it offers a Linux machine with no `pci.ids` database — vendor
    // absent, name still `NVIDIA GeForce RTX 4090` — CUDA and not TensorRT.
    is_nvidia(gpu) && contains_ignore_case(&gpu.name, "rtx")
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The two spellings one NVIDIA card has: the way DXGI reports it, and the way Linux takes it from `pci.ids` —
    /// the model bracketed after a codename, with the vendor carried separately.
    ///
    /// Both are fed through every classifier and asserted to agree, because a rule anchored on a leading vendor word
    /// or on a `GeForce`/`Quadro` prefix would pass on one platform and fail on the other.
    const NVIDIA_SPELLINGS: &[(&str, &str)] = &[
        // Consumer RTX.
        ("NVIDIA GeForce RTX 4090", "AD102 [GeForce RTX 4090]"),
        ("NVIDIA GeForce RTX 3090", "GA102 [GeForce RTX 3090]"),
        // Professional RTX, every one of which the Go rule leaves on CUDA.
        ("NVIDIA RTX A6000", "GA102GL [RTX A6000]"),
        ("NVIDIA RTX 5000 Ada Generation", "AD102GL [RTX 5000 Ada Generation]"),
        ("NVIDIA RTX PRO 6000 Blackwell", "GB202GL [RTX PRO 6000 Blackwell]"),
        ("NVIDIA Quadro RTX 6000", "TU102GL [Quadro RTX 6000/8000]"),
    ];

    #[test]
    fn every_rtx_card_is_nvidia_and_tensorrt_capable_however_its_platform_spells_it() {
        for (dxgi, pci_ids) in NVIDIA_SPELLINGS {
            // The Linux spelling carries the vendor in its own field, so the name alone never says "NVIDIA".
            let spellings = [adapter(dxgi, "NVIDIA"), adapter(pci_ids, "NVIDIA")];

            for gpu in &spellings {
                assert!(is_nvidia(gpu), "{} was not recognised as NVIDIA", gpu.name);
                assert!(is_tensorrt_capable(gpu), "{} was not recognised as TensorRT-capable", gpu.name);
            }

            assert_eq!(
                is_tensorrt_capable(&spellings[0]),
                is_tensorrt_capable(&spellings[1]),
                "the two spellings of {dxgi} disagree"
            );
        }
    }

    #[test]
    fn a_pre_rtx_nvidia_card_gets_cuda_and_not_tensorrt() {
        // Pascal: CUDA-capable, below the compute capability the pinned TensorRT requires.
        for gpu in [
            adapter("NVIDIA GeForce GTX 1080 Ti", "NVIDIA"),
            adapter("GP102 [GeForce GTX 1080 Ti]", "NVIDIA"),
        ] {
            assert!(is_nvidia(&gpu), "{}", gpu.name);
            assert!(!is_tensorrt_capable(&gpu), "{} was offered TensorRT", gpu.name);
        }
    }

    #[test]
    fn a_non_nvidia_adapter_gets_neither() {
        for gpu in [
            adapter("AMD Radeon RX 7900 XTX", "AMD"),
            adapter("Navi 31 [Radeon RX 7900 XT/7900 XTX]", "AMD"),
            adapter("Apple M2 Max", "Apple"),
            adapter("Intel(R) UHD Graphics 630", "Intel"),
        ] {
            assert!(!is_nvidia(&gpu), "{} was recognised as NVIDIA", gpu.name);
            assert!(!is_tensorrt_capable(&gpu), "{} was offered TensorRT", gpu.name);
        }
    }

    #[test]
    fn an_nvidia_adapter_with_no_vendor_is_still_nvidia() {
        // Linux with no `pci.ids` database: the vendor field is empty and the model name is the only evidence. Both
        // classifiers share `is_nvidia`, so this card is offered TensorRT as well as CUDA, where the Go original offers
        // it CUDA only.
        let gpu = unnamed_vendor("NVIDIA GeForce RTX 4090");

        assert!(is_nvidia(&gpu));
        assert!(is_tensorrt_capable(&gpu));
    }

    #[test]
    fn an_unnamed_vendor_on_an_rtx_name_that_does_not_say_nvidia_is_not_claimed() {
        // The `pci.ids` spelling with the vendor lost too: nothing left identifies the manufacturer, so neither
        // library is installed. Better than guessing from "RTX" alone, which is a brand another vendor could use.
        let gpu = unnamed_vendor("GA102GL [RTX A6000]");

        assert!(!is_nvidia(&gpu));
        assert!(!is_tensorrt_capable(&gpu));
    }

    #[test]
    fn a_machine_with_no_adapters_offers_nothing() {
        let none: &[GpuInfo] = &[];

        assert!(!none.iter().any(is_nvidia));
        assert!(!none.iter().any(is_tensorrt_capable));
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
