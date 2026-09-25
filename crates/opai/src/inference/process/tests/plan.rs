//! What a chain plans to run, and what it refuses before anything is installed.

use super::*;

#[test]
fn kyoto_is_planned_with_the_passes_covering_the_requested_scale() {
    let planned = plan::<Fake>(&[kyoto(8.0)]).unwrap();

    assert_eq!(planned.len(), 1);
    // Kyoto publishes native 2x and 4x weights, so 8x is the 4x pass and then the 2x one — visible here as the
    // two sets of weights the run would open, in the order the passes run them.
    assert_eq!(session_artifacts(&planned[0]), vec!["up_kyoto_4x_fp16", "up_kyoto_2x_fp16"]);
    assert_eq!(planned[0].pipeline.stages(), 2, "an 8x Kyoto run is two passes");
    assert_eq!(planned[0].operation(), &kyoto(8.0), "the plan lost the operation it came from");
}

#[tokio::test]
async fn the_requested_scale_survives_the_plan_rather_than_being_left_at_what_the_passes_reach() {
    // What the deleted `Planned::Passes { requested, .. }` field was checked for, through the only thing that can
    // still see it: a 1.5x request is served by the 2x weights and corrected back afterwards, so a plan that lost
    // the request would hand back a doubled image rather than a 1.5x one.
    let backend = Fake::new();

    let produced = run_with(&backend, source(100, 80), &[kyoto(1.5)]).await.expect("a 1.5x Kyoto run");

    assert_eq!(backend.log().ran, vec!["up_kyoto_2x_fp16"], "1.5x was not served by the 2x weights");
    assert_eq!((produced.width(), produced.height()), (150, 120), "the correcting resample did not happen");
}

#[test]
fn the_diffusion_upscaler_plans_through_the_seam_that_used_to_refuse_it() {
    // `plan_one` once matched on the contract itself, refusing one arm of it and handing the other to a second
    // planner. It now asks the operation for its pipeline and knows neither. What is checked is that the answer
    // is still the diffusion one — three graphs opened together as stages of one pass, rather than a pass
    // sequence — seen through the only surface left: what the run would open.
    for precision in [OsakaPrecision::Fp16, OsakaPrecision::Int8] {
        let osaka = upscale(UpscaleVariant::Osaka(precision));

        let planned = plan::<Fake>(std::slice::from_ref(&osaka))
            .unwrap_or_else(|error| panic!("Osaka is still refused at {precision:?}: {error}"));

        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].operation(), &osaka, "at {precision:?}");
        assert_eq!(session_artifacts(&planned[0]).len(), 3, "at {precision:?}");
        assert_eq!(
            planned[0].pipeline.stages(),
            3,
            "a graph set reported something other than its three stages at {precision:?}"
        );
        // The weights a convolutional operation would have named are absent rather than empty: every session this
        // opens is one of Osaka's own graphs. Not all three at the operation's precision — a VAE half resolves at
        // the precision *it* is published in, which is what `ResolvedGraph` carries the artifact for.
        for artifact in session_artifacts(&planned[0]) {
            assert!(artifact.starts_with("up_osaka"), "{artifact} is not one of Osaka's graphs at {precision:?}");
            assert!(!artifact.contains("x_"), "{artifact} names a native scale, which no graph has");
        }
    }
}

