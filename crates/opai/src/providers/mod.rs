//! What a model is asked to run on, and the settings a session is built with.
//!
//! [`SupportedProviders`] is what the *machine* can offer — a report initialization folds out of the GPU install
//! plan. [`ExecutionProvider`] is what a *user* asked for, which a front end offers in a settings pane and hands back.
//! `Accelerator` is the narrower thing a session actually attaches and configures: the hardware providers,
//! without the request (`Auto`) or the fallback that takes no configuration (the CPU). [`options`] puts the request
//! and the machine together into the plan a session is built from, and [`profile::EpProfile`] carries everything a
//! measurement changes, declared by the variant it was measured against.

pub(crate) mod options;
pub(crate) mod profile;

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::InitError;

/// The execution providers a run can be asked for.
///
/// Public vocabulary, alongside [`Precision`](crate::Precision): a front end offers these, persists the choice and
/// hands it back. The spellings below are user-facing — they are what a settings file stores and what a command line
/// accepts — so they are stable, and both [`Display`](std::fmt::Display) and the serialized form render them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExecutionProvider {
    // No OpenVINO, which the reference implementation names: its appender returns *"the OpenVINO provider is disabled
    // in this build"* on every platform — so a machine resolving to it runs on CPU kernels and logs a decline, every
    // time. A provider that exists only to fail is not published here; an arm belongs here once one is actually built.
    /// Whichever of the providers below this machine supports, best first.
    Auto,
    /// The CPU. Always available, takes no configuration, and what every other provider falls back to.
    #[serde(rename = "CPU")]
    Cpu,
    /// Apple's CoreML, on macOS 12 or newer.
    #[serde(rename = "CoreML")]
    CoreMl,
    /// NVIDIA's CUDA provider.
    #[serde(rename = "CUDA")]
    Cuda,
    /// NVIDIA's TensorRT provider, which compiles an engine from the graph.
    #[serde(rename = "TensorRT")]
    TensorRt,
    /// The WebGPU plugin provider: any GPU the platform's native graphics API (Metal, D3D12, Vulkan) reaches.
    #[serde(rename = "WebGPU")]
    WebGpu,
}

impl ExecutionProvider {
    // The one source for anything that has to enumerate them — a settings pane, a CLI flag's help text, the parse
    // below — so a sixth arm is reachable everywhere by adding it here.
    /// Every published provider, in the order a user is most likely to reach for one.
    pub const ALL: [Self; 6] = [Self::Auto, Self::Cpu, Self::CoreMl, Self::Cuda, Self::TensorRt, Self::WebGpu];

    /// Every provider a session can be **built on**, in [`ALL`](Self::ALL) order: the CPU and each accelerator.
    /// [`Auto`](Self::Auto) is a request — "pick for me" — rather than something anything runs on.
    ///
    /// [`ALL`](Self::ALL) past its first entry, which is `Auto`, so the two lists cannot drift apart.
    pub(crate) const BUILT_ON: [Self; 5] = {
        assert!(matches!(Self::ALL[0], Self::Auto), "`ALL` must begin with the request");
        *Self::ALL.last_chunk().unwrap()
    };

    /// The accelerator this names, or `None` for the two that are not one: [`Auto`](Self::Auto), which is a request,
    /// and [`Cpu`](Self::Cpu), which is attached by nobody and configured with nothing — it is what the runtime falls
    /// back to on its own.
    ///
    /// The one place the request vocabulary is narrowed to the hardware one, so everything downstream of it — the
    /// attach list, the options it is configured with, the builder it reaches — holds an [`Accelerator`] and has no
    /// arm for either of the two to answer.
    pub(crate) const fn accelerator(self) -> Option<Accelerator> {
        match self {
            Self::Auto | Self::Cpu => None,
            Self::CoreMl => Some(Accelerator::CoreMl),
            Self::Cuda => Some(Accelerator::Cuda),
            Self::TensorRt => Some(Accelerator::TensorRt),
            Self::WebGpu => Some(Accelerator::WebGpu),
        }
    }

    /// The user-facing spelling: `Auto`, `CPU`, `CoreML`, `CUDA`, `TensorRT`, `WebGPU`.
    pub const fn as_str(self) -> &'static str {
        // Written once, here, and matched by the `serde` renames on the arms above, so a stored choice and a displayed
        // one cannot drift into two spellings.
        match self {
            Self::Auto => "Auto",
            Self::Cpu => "CPU",
            Self::CoreMl => "CoreML",
            Self::Cuda => "CUDA",
            Self::TensorRt => "TensorRT",
            Self::WebGpu => "WebGPU",
        }
    }
}

