//! What an operation resolves to: the passes a convolutional variant runs, or the graphs Osaka loads.

use super::super::artifact::ArtifactId;
use super::Upscale;
use super::osaka::graph::{GraphRole, GraphSet, UnknownRole};

/// One native scaling pass, and the artifact that serves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pass {
    /// The native scale of the weights this pass runs, which is a whole number because the weights are published per
    /// whole factor. Emphatically not the scale that was requested.
    pub scale: u8,
    /// The artifact serving this pass.
    pub artifact: ArtifactId,
}

// An enum rather than two methods returning `Option`, so that the two contracts stay visibly separate and a caller
// running an operation has to say what it does with each rather than defaulting one of them to the other's shape.
/// What one upscale operation resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// A convolutional variant: an ordered sequence of native passes covering the requested scale. Where the sequence
    /// overshoots the request — a 1.5x request served by a 2x pass — the overshoot is corrected after inference and
    /// is not a property of this resolution.
    Passes(Vec<Pass>),
    /// A diffusion variant: graphs loaded together as stages of one pass.
    Graphs(GraphSet),
}

impl Upscale {
    /// The passes or graphs this operation runs. Never empty on either branch.
    pub fn resolve(self) -> Resolution {
        self.variant().model().resolve(self.precision(), self.scale())
    }

    /// The distinct artifacts that must be on disk before this operation can run, in the order they are first needed.
    ///
    /// Deduplicated, because a sequence that runs one artifact twice still requires it once — Tokyo at 8x is two
    /// passes of one set of weights, not two downloads.
    pub fn required_artifacts(self) -> Vec<ArtifactId> {
        let mut required: Vec<ArtifactId> = Vec::new();

        // Pushed straight into the answer rather than collected and then filtered: the duplicates are the point of
        // the scan, and a second vector holding them on the way through buys nothing. `contains` rather than a sort
        // or a set because the order is the order they are first needed, and the list is three entries at most.
        match self.resolve() {
            Resolution::Passes(passes) => {
                for pass in passes {
                    if !required.contains(&pass.artifact) {
                        required.push(pass.artifact);
                    }
                }
            }
            Resolution::Graphs(graphs) => {
                for artifact in graphs.artifacts() {
                    if !required.contains(artifact) {
                        required.push(artifact.clone());
                    }
                }
            }
        }

        required
    }