#[tokio::test]
async fn an_osaka_run_acquires_exactly_three_sessions_under_the_settings_measured_for_each_graph() {
    // The acquisition loop's contract seen through a whole run rather than off `Planned::sessions`: three
    // artifacts in the variant's own order, each opened under the profile measured for **it**. A loop that paired
    // by position against one shared profile would be visible here rather than only on a GPU.
    let backend = Fake::new();
    let osaka = upscale(UpscaleVariant::Osaka(OsakaPrecision::Fp16));

    run_with(&backend, source(64, 64), std::slice::from_ref(&osaka)).await.expect("an Osaka run");

    let opened = backend.log().opened_under.clone();
    assert_eq!(
        opened.iter().map(|(artifact, _)| artifact.as_str()).collect::<Vec<_>>(),
        vec!["up_osaka_vae_encoder_fp16", "up_osaka_fp16", "up_osaka_vae_decoder_fp16"],
        "an Osaka operation did not acquire its three graphs in the variant's order"
    );

    // The transformer is the one whose declaration differs, and it is the middle entry.
    assert!(opened[0].1.cuda_prefer_nhwc, "the encoder was opened under the transformer's layout");
    assert!(!opened[1].1.cuda_prefer_nhwc, "the transformer was opened under the VAE halves' layout");
    assert!(opened[2].1.cuda_prefer_nhwc, "the decoder was opened under the transformer's layout");

    for (artifact, profile) in &opened {
        assert_ne!(profile, &EpProfile::default(), "{artifact} was opened on the default profile by omission");
        assert_eq!(
            profile.disabled_optimizers,
            ["ReshapeFusion", "SimplifiedLayerNormFusion"],
            "{artifact} would not open on the CPU without the second of these"
        );
    }

    // All three ran, and each exactly once: a region is one run of each rather than a sequence over them.
    assert_eq!(backend.log().ran.len(), 3, "one region is three graph runs");
}

#[tokio::test]
async fn a_chain_mixing_osaka_with_a_convolutional_upscale_runs_both() {
    // The two contracts in one chain, through the one run loop. Osaka first so that the convolutional pass runs
    // over what the diffusion pipeline produced rather than over the source, which is what a chain *is*.
    let backend = Fake::new();
    let osaka = Operation::Upscale(Upscale::new(UpscaleVariant::Osaka(OsakaPrecision::Fp16), Scale::new(1.0).unwrap()));
    let chain = [osaka, kyoto(2.0)];

    let produced = run_with(&backend, source(64, 64), &chain).await.expect("a mixed chain");

    assert_eq!(
        backend.log().ran,
        vec![
            "up_osaka_vae_encoder_fp16",
            "up_osaka_fp16",
            "up_osaka_vae_decoder_fp16",
            // One tile: 64x64 is smaller than the convolutional geometry's 256.
            "up_kyoto_2x_fp16",
        ],
        "the two contracts did not both run, in order"
    );

    // Osaka at 1x restores in place and Kyoto doubles what it was handed.
    assert_eq!((produced.width(), produced.height()), (128, 128));
}

#[test]
fn a_chain_mixing_the_two_contracts_plans_each_one_as_its_own() {
    // The two live side by side within one family, so a chain naming both is planned rather than refused — and
    // each arrives as the shape its own contract has.
    let chain = [upscale(UpscaleVariant::Osaka(OsakaPrecision::Fp16)), kyoto(8.0)];
    let planned = plan::<Fake>(&chain).expect("a chain mixing the two contracts was refused");

    assert_eq!(
        session_artifacts(&planned[0]),
        vec!["up_osaka_vae_encoder_fp16", "up_osaka_fp16", "up_osaka_vae_decoder_fp16"],
        "Osaka did not plan as a graph set"
    );
    assert_eq!(
        session_artifacts(&planned[1]),
        vec!["up_kyoto_4x_fp16", "up_kyoto_2x_fp16"],
        "Kyoto did not plan as its own pass sequence"
    );
}