/// A provider a session attaches and configures: the hardware, without the request or the CPU.
///
/// Crate-internal, and deliberately not a replacement for [`ExecutionProvider`]: that is the public vocabulary a user
/// chooses from and a settings file stores, and it has to be able to say "pick for me" and "the CPU". This is what
/// the resolution narrows a request to, so the attach list, the options a provider is configured with and the builder
/// dispatch each match only the hardware arms rather than two more that must never happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Accelerator {
    /// Apple's CoreML.
    CoreMl,
    /// NVIDIA's CUDA provider.
    Cuda,
    /// NVIDIA's TensorRT provider.
    TensorRt,
    /// The WebGPU plugin provider, attached through its devices rather than by name.
    WebGpu,
}

impl From<Accelerator> for ExecutionProvider {
    fn from(accelerator: Accelerator) -> Self {
        match accelerator {
            Accelerator::CoreMl => Self::CoreMl,
            Accelerator::Cuda => Self::Cuda,
            Accelerator::TensorRt => Self::TensorRt,
            Accelerator::WebGpu => Self::WebGpu,
        }
    }
}

impl std::fmt::Display for Accelerator {
    /// The user-facing spelling of the provider it is: `CoreML`, `CUDA`, `TensorRT`, `WebGPU`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(ExecutionProvider::from(*self).as_str())
    }
}

impl std::fmt::Display for ExecutionProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ExecutionProvider {
    type Err = InitError;

    /// The provider `text` names, case-insensitively, or [`InitError::UnknownProvider`] where it names none.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        // The text comes from somewhere this application did not write: a settings file left by an older release, a
        // command line, a message across a process boundary. Case-insensitive because `CoreML` and `TensorRT` are not
        // what anyone types.
        //
        // An unknown name is refused rather than downgraded, which is the one thing here that is *not* softened into a
        // fallback: a valid name this machine cannot serve is an ordinary Tuesday and resolves to a CPU run, while a
        // name no build ever published is a bug in whatever produced it.
        Self::ALL
            .into_iter()
            .find(|provider| provider.as_str().eq_ignore_ascii_case(text))
            .ok_or_else(|| InitError::UnknownProvider { name: text.to_string() })
    }
}

/// The execution providers initialization found this machine can offer.
///
/// What the machine can be *asked* for, not a promise that a session will build: a provider reported as supported can
/// still fail at session-build time, and the application falls back to the next provider when it does.
///
/// `#[non_exhaustive]` reaches the wire too. A provider added later is a new field in the JSON, which a reader that
/// names the fields it knows tolerates, and one it does not offer until it is taught the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct SupportedProviders {
    // Booleans rather than a set of `ExecutionProvider`, which is what `supports` reads them as.
    //
    // `Serialize` and not `Deserialize`, on the same reasoning the catalogue's entries carry it: these four fields are
    // what a front end builds its provider list from, and this crate publishes the report rather than accepting one
    // back. A `cuda` a settings file claimed would be a machine describing itself from somewhere other than its own
    // install.
    //
    // `cpu` is a field rather than an implicit truth so a front end can render the list without a special case for
    // the one that is always there.
    /// Always supported. There is no machine the runtime cannot run on.
    pub cpu: bool,
    /// macOS 12 or newer.
    pub coreml: bool,
    /// The NVIDIA driver and device can run the pinned CUDA release *and* its libraries were installed for this
    /// platform.
    pub cuda: bool,
    /// CUDA is supported, the device can run the pinned TensorRT release *and* TensorRT was installed for this
    /// platform.
    pub tensorrt: bool,
    /// The WebGPU plugin installed with the runtime registered *and* offered at least one device.
    pub webgpu: bool,
}

impl SupportedProviders {
    /// Whether this machine supports `provider`.
    ///
    /// [`Auto`](ExecutionProvider::Auto) and [`Cpu`](ExecutionProvider::Cpu) are `true` on every machine — there is no
    /// machine the runtime cannot run on, so `Auto` always has at least the CPU to resolve to. An accelerator reads the
    /// flag the install plan set; see `offers`.
    pub fn supports(self, provider: ExecutionProvider) -> bool {
        provider.accelerator().is_none_or(|accelerator| self.offers(accelerator))
    }

    /// Whether this machine offers `accelerator`: the flag the install plan set for it, and no other.
    pub(crate) const fn offers(self, accelerator: Accelerator) -> bool {
        match accelerator {
            Accelerator::CoreMl => self.coreml,
            Accelerator::Cuda => self.cuda,
            Accelerator::TensorRt => self.tensorrt,
            Accelerator::WebGpu => self.webgpu,
        }
    }

