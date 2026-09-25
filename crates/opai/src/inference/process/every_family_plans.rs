//! The planner over every operation the library can name, family by family.

use super::plan;
use crate::models::face::tests::face_at;
use crate::models::test_support::every_operation;
use crate::models::{
    Bias, ColorBalance, Colorization, Denoise, FaceRecovery, Faces, Fidelity, FloatPrecision, LightAdjustment,
    Operation, Scale, Sharpen, Strength, Upscale,
};
use crate::pipeline::test_support::NoBackend;

#[test]
fn every_operation_the_carrier_can_hold_reaches_a_pipeline() {
    // The seam's exhaustiveness, over every operation the library can name: every published variant of every
    // enhancement family reaches its own family's contract seam, which is the only place a model is named at all.
    // Asked per **operation** rather than per family, because a family could land one variant before another.
    //
    // And it plans, which is the seam above this one and checks the whole chain before anything is installed. There
    // is no refusal here for an operation that produces no image, because such an operation cannot be carried at
    // all: see `Analysis`. `NoBackend`'s `acquire` is `unreachable!`, so nothing below installs anything.
    for operation in every_operation() {
        assert!(operation.pipeline::<NoBackend>().is_ok(), "{operation:?} reached no pipeline");
        assert!(
            plan::<NoBackend>(std::slice::from_ref(&operation)).is_ok(),
            "{operation:?} was refused by the planner"
        );
    }
}

#[test]
fn a_colorization_chained_with_another_family_plans_whole() {
    // The traversal above plans one operation at a time; this is the chain the colorization spec names. Every
    // variant, on both sides of another family's operation, so neither contract is refused by its position.
    let kyoto = Upscale::kyoto(FloatPrecision::Fp32, Scale::new(4.0).expect("in range"));

    for variant in crate::models::colorization::variant::tests::every_variant() {
        let colorization = Operation::Colorization(Colorization::new(variant));

        for chain in [[colorization.clone(), kyoto.clone()], [kyoto.clone(), colorization]] {
            let planned = plan::<NoBackend>(&chain).unwrap_or_else(|error| panic!("{chain:?} was refused: {error}"));

            assert_eq!(planned.len(), 2, "{chain:?} did not plan both operations");
        }
    }
}

#[test]
fn both_variants_of_the_face_recovery_family_reach_a_pipeline() {
    // Both are checked here rather than left to the traversal above, because the two reach the same contract
    // through different weight resolutions and a seam that had regressed to naming one model would still serve the
    // other.
    let faces = Faces::new([face_at(10.0, 20.0, 60.0, 80.0)]);
    let fidelity = Fidelity::MAXIMUM;

    for precision in [FloatPrecision::Fp32, FloatPrecision::Fp16] {
        for operation in [
            FaceRecovery::athens(precision, faces.clone(), fidelity),
            FaceRecovery::santorini(precision, faces.clone()),
        ] {
            assert!(operation.pipeline::<NoBackend>().is_ok(), "{} reached no pipeline", operation.display_name());

            // And it plans, which is the seam above this one. `NoBackend`'s `acquire` is `unreachable!`, so
            // nothing below opens a session or installs a file.
            assert!(
                plan::<NoBackend>(std::slice::from_ref(&operation)).is_ok(),
                "{} was refused by the planner",
                operation.display_name()
            );
        }
    }
}

#[test]
fn every_light_adjustment_variant_reaches_a_pipeline_and_plans() {
    // Both variants at both precisions. Checked at the planner as well as at the seam, because the planner checks
    // the **whole** chain before anything is installed — so a family that reached a pipeline but was still refused
    // a plan would look served here and fail on a real run.
    let bias = Bias::new(0.5).expect("the test supplied a bias in range");

    for precision in [FloatPrecision::Fp32, FloatPrecision::Fp16] {
        for operation in [LightAdjustment::paris(precision, bias), LightAdjustment::lyon(precision, bias)] {
            assert!(operation.pipeline::<NoBackend>().is_ok(), "{} reached no pipeline", operation.display_name());

            // `NoBackend`'s `acquire` is `unreachable!`, so nothing below opens a session or installs a file.
            assert!(
                plan::<NoBackend>(std::slice::from_ref(&operation)).is_ok(),
                "{} was refused by the planner",
                operation.display_name()
            );
        }
    }
}

#[test]
fn every_denoise_variant_reaches_a_pipeline_and_plans() {
    // All three variants at both precisions, checked at the planner as well as at the seam for the reason the
    // light adjustment test gives.
    let strength = Strength::new(1.5).expect("the test supplied a strength in range");

    for precision in [FloatPrecision::Fp32, FloatPrecision::Fp16] {
        for operation in [
            Denoise::stockholm(precision, strength),
            Denoise::gothenburg(precision, strength),
            Denoise::malmo(precision, strength),
        ] {
            assert!(operation.pipeline::<NoBackend>().is_ok(), "{} reached no pipeline", operation.display_name());

            // `NoBackend`'s `acquire` is `unreachable!`, so nothing below opens a session or installs a file.
            assert!(
                plan::<NoBackend>(std::slice::from_ref(&operation)).is_ok(),
                "{} was refused by the planner",
                operation.display_name()
            );
        }
    }
}

#[test]
fn every_sharpen_variant_reaches_a_pipeline_and_plans() {
    // All three variants at both precisions, checked at the planner as well as at the seam for the reason the
    // light adjustment test gives.
    let strength = Strength::new(1.5).expect("the test supplied a strength in range");

    for precision in [FloatPrecision::Fp32, FloatPrecision::Fp16] {
        for operation in [
            Sharpen::moscow(precision, strength),
            Sharpen::stpetersburg(precision, strength),
            Sharpen::novgorod(precision, strength),
        ] {
            assert!(operation.pipeline::<NoBackend>().is_ok(), "{} reached no pipeline", operation.display_name());

            // `NoBackend`'s `acquire` is `unreachable!`, so nothing below opens a session or installs a file.
            assert!(
                plan::<NoBackend>(std::slice::from_ref(&operation)).is_ok(),
                "{} was refused by the planner",
                operation.display_name()
            );
        }
    }
}

#[test]
fn both_colour_balance_variants_reach_a_pipeline_and_plan() {
    // Two contracts, both written, neither refused. Checked at the planner as well as at the seam, because the
    // planner checks the **whole** chain before anything is installed — so a variant that reached a pipeline but
    // was still refused a plan would look served here and fail on a real run.
    let bias = Bias::new(0.5).expect("the test supplied a bias in range");

    for precision in [FloatPrecision::Fp32, FloatPrecision::Fp16] {
        for operation in [ColorBalance::rio(precision, bias), ColorBalance::saopaulo(precision, bias)] {
            assert!(operation.pipeline::<NoBackend>().is_ok(), "{} reached no pipeline", operation.display_name());

            // `NoBackend`'s `acquire` is `unreachable!`, so nothing below opens a session or installs a file.
            assert!(
                plan::<NoBackend>(std::slice::from_ref(&operation)).is_ok(),
                "{} was refused by the planner",
                operation.display_name()
            );
        }
    }
}