#[test]
fn osakas_three_graphs_each_plan_with_the_settings_measured_for_that_graph() {
    // The risk the trait was written around, checked at the seam it crosses: a plan's `sessions()` must hand back
    // each graph's **own** profile, still attached to the artifact it was measured for. The project's one
    // per-graph override is here — the transformer declines the CUDA layout the two VAE halves take — so a trait
    // that had split the artifact from its profile, or paired them by position, would be visible here rather than
    // only on a GPU.
    for precision in [OsakaPrecision::Fp16, OsakaPrecision::Int8] {
        let osaka = upscale(UpscaleVariant::Osaka(precision));

        let planned = plan::<Fake>(std::slice::from_ref(&osaka)).unwrap_or_else(|error| panic!("Osaka: {error}"));

        assert_eq!(planned[0].operation(), &osaka, "the plan lost the operation it came from");

        let artifacts = session_artifacts(&planned[0]);
        let profiles = session_profiles(&planned[0]);

        // In the variant's own declared order — encoder, transformer, decoder — which is also the order the
        // acquisition loop takes them in, so a position here is a role there.
        assert_eq!(artifacts.len(), 3, "at {precision:?}: all three are stages of one pass");
        assert_eq!(
            planned[0].pipeline.required().len(),
            3,
            "at {precision:?}: all three must be on disk before any of it runs"
        );

        assert!(profiles[0].cuda_prefer_nhwc, "{} carried the transformer's layout", artifacts[0]);
        assert!(!profiles[1].cuda_prefer_nhwc, "{} carried the VAE halves' layout", artifacts[1]);
        assert!(profiles[2].cuda_prefer_nhwc, "{} carried the transformer's layout", artifacts[2]);

        // And nothing arrives on the default by omission — every graph carries the measured declaration.
        for (artifact, profile) in artifacts.iter().zip(&profiles) {
            assert_ne!(profile, &EpProfile::default(), "{artifact} planned on the default profile");
            assert_eq!(profile.execution_mode, ExecutionMode::Sequential, "{artifact}");
            assert_eq!(
                profile.disabled_optimizers,
                ["ReshapeFusion", "SimplifiedLayerNormFusion"],
                "{artifact} would not open on the CPU without the second of these"
            );
        }
    }
}

#[test]
fn a_planned_graph_set_acquires_one_session_per_graph_under_that_graphs_settings() {
    // The acquisition loop's own contract, which is what `Planned::sessions` exists to make unmistakable: three
    // artifacts in the variant's order, each paired with the profile measured for it rather than with one shared
    // profile the loop looked up beside them.
    let osaka = upscale(UpscaleVariant::Osaka(OsakaPrecision::Fp16));
    let planned = plan::<Fake>(std::slice::from_ref(&osaka)).expect("Osaka plans");

    assert_eq!(
        session_artifacts(&planned[0]),
        vec!["up_osaka_vae_encoder_fp16", "up_osaka_fp16", "up_osaka_vae_decoder_fp16"]
    );
    // The transformer is the one that differs, and it is the middle entry — so a loop that paired by position
    // against a shared profile would be visible here rather than only on a GPU.
    let profiles = session_profiles(&planned[0]);
    assert!(profiles[0].cuda_prefer_nhwc, "the encoder would open under the transformer's layout");
    assert!(!profiles[1].cuda_prefer_nhwc, "the transformer would open under the VAE halves' layout");
    assert!(profiles[2].cuda_prefer_nhwc, "the decoder would open under the transformer's layout");

    // The convolutional contract against the same method, because the point of it is that one loop serves both:
    // a repeated pass is two sessions of one artifact, and both take the operation's single profile.
    let tokyo = plan::<Fake>(&[upscale(UpscaleVariant::Tokyo(FloatPrecision::Fp16))]).unwrap();

    assert_eq!(session_artifacts(&tokyo[0]).len(), 1, "a 4x Tokyo run is one native pass");
    assert_eq!(session_profiles(&tokyo[0])[0], upscale(UpscaleVariant::Tokyo(FloatPrecision::Fp16)).profile());
}