    /// Every provider this machine can actually offer, in [`ExecutionProvider::ALL`] order.
    ///
    /// [`Auto`](ExecutionProvider::Auto) is **not** in it, although [`supports`](Self::supports) is `true` for it on
    /// every machine: `Auto` is a request — "pick for me" — rather than a capability, which is why this reads the
    /// providers a session can be built on rather than every published one.
    pub fn available(self) -> Vec<ExecutionProvider> {
        // Beside `supports` so a front end gets the list without writing a filter of its own — the same reasoning
        // `cpu` is a field for, one level up.
        ExecutionProvider::BUILT_ON.into_iter().filter(|provider| self.supports(*provider)).collect()
    }

    /// This report with WebGPU claimed or not, as the runtime's start found its plugin.
    ///
    /// Separate from [`with`](Self::with) because nothing is installed for it: the plugin arrives inside the runtime's
    /// own archive, and whether it works is only known once that runtime has loaded it.
    pub(crate) const fn with_webgpu(mut self, webgpu: bool) -> Self {
        self.webgpu = webgpu;
        self
    }
}

impl From<rust_sak::ai::Supported> for SupportedProviders {
    /// What `rust-sak` found this machine offers: the CPU always, CoreML on every macOS the runtime loads on (macOS 14
    /// and newer for 1.30.0, past the macOS 12 the ML Program format needs, so no version is probed), CUDA and
    /// TensorRT where their libraries were installed, and WebGPU where the runtime's plugin registered and offered a
    /// device.
    fn from(supported: rust_sak::ai::Supported) -> Self {
        let rust_sak::ai::Supported { cpu, coreml, cuda, tensorrt, webgpu } = supported;
        Self { cpu, coreml, cuda, tensorrt, webgpu }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A machine reporting exactly the providers named, whatever the machine running the test actually is.
    pub(crate) fn machine_supporting(coreml: bool, cuda: bool, tensorrt: bool) -> SupportedProviders {
        // The whole reason the resolution needs no platform seam: a report is a handful of booleans, so a Mac, an NVIDIA
        // workstation and a machine with no accelerator are three values a test writes down on any CI runner.
        SupportedProviders { cpu: true, coreml, cuda, tensorrt, webgpu: false }
    }

    /// A machine with no accelerator: the CPU and nothing else.
    pub(crate) fn no_accelerator() -> SupportedProviders {
        machine_supporting(false, false, false)
    }

    #[test]
    fn the_report_is_rust_saks_field_by_field() {
        let report = SupportedProviders::from(rust_sak::ai::Supported::detect(true, true));
        assert!(report.cpu, "a machine that cannot run on the CPU cannot run at all");
        assert_eq!(report.coreml, cfg!(target_os = "macos"));
        assert!(report.cuda && report.tensorrt, "as installed");
        assert!(!report.webgpu, "nothing is registered before the runtime loads");

        let tensorrt_alone = SupportedProviders::from(rust_sak::ai::Supported::detect(false, true));
        assert!(!tensorrt_alone.tensorrt, "TensorRT runs on CUDA");
    }

    /// What the report lands as on the other side of a process boundary.
    #[test]
    fn the_report_serializes_as_the_fields_a_front_end_reads() {
        // The four keys are what a front end's provider list is built from, and renaming one is a compile error
        // nowhere: it is a select that goes empty. Asserted key by key rather than against a whole JSON literal because
        // the struct is `#[non_exhaustive]` — a provider added later is a fifth key, and this should keep passing when
        // it arrives.
        let json = serde_json::to_value(machine_supporting(true, true, false)).expect("the report should serialize");

        assert_eq!(json["cpu"], true);
        assert_eq!(json["coreml"], true);
        assert_eq!(json["cuda"], true);
        assert_eq!(json["tensorrt"], false);
    }

    #[test]
    fn neither_gpu_provider_is_claimed_before_its_library_is_installed() {
        // The report is honest about what is on disk, not about what hardware was seen — which is what makes it
        // correct on a platform where an NVIDIA card is detected and nothing is published for it.
        let detected = SupportedProviders::from(rust_sak::ai::Supported::detect(false, false));

        assert!(!detected.cuda);
        assert!(!detected.tensorrt);
    }

    #[test]
    fn every_provider_parses_from_the_spelling_it_reports_whatever_the_casing() {
        // The three casings a settings file, a command line and a hand-edited config actually carry. Driven from
        // `ALL` rather than from a list written here, so a sixth provider left out of the parse fails this.
        for provider in ExecutionProvider::ALL {
            let spelling = provider.as_str();

            for text in [spelling.to_string(), spelling.to_lowercase(), spelling.to_uppercase()] {
                assert_eq!(
                    text.parse::<ExecutionProvider>().expect("a published spelling parses"),
                    provider,
                    "{text:?} did not parse as {provider}"
                );
            }
        }
    }

    #[test]
    fn text_naming_no_published_provider_is_refused_with_the_text_it_was_given() {
        // `openvino` is the pointed one: the reference implementation publishes it, so a settings file written by
        // that application names it, and the answer here is a refusal rather than a provider that never attaches.
        for text in ["openvino", "metal", "", "cuda11", "CoreML "] {
            let error = text.parse::<ExecutionProvider>().expect_err("unpublished text named a provider");

            assert!(
                matches!(&error, InitError::UnknownProvider { name } if name == text),
                "the error did not carry the text that was given: {error}"
            );
            assert!(error.to_string().contains(text), "the message did not name the text: {error}");
        }
    }

    #[test]
    fn each_accelerator_is_the_provider_of_its_own_name_and_the_request_and_the_cpu_are_none() {
        for provider in ExecutionProvider::ALL {
            match provider.accelerator() {
                Some(accelerator) => {
                    assert_eq!(ExecutionProvider::from(accelerator), provider);
                    assert_eq!(accelerator.to_string(), provider.as_str());
                }
                None => assert!(matches!(provider, ExecutionProvider::Auto | ExecutionProvider::Cpu), "{provider}"),
            }
        }
    }

    #[test]
    fn the_published_spellings_are_pinned_as_literals() {
        // Pinned before the request and the hardware were told apart in this crate, so that split could not move a
        // spelling a settings file has stored.
        assert_eq!(
            ExecutionProvider::ALL.map(ExecutionProvider::as_str),
            ["Auto", "CPU", "CoreML", "CUDA", "TensorRT", "WebGPU"]
        );
        assert_eq!(
            serde_json::to_string(&ExecutionProvider::ALL).expect("the providers serialize"),
            r#"["Auto","CPU","CoreML","CUDA","TensorRT","WebGPU"]"#
        );
    }

    #[test]
    fn a_provider_round_trips_through_its_serialized_form_as_the_spelling_a_user_sees() {
        // The serialized text is a compatibility surface the moment a front end persists a choice, so it is pinned
        // here rather than left to whatever the derive produces from the arm names — which would store `CoreMl`.
        for provider in ExecutionProvider::ALL {
            let json = serde_json::to_string(&provider).expect("a provider serializes");

            assert_eq!(json, format!("\"{}\"", provider.as_str()));
            assert_eq!(serde_json::from_str::<ExecutionProvider>(&json).expect("a provider deserializes"), provider);
        }
    }

    #[test]
    fn the_displayed_name_is_the_spelling_that_parses_back() {
        // What makes a provider printed in a log or an error usable as input: the round trip closes through the
        // three surfaces a user sees.
        for provider in ExecutionProvider::ALL {
            assert_eq!(provider.to_string(), provider.as_str());
            assert_eq!(provider.to_string().parse::<ExecutionProvider>().expect("a display name parses"), provider);
        }
    }

    #[test]
    fn a_provider_whose_library_was_never_installed_is_not_supported() {
        // `supports` reads what is on disk rather than what hardware was seen, which is what stops a request for CUDA
        // on a machine that never installed it from reaching a session build as a provider that declines natively.
        let bare = no_accelerator();

        assert!(!bare.supports(ExecutionProvider::Cuda));
        assert!(!bare.supports(ExecutionProvider::TensorRt));
        assert!(!bare.supports(ExecutionProvider::CoreMl));
    }

    #[test]
    fn auto_and_the_cpu_are_supported_on_every_machine() {
        // Checked against the barest report there is and against one claiming everything.
        for machine in [no_accelerator(), machine_supporting(true, true, true), machine_supporting(true, false, false)]
        {
            assert!(machine.supports(ExecutionProvider::Auto), "{machine:?} could not be asked for Auto");
            assert!(machine.supports(ExecutionProvider::Cpu), "{machine:?} could not be asked for the CPU");
        }
    }

    #[test]
    fn each_named_provider_answers_from_its_own_flag_and_no_other() {
        // One flag per provider, in the same direction the install plan set it: a report claiming CUDA alone must not
        // answer for TensorRT, which is the mistake that would attach a provider whose library is absent.
        let cuda_only = machine_supporting(false, true, false);
        assert!(cuda_only.supports(ExecutionProvider::Cuda));
        assert!(!cuda_only.supports(ExecutionProvider::TensorRt));
        assert!(!cuda_only.supports(ExecutionProvider::CoreMl));

        let tensorrt_only = machine_supporting(false, false, true);
        assert!(tensorrt_only.supports(ExecutionProvider::TensorRt));
        assert!(!tensorrt_only.supports(ExecutionProvider::Cuda));

        let mac = machine_supporting(true, false, false);
        assert!(mac.supports(ExecutionProvider::CoreMl));
        assert!(!mac.supports(ExecutionProvider::Cuda));
        assert!(!mac.supports(ExecutionProvider::TensorRt));
    }
}
