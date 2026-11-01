//! What a model is asked to run on, and the settings a session is built with.
//!
//! [`SupportedProviders`] is what the *machine* can offer — a report initialization folds out of the GPU install
//! plan and the runtime's load. [`ExecutionProvider`] is what a *user* asked for, which a front end offers in a settings
//! pane and hands back. Both are `rust-sak`'s, which shares this crate's spellings and serialized form, so a stored
//! choice and the front end's JSON mean the same in both. [`profile`] holds the helpers each model's measured
//! `rust_sak::ai::Tuning` is built from.

#[cfg(test)]
mod option_maps;
pub(crate) mod profile;

pub use rust_sak::ai::{ExecutionProvider, Supported as SupportedProviders};

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::error::InitError;

    /// A machine reporting exactly the providers named, whatever the machine running the test actually is.
    pub(crate) fn machine_supporting(coreml: bool, cuda: bool, tensorrt: bool) -> SupportedProviders {
        // The whole reason the resolution needs no platform seam: a report is a handful of booleans, so a Mac, an NVIDIA
        // workstation and a machine with no accelerator are three values a test writes down on any CI runner.
        SupportedProviders { cpu: true, coreml, cuda, tensorrt, webgpu: false }
    }

    /// `machine` with WebGPU offered as well.
    pub(crate) fn with_webgpu(machine: SupportedProviders) -> SupportedProviders {
        SupportedProviders { webgpu: true, ..machine }
    }

    /// A machine with no accelerator: the CPU and nothing else.
    pub(crate) fn no_accelerator() -> SupportedProviders {
        machine_supporting(false, false, false)
    }

    #[test]
    fn the_report_is_what_rust_sak_detects() {
        let report = SupportedProviders::detect(true, true);
        assert!(report.cpu, "a machine that cannot run on the CPU cannot run at all");
        assert_eq!(report.coreml, cfg!(target_os = "macos"));
        assert!(report.cuda && report.tensorrt, "as installed");
        assert!(!report.webgpu, "nothing is registered before the runtime loads");

        assert!(!SupportedProviders::detect(false, true).tensorrt, "TensorRT runs on CUDA");
    }

    /// What the report lands as on the other side of a process boundary.
    #[test]
    fn the_report_serializes_as_the_fields_a_front_end_reads() {
        // The keys are what a front end's provider list is built from, and renaming one is a compile error nowhere: it
        // is a select that goes empty. Asserted key by key rather than against a whole JSON literal, so a provider
        // added later is one more key and this keeps passing when it arrives.
        let json = serde_json::to_value(with_webgpu(machine_supporting(true, true, false)))
            .expect("the report should serialize");

        assert_eq!(json["cpu"], true);
        assert_eq!(json["coreml"], true);
        assert_eq!(json["cuda"], true);
        assert_eq!(json["tensorrt"], false);
        assert_eq!(json["webgpu"], true);
    }

    #[test]
    fn neither_gpu_provider_is_claimed_before_its_library_is_installed() {
        // The report is honest about what is on disk, not about what hardware was seen — which is what makes it
        // correct on a platform where an NVIDIA card is detected and nothing is published for it.
        let detected = SupportedProviders::detect(false, false);

        assert!(!detected.cuda);
        assert!(!detected.tensorrt);
    }

    #[test]
    fn every_provider_parses_from_the_spelling_it_reports_whatever_the_casing() {
        // The three casings a settings file, a command line and a hand-edited config actually carry. Driven from
        // `ALL` rather than from a list written here, so a seventh provider left out of the parse fails this.
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
            let error =
                InitError::from(text.parse::<ExecutionProvider>().expect_err("unpublished text named a provider"));

            assert!(
                matches!(&error, InitError::UnknownProvider { name } if name == text),
                "the error did not carry the text that was given: {error}"
            );
            assert!(error.to_string().contains(text), "the message did not name the text: {error}");
        }
    }

    #[test]
    fn the_published_spellings_are_pinned_as_literals() {
        // Pinned as literals, so the move to `rust-sak`'s type could not move a spelling a settings file has stored.
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
        for provider in ExecutionProvider::ALL {
            assert_eq!(provider.to_string(), provider.as_str());
            assert_eq!(provider.to_string().parse::<ExecutionProvider>().expect("a display name parses"), provider);
        }
    }

    #[test]
    fn a_provider_whose_library_was_never_installed_is_not_supported() {
        let bare = no_accelerator();

        assert!(!bare.supports(ExecutionProvider::Cuda));
        assert!(!bare.supports(ExecutionProvider::TensorRt));
        assert!(!bare.supports(ExecutionProvider::CoreMl));
        assert!(!bare.supports(ExecutionProvider::WebGpu));
    }

    #[test]
    fn auto_and_the_cpu_are_supported_on_every_machine() {
        for machine in [no_accelerator(), machine_supporting(true, true, true), machine_supporting(true, false, false)]
        {
            assert!(machine.supports(ExecutionProvider::Auto), "{machine:?} could not be asked for Auto");
            assert!(machine.supports(ExecutionProvider::Cpu), "{machine:?} could not be asked for the CPU");
        }
    }

    #[test]
    fn the_available_list_is_in_published_order_without_auto() {
        assert_eq!(
            with_webgpu(machine_supporting(true, false, false)).available(),
            [ExecutionProvider::Cpu, ExecutionProvider::CoreMl, ExecutionProvider::WebGpu]
        );
        assert_eq!(
            machine_supporting(false, true, true).available(),
            [ExecutionProvider::Cpu, ExecutionProvider::Cuda, ExecutionProvider::TensorRt]
        );
    }
}