#[test]
fn tokyo_and_saitama_plan_at_both_precisions_carrying_the_profile_their_arm_declares() {
    // The seam a refusal used to occupy, kept rather than deleted: these four were gated on their profiles being
    // unmeasured, so what replaces the gate is the assertion that each one now plans *and* arrives with the
    // measured settings attached. Nothing may reach the pipeline on `EpProfile::default()` by omission.
    for variant in [
        UpscaleVariant::Tokyo(FloatPrecision::Fp16),
        UpscaleVariant::Tokyo(FloatPrecision::Fp32),
        UpscaleVariant::Saitama(FloatPrecision::Fp16),
        UpscaleVariant::Saitama(FloatPrecision::Fp32),
    ] {
        let operation = upscale(variant);

        let planned = plan::<Fake>(std::slice::from_ref(&operation))
            .unwrap_or_else(|error| panic!("{variant:?} was refused: {error}"));

        assert_eq!(planned.len(), 1);
        // Both publish native 4x weights only, so a 4x request is the single native pass.
        assert_eq!(planned[0].pipeline.stages(), 1, "{variant:?}");
        assert_eq!(
            session_profiles(&planned[0]),
            vec![operation.profile()],
            "{variant:?} was planned with a different profile"
        );
    }

    // And the values themselves, so that "carries its arm's profile" cannot pass by both sides being the default.
    // Tokyo excludes the Neural Engine at both precisions because its window attention is what the ANE compiler
    // cannot compile; Saitama names it at FP16 alone, because an FP32 MLProgram cannot reach it anyway.
    let tokyo = plan::<Fake>(&[upscale(UpscaleVariant::Tokyo(FloatPrecision::Fp32))]).unwrap();
    assert_eq!(session_profiles(&tokyo[0])[0].coreml_compute_units, CoreMlComputeUnits::CpuAndGpu);
    assert_eq!(session_profiles(&tokyo[0])[0].execution_mode, ExecutionMode::Sequential);

    let saitama = plan::<Fake>(&[upscale(UpscaleVariant::Saitama(FloatPrecision::Fp16))]).unwrap();
    assert_eq!(session_profiles(&saitama[0])[0].coreml_compute_units, CoreMlComputeUnits::CpuAndNeuralEngine);

    let saitama_fp32 = plan::<Fake>(&[upscale(UpscaleVariant::Saitama(FloatPrecision::Fp32))]).unwrap();
    assert_eq!(session_profiles(&saitama_fp32[0])[0], EpProfile::default());
}

#[test]
fn every_scale_a_user_can_ask_for_is_covered_by_a_pass_sequence() {
    // The `NoPasses` refusal's own reachability, stated as what it currently is: unreachable. A bucket table that
    // grew a gap would fail here rather than downstream, where the failure is a plain resample presented as an
    // enhancement.
    for steps in 0..=70_u16 {
        let scale = Scale::MIN + f64::from(steps) / 10.0;
        if scale > Scale::MAX {
            break;
        }

        let planned = plan::<Fake>(&[kyoto(scale)]).unwrap();
        assert!(planned[0].pipeline.stages() > 0, "no pass sequence covers {scale}x");
    }
}

#[test]
fn an_empty_chain_plans_nothing_rather_than_being_refused() {
    assert!(plan::<Fake>(&[]).unwrap().is_empty());
}

#[test]
fn a_chain_is_put_into_the_apply_order_before_it_is_planned() {
    // Out of order: the upscale would move the faces a recovery after it restores. The run order is the family's,
    // not the caller's, so both listings ask for one result.
    let recovery = crate::models::FaceRecovery::santorini(FloatPrecision::Fp32, crate::models::Faces::empty());
    let denoise = crate::models::Denoise::stockholm(FloatPrecision::Fp32, crate::models::Strength::clamped(1.0));

    let given = [kyoto(2.0), recovery.clone(), denoise.clone()];
    assert_eq!(&*in_apply_order(&given), [denoise.clone(), recovery.clone(), kyoto(2.0)]);

    // A chain already in order is borrowed rather than copied, which is what keeps its cache keys the ones it has
    // always had.
    let ordered = [denoise, recovery, kyoto(2.0)];
    assert!(matches!(in_apply_order(&ordered), std::borrow::Cow::Borrowed(_)));

    // Operations of one family keep the order they were given in.
    let two = [kyoto(4.0), kyoto(2.0)];
    assert_eq!(&*in_apply_order(&two), two);
}