    /// The graph filling `role`, for an operation that resolves to graphs.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownRole`] when this operation's variant declares no graph for `role` — which includes every
    /// convolutional variant, none of which declares any.
    pub fn graph(self, role: GraphRole) -> Result<ArtifactId, UnknownRole> {
        // Answered off the resolution rather than through a second accessor on the row. Composing the other two
        // artifact names to hand back one is a few string builds against a call a front end makes once, and the
        // alternative costs every convolutional model a `graph` method whose whole body is a hardcoded refusal.
        match self.resolve() {
            Resolution::Graphs(set) => set.graph(role).cloned(),
            Resolution::Passes(_) => Err(UnknownRole { role, variant: self.variant().model().codename() }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::UpscaleVariant;
    use super::super::variant::tests::every_variant;
    use super::*;
    use crate::models::precision::FloatPrecision;
    use crate::models::scale::Scale;
    use crate::models::upscale::osaka::precision::OsakaPrecision;

    fn operation(variant: UpscaleVariant, value: f64) -> Upscale {
        Upscale::new(variant, Scale::new(value).expect("the test supplied a scale in range"))
    }

    fn passes(operation: Upscale) -> Vec<Pass> {
        match operation.resolve() {
            Resolution::Passes(passes) => passes,
            Resolution::Graphs(_) => panic!("{operation} resolved to graphs rather than to passes"),
        }
    }

    fn names(artifacts: &[ArtifactId]) -> Vec<&str> {
        artifacts.iter().map(ArtifactId::as_str).collect()
    }

    #[test]
    fn kyoto_covers_a_small_scale_with_its_two_times_weights() {
        let resolved = passes(operation(UpscaleVariant::Kyoto(FloatPrecision::Fp32), 1.5));

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].scale, 2);
        assert_eq!(resolved[0].artifact.as_str(), "up_kyoto_2x_fp32");
    }

    #[test]
    fn kyoto_covers_a_large_scale_with_two_passes_of_different_weights() {
        let operation = operation(UpscaleVariant::Kyoto(FloatPrecision::Fp32), 8.0);
        let resolved = passes(operation);

        assert_eq!(resolved.iter().map(|pass| pass.scale).collect::<Vec<_>>(), vec![4, 2]);
        assert_eq!(
            resolved.iter().map(|pass| pass.artifact.as_str()).collect::<Vec<_>>(),
            vec!["up_kyoto_4x_fp32", "up_kyoto_2x_fp32"]
        );
        assert_eq!(names(&operation.required_artifacts()), vec!["up_kyoto_4x_fp32", "up_kyoto_2x_fp32"]);
    }

    #[test]
    fn tokyo_covers_a_large_scale_by_repeating_one_set_of_weights() {
        let operation = operation(UpscaleVariant::Tokyo(FloatPrecision::Fp16), 8.0);
        let resolved = passes(operation);

        assert_eq!(resolved.iter().map(|pass| pass.scale).collect::<Vec<_>>(), vec![4, 4]);
        assert!(resolved.iter().all(|pass| pass.artifact.as_str() == "up_tokyo_4x_fp16"));
        assert_eq!(
            names(&operation.required_artifacts()),
            vec!["up_tokyo_4x_fp16"],
            "a sequence running one artifact twice required it twice"
        );
    }

    #[test]
    fn a_requested_scale_never_appears_in_an_artifact_name() {
        // The two differ whenever a request falls between native scales, and naming an artifact from the request
        // would name a file that was never published.
        let operation = operation(UpscaleVariant::Kyoto(FloatPrecision::Fp32), 2.5);

        assert_eq!(names(&operation.required_artifacts()), vec!["up_kyoto_4x_fp32"]);
    }

    #[test]
    fn every_scale_in_range_resolves_to_at_least_one_pass() {
        // Zero passes downstream is a plain resize presented as a successful AI upscale — no error and no log line.
        for variant in every_variant().into_iter().filter(|variant| !variant.is_diffusion()) {
            let mut value = Scale::MIN;
            while value <= Scale::MAX {
                let operation = operation(variant, value);
                assert!(!passes(operation).is_empty(), "{operation} resolved to no passes");
                assert!(!operation.required_artifacts().is_empty(), "{operation} required no artifacts");
                value += 0.001;
            }
        }
    }

    #[test]
    fn every_pass_is_served_by_an_artifact_named_from_its_own_native_scale() {
        for variant in every_variant().into_iter().filter(|variant| !variant.is_diffusion()) {
            for value in [1.0, 1.5, 2.0, 2.5, 4.0, 6.0, 8.0] {
                let operation = operation(variant, value);

                for pass in passes(operation) {
                    let expected = format!("up_{}_{}x_{}", variant.codename(), pass.scale, variant.precision());
                    assert_eq!(pass.artifact.as_str(), expected, "{operation} named a pass from something else");
                }
            }
        }
    }

    #[test]
    fn a_convolutional_operation_declares_no_graphs() {
        let operation = operation(UpscaleVariant::Saitama(FloatPrecision::Fp32), 4.0);
        let error = operation.graph(GraphRole::Encoder).expect_err("a convolutional variant resolved to a graph");

        assert_eq!(error.role, GraphRole::Encoder);
        assert_eq!(error.variant, "saitama");
    }

    #[test]
    fn an_osaka_operation_resolves_to_graphs_rather_than_passes() {
        let operation = operation(UpscaleVariant::Osaka(OsakaPrecision::Fp16), 4.0);

        assert!(matches!(operation.resolve(), Resolution::Graphs(_)));
    }

    fn graphs(operation: Upscale) -> GraphSet {
        match operation.resolve() {
            Resolution::Graphs(graphs) => graphs,
            Resolution::Passes(_) => panic!("{operation} resolved to passes rather than to graphs"),
        }
    }

    #[test]
    fn an_fp16_osaka_operation_requires_three_fp16_artifacts() {
        let operation = operation(UpscaleVariant::Osaka(OsakaPrecision::Fp16), 2.0);

        assert_eq!(
            names(&operation.required_artifacts()),
            vec!["up_osaka_vae_encoder_fp16", "up_osaka_fp16", "up_osaka_vae_decoder_fp16"]
        );
    }

    #[test]
    fn an_int8_osaka_operation_quantizes_only_the_transformer() {
        // The two VAE halves are published only as FP16 — one pair of files shared by both builds — so a half that
        // followed the operation would name `up_osaka_vae_encoder_int8`, which does not exist.
        let operation = operation(UpscaleVariant::Osaka(OsakaPrecision::Int8), 2.0);

        assert_eq!(
            names(&operation.required_artifacts()),
            vec!["up_osaka_vae_encoder_fp16", "up_osaka_int8", "up_osaka_vae_decoder_fp16"]
        );
    }

    #[test]
    fn every_osaka_graph_is_reached_by_its_role_rather_than_by_position() {
        for precision in [OsakaPrecision::Fp16, OsakaPrecision::Int8] {
            let operation = operation(UpscaleVariant::Osaka(precision), 4.0);
            let resolved = graphs(operation);

            assert_eq!(
                resolved.roles().collect::<Vec<_>>(),
                vec![GraphRole::Encoder, GraphRole::Transformer, GraphRole::Decoder]
            );
            assert_eq!(
                operation.graph(GraphRole::Encoder).expect("Osaka declares an encoder").as_str(),
                "up_osaka_vae_encoder_fp16"
            );
            assert_eq!(
                operation.graph(GraphRole::Decoder).expect("Osaka declares a decoder").as_str(),
                "up_osaka_vae_decoder_fp16"
            );
            assert_eq!(
                operation.graph(GraphRole::Transformer).expect("Osaka declares a transformer").as_str(),
                &format!("up_osaka_{}", operation.precision())
            );
        }
    }

    #[test]
    fn each_osaka_graph_carries_the_settings_measured_for_it_rather_than_the_variants_alone() {
        // The per-graph contract, checked from **one** operation rather than by resolving three times: what would go
        // wrong without it is that a graph opens under another graph's configuration, and that is only visible when
        // the three are looked at side by side.
        for precision in [OsakaPrecision::Fp16, OsakaPrecision::Int8] {
            let operation = operation(UpscaleVariant::Osaka(precision), 4.0);
            let resolved = graphs(operation);
            let of = |role| resolved.resolved(role).expect("Osaka declares this role").profile.clone();

            // The two convolutional halves take the variant's layout: cuDNN has FP16 tensor-core kernels for NHWC at
            // these shapes, worth -8.0% and -9.4%.
            assert!(of(GraphRole::Encoder).cuda_prefer_nhwc, "the encoder lost the layout measured for it");
            assert!(of(GraphRole::Decoder).cuda_prefer_nhwc, "the decoder lost the layout measured for it");
            // And the transformer does not: no `Conv` in 12,940 nodes, so all NHWC buys there is its own transform.
            assert!(
                !of(GraphRole::Transformer).cuda_prefer_nhwc,
                "the transformer was given the VAE halves' layout, which costs it 1.3%"
            );

            // The override names one setting and leaves the rest of the variant's declaration alone — otherwise a
            // graph that deviates on one figure would quietly lose the four it does not.
            let variant = UpscaleVariant::Osaka(precision).profile();
            for role in [GraphRole::Encoder, GraphRole::Transformer, GraphRole::Decoder] {
                let profile = of(role);

                assert_eq!(profile.execution_mode, variant.execution_mode, "{role} at {precision:?}");
                assert_eq!(profile.disable_mem_pattern, variant.disable_mem_pattern, "{role} at {precision:?}");
                assert_eq!(
                    profile.disabled_optimizers, variant.disabled_optimizers,
                    "{role} at {precision:?}: without the second of these the transformer does not load at all"
                );
                assert_eq!(profile.coreml_compute_units, variant.coreml_compute_units, "{role} at {precision:?}");
                assert_eq!(profile.trt_options, variant.trt_options, "{role} at {precision:?}");
            }

            // The two halves agree with each other and neither agrees with the transformer, which is the whole claim
            // stated as one comparison rather than inferred from the three above.
            assert_eq!(of(GraphRole::Encoder), of(GraphRole::Decoder));
            assert_ne!(of(GraphRole::Encoder), of(GraphRole::Transformer));
        }
    }

    #[test]
    fn a_graph_naming_no_deviation_carries_the_variants_declaration_unchanged() {
        // The other half of the requirement, and the one a later variant depends on: a graph that names nothing is
        // one nobody measured separately, so it must follow the variant wherever the variant goes.
        let operation = operation(UpscaleVariant::Osaka(OsakaPrecision::Fp16), 4.0);
        let resolved = graphs(operation);

        assert_eq!(
            resolved.resolved(GraphRole::Encoder).expect("Osaka declares an encoder").profile,
            UpscaleVariant::Osaka(OsakaPrecision::Fp16).profile(),
            "the graph that declares no deviation did not carry the variant's own settings"
        );
    }

    #[test]
    fn all_three_osaka_graphs_are_required_together() {
        // They are stages of one pass, so there is no partial set that runs.
        for precision in [OsakaPrecision::Fp16, OsakaPrecision::Int8] {
            let operation = operation(UpscaleVariant::Osaka(precision), 4.0);

            assert_eq!(
                operation.required_artifacts().len(),
                3,
                "{operation} required something other than its three graphs"
            );
            assert_eq!(graphs(operation).artifacts().count(), 3);
        }
    }

    #[test]
    fn osakas_scale_changes_nothing_about_what_it_requires() {
        // One set of weights serves every scale; the scale travels per run instead.
        let two_times = operation(UpscaleVariant::Osaka(OsakaPrecision::Fp16), 2.0);
        let six_times = operation(UpscaleVariant::Osaka(OsakaPrecision::Fp16), 6.0);

        assert_eq!(two_times.required_artifacts(), six_times.required_artifacts());
    }

    #[test]
    fn every_artifact_id_is_a_stem_carrying_no_file_extension() {
        // One artifact is often several files — a large model keeps its weights in a sibling `.onnx.data` — and which
        // ones do is discovered from the remote listing rather than predicted here.
        for variant in every_variant() {
            for artifact in operation(variant, 8.0).required_artifacts() {
                assert!(!artifact.as_str().contains('.'), "{artifact} carried a file extension");
                assert!(artifact.as_str().starts_with("up_"), "{artifact} is not named for the upscale family");
            }
        }
    }
}
