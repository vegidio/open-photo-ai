//! The operation on the wire: what the window names, and what it resolves to.
//!
//! An operation is named rather than spelled — see the module documentation in `mod.rs` for why, and
//! design.md D1.

use opai::{
    Bias, BuildError, Face, Faces, Family, Fidelity, ParameterValues, Precision, Scale, Strength, Subject, VariantEntry,
};
use serde::{Deserialize, Serialize};

/// One operation, as the window names it.
///
/// Tagged by family, spelled as [`Family`] spells itself, carrying the codename and precision every family
/// names plus that family's own parameters.
///
/// **Not [`opai::Operation`] itself** — that type's `Deserialize` reads its internal enum shape, not the
/// catalogue's vocabulary. This shape mirrors what [`VariantEntry`] publishes (`codename` + `precision`),
/// so a chooser built from the catalogue sends back exactly what it was given.
///
/// Seven arms, one per enhancement the window presents. Detection has none: the window asks for faces through
/// [`crate::faces::detect_faces`], never as an operation in a chain.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub(crate) enum Requested {
    /// An upscale run: which model, at which precision, enlarging by how much.
    Upscale {
        /// The model's developer-facing identifier, as [`opai::VariantEntry::codename`] publishes it.
        codename: String,
        /// The precision to run at, as [`opai::VariantEntry::precisions`] publishes them.
        precision: Precision,
        /// The factor to enlarge by. Clamped into range rather than refused — see [`Requested::resolve`].
        scale: f64,
    },

    /// A denoise run: which model, at which precision, applied how strongly.
    Denoise {
        /// The model's developer-facing identifier, as [`opai::VariantEntry::codename`] publishes it.
        codename: String,
        /// The precision to run at, as [`opai::VariantEntry::precisions`] publishes them.
        precision: Precision,
        /// The strength as the library's unit value, 0..3 where 1 is the model's own output — not the percentage the
        /// window shows. Clamped into range rather than refused — see [`Requested::resolve`].
        strength: f64,
    },

    /// A face-recovery run: which model, at which precision, over which faces.
    ///
    /// **No fidelity.** The window fixes it at [`Fidelity::MAXIMUM`] for every run and offers no control for it,
    /// so a field here would describe a choice the user cannot make — see [`Requested::resolve`] and design.md D7.
    FaceRecovery {
        /// The model's developer-facing identifier, as [`opai::VariantEntry::codename`] publishes it.
        codename: String,
        /// The precision to run at, as [`opai::VariantEntry::precisions`] publishes them.
        precision: Precision,
        /// The faces to restore, as [`crate::faces::detect_faces`] found them in the framed photograph.
        ///
        /// Order-sensitive: [`Faces`] keeps the order it is given, and that order is folded into the run cache
        /// tag — so the set the window hands back is the set the result is stored under.
        faces: Vec<Face>,
    },

    /// A light-adjustment run: which model, at which precision, shifted which way and how far.
    LightAdjustment {
        /// The model's developer-facing identifier, as [`opai::VariantEntry::codename`] publishes it.
        codename: String,
        /// The precision to run at, as [`opai::VariantEntry::precisions`] publishes them.
        precision: Precision,
        /// The bias as the library's unit value, -1..1 about a neutral 0 — not the percentage the window shows.
        /// Clamped into range rather than refused — see [`Requested::resolve`].
        bias: f64,
    },

    /// A colour-balance run: which model, at which precision, shifted which way and how far.
    ColorBalance {
        /// The model's developer-facing identifier, as [`opai::VariantEntry::codename`] publishes it.
        codename: String,
        /// The precision to run at, as [`opai::VariantEntry::precisions`] publishes them.
        precision: Precision,
        /// The bias as the library's unit value, -1..1 about a neutral 0 — not the percentage the window shows.
        /// Clamped into range rather than refused — see [`Requested::resolve`].
        bias: f64,
    },

    /// A sharpen run: which model, at which precision, applied how strongly.
    Sharpen {
        /// The model's developer-facing identifier, as [`opai::VariantEntry::codename`] publishes it.
        codename: String,
        /// The precision to run at, as [`opai::VariantEntry::precisions`] publishes them.
        precision: Precision,
        /// The strength as the library's unit value, 0..3 where 1 is the model's own output — not the percentage the
        /// window shows. Clamped into range rather than refused — see [`Requested::resolve`].
        strength: f64,
    },

    /// A colorization run: which model, at which precision.
    ///
    /// **No value.** Colorization takes no parameter, so a field here would describe a choice the user cannot make —
    /// see design.md D1 of `add-gui-colorization`.
    Colorization {
        /// The model's developer-facing identifier, as [`opai::VariantEntry::codename`] publishes it.
        codename: String,
        /// The precision to run at, as [`opai::VariantEntry::precisions`] publishes them.
        precision: Precision,
    },
}

/// Why an operation the window named cannot be run.
///
/// Two cases: an unpublished codename, or a precision the model was never released at (e.g. `up_osaka_fp32`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum UnknownOperation {
    /// No variant of this family carries that codename.
    #[error("{} publishes no model called `{codename}`", family.label())]
    Model {
        /// The family the codename was looked for in, as the catalogue spells it.
        family: Family,
        /// What was asked for.
        codename: String,
    },

    /// The model exists but isn't published at the precision asked for.
    #[error("`{codename}` is not published at {precision}")]
    Precision {
        /// The model that was found.
        codename: String,
        /// The precision it does not carry.
        precision: Precision,
    },
}

impl From<BuildError> for UnknownOperation {
    /// The library's refusal in the words the window already knows.
    ///
    /// One arm each, and the two types say the same two things — which is why this is a mapping rather than a
    /// re-export: [`UnknownOperation`] is what crosses the wire, tagged and shaped for the window, and a library
    /// error is neither. Nothing is lost on the way through and the rendered sentences are the same.
    fn from(error: BuildError) -> Self {
        match error {
            BuildError::UnknownModel { family, codename } => Self::Model { family, codename },
            BuildError::UnpublishedPrecision { codename, precision } => {
                Self::Precision { codename: codename.to_string(), precision }
            }
        }
    }
}

impl Requested {
    /// The [`opai::Operation`] this names, or why it cannot be run.
    ///
    /// **No place in this crate names a model.** The codename and the precision go to the library, which resolves
    /// them against the same catalogue this window built its chooser from and hands back the run they name — so a
    /// model `opai` publishes is runnable by being published, and there is no table here to keep in step with it.
    ///
    /// What is left is the one decision this crate owns: a scale, a strength or a bias is clamped via
    /// [`Scale::clamped`], [`Strength::clamped`] or [`Bias::clamped`] rather than refused, because the slider that
    /// drives it is already bounded — an out-of-range value can only be a fault, and clamping is more useful to the user
    /// than a failed enhancement. See design.md D7.
    ///
    /// # Errors
    ///
    /// [`UnknownOperation`]. Nothing is fetched or run until the whole chain has been checked.
    pub(crate) fn resolve(&self) -> Result<opai::Operation, UnknownOperation> {
        match self {
            Self::Upscale { codename, precision, scale } => {
                // Logged rather than refused, so a control that sent an out-of-range value is diagnosable.
                if !(Scale::MIN..=Scale::MAX).contains(scale) {
                    tracing::warn!(
                        codename,
                        scale,
                        min = Scale::MIN,
                        max = Scale::MAX,
                        "a scale outside the permitted range was requested; running at the nearest permitted value"
                    );
                }

                let values = ParameterValues::new().with_scale(Scale::clamped(*scale));
                let built = VariantEntry::published(Family::Upscale, codename)?.build(*precision, &values)?;

                enhancement(Family::Upscale, codename, built)
            }
            Self::Denoise { codename, precision, strength } => {
                // Logged rather than refused, for the reason the scale is.
                if !(Strength::MIN..=Strength::MAX).contains(strength) {
                    tracing::warn!(
                        codename,
                        strength,
                        min = Strength::MIN,
                        max = Strength::MAX,
                        "a strength outside the permitted range was requested; running at the nearest permitted value"
                    );
                }

                let values = ParameterValues::new().with_strength(Strength::clamped(*strength));
                let built = VariantEntry::published(Family::Denoise, codename)?.build(*precision, &values)?;

                enhancement(Family::Denoise, codename, built)
            }
            Self::FaceRecovery { codename, precision, faces } => {
                // `with_fidelity` although `ParameterValues::new()` already defaults it to `MAXIMUM`: one line, and
                // it pins the *window's* intent to run every face recovery at 1.0 rather than inheriting a library
                // default that is free to change. There is no per-model branch here because `build`'s own arm is
                // the branch — Athens reads the fidelity and Santorini has no parameter for one. See design.md D7.
                let values = ParameterValues::new()
                    .with_faces(Faces::new(faces.iter().copied()))
                    .with_fidelity(Fidelity::MAXIMUM);
                let built = VariantEntry::published(Family::FaceRecovery, codename)?.build(*precision, &values)?;

                enhancement(Family::FaceRecovery, codename, built)
            }
            Self::LightAdjustment { codename, precision, bias } => {
                // Logged rather than refused, for the reason the scale is.
                if !(Bias::MIN..=Bias::MAX).contains(bias) {
                    tracing::warn!(
                        codename,
                        bias,
                        min = Bias::MIN,
                        max = Bias::MAX,
                        "a bias outside the permitted range was requested; running at the nearest permitted value"
                    );
                }

                let values = ParameterValues::new().with_bias(Bias::clamped(*bias));
                let built = VariantEntry::published(Family::LightAdjustment, codename)?.build(*precision, &values)?;

                enhancement(Family::LightAdjustment, codename, built)
            }
            // Written out rather than folded into the arm above: serde needs one variant per tag, and a shared helper
            // would hide which family a reader is looking at. See design.md D1 of `add-gui-color-balance`.
            Self::ColorBalance { codename, precision, bias } => {
                if !(Bias::MIN..=Bias::MAX).contains(bias) {
                    tracing::warn!(
                        codename,
                        bias,
                        min = Bias::MIN,
                        max = Bias::MAX,
                        "a bias outside the permitted range was requested; running at the nearest permitted value"
                    );
                }

                let values = ParameterValues::new().with_bias(Bias::clamped(*bias));
                let built = VariantEntry::published(Family::ColorBalance, codename)?.build(*precision, &values)?;

                enhancement(Family::ColorBalance, codename, built)
            }
            // Written out rather than folded into the denoise arm: serde needs one variant per tag, and a shared helper
            // would hide which family a reader is looking at. See design.md D1 of `add-gui-sharpen`.
            Self::Sharpen { codename, precision, strength } => {
                if !(Strength::MIN..=Strength::MAX).contains(strength) {
                    tracing::warn!(
                        codename,
                        strength,
                        min = Strength::MIN,
                        max = Strength::MAX,
                        "a strength outside the permitted range was requested; running at the nearest permitted value"
                    );
                }

                let values = ParameterValues::new().with_strength(Strength::clamped(*strength));
                let built = VariantEntry::published(Family::Sharpen, codename)?.build(*precision, &values)?;

                enhancement(Family::Sharpen, codename, built)
            }
            Self::Colorization { codename, precision } => {
                let built = VariantEntry::published(Family::Colorization, codename)?
                    .build(*precision, &ParameterValues::new())?;

                enhancement(Family::Colorization, codename, built)
            }
        }
    }
}

/// The enhancement `built` carries, or a refusal naming what could not be served.
///
/// Unreachable while every model these families publish produces an image, which is a property of the library
/// rather than of anything the window can send. Answered rather than asserted, for the reason the codename match
/// this replaced was: a refusal, not a panic reachable from the wire.
fn enhancement(family: Family, codename: &str, built: Subject) -> Result<opai::Operation, UnknownOperation> {
    match built {
        Subject::Enhancement(operation) => Ok(operation),
        Subject::Analysis(_) => Err(UnknownOperation::Model { family, codename: codename.to_string() }),
    }
}

#[cfg(test)]
mod tests {
    // The typed constructors the assertions compare against: what `resolve` produced has to be the operation the
    // library's own constructor produces, not merely something of the right family.
    use opai::{
        ColorBalance, Colorization, Confidence, Denoise, FaceRecovery, FloatPrecision, LightAdjustment, Point, Rect,
        Sharpen, Upscale,
    };

    use super::*;

    /// One face, as the detector would have reported it.
    fn face(left: f32, top: f32) -> Face {
        Face::new(
            Rect::new(Point::new(left, top), Point::new(left + 40.0, top + 50.0)),
            [Point::new(left + 10.0, top + 15.0); Face::LANDMARKS],
            Confidence::new(0.9).expect("0.9 is inside the permitted range"),
        )
    }

    /// The faces a window would hand back for a photograph with two people in it.
    fn two_faces() -> Vec<Face> {
        vec![face(10.0, 20.0), face(120.0, 30.0)]
    }

    /// The float precision the wire's own [`Precision`] names.
    ///
    /// Written out here rather than taken from the library, which widens in one direction only: narrowing would let
    /// `int8` name a build the float families never publish, and every caller of this is already inside a float
    /// family's own published list.
    fn float(precision: Precision) -> FloatPrecision {
        match precision {
            Precision::Fp32 => FloatPrecision::Fp32,
            Precision::Fp16 => FloatPrecision::Fp16,
            Precision::Int8 => {
                panic!(
                    "denoise, face recovery, light adjustment, colour balance, sharpen and colorization publish no INT8 build"
                )
            }
        }
    }

    /// What the window would send for one face recovery.
    fn recovery(codename: &str, precision: &str, faces: &[Face]) -> serde_json::Value {
        serde_json::json!({
            "family": "face_recovery",
            "codename": codename,
            "precision": precision,
            "faces": faces,
        })
    }

    /// The face-recovery family as the catalogue publishes it.
    fn published_face_recovery() -> &'static opai::FamilyEntry {
        opai::catalogue()
            .iter()
            .find(|entry| entry.family == Family::FaceRecovery)
            .expect("face recovery is a family the library publishes")
    }

    /// What the window would send for one upscale.
    fn upscale(codename: &str, precision: &str, scale: f64) -> serde_json::Value {
        serde_json::json!({ "family": "upscale", "codename": codename, "precision": precision, "scale": scale })
    }

    /// The wire shape parsed from what the window would actually send.
    fn parse(value: serde_json::Value) -> Requested {
        serde_json::from_value(value).expect("the operation should deserialize")
    }

    /// Every upscale model, at every precision the catalogue says it is published at — driven from the
    /// catalogue rather than a hardcoded list, so a model added there and unnamed here fails this test.
    #[test]
    fn every_published_upscale_model_can_be_named_at_every_precision_it_is_published_at() {
        let upscale = opai::catalogue()
            .iter()
            .find(|entry| entry.family == Family::Upscale)
            .expect("upscale is a family the library publishes");

        let mut resolved = 0;

        for variant in &upscale.variants {
            for precision in variant.precisions {
                let operation = parse(upscale_of(variant.codename, *precision))
                    .resolve()
                    .unwrap_or_else(|error| panic!("{} at {precision} should resolve: {error}", variant.codename));

                assert_eq!(
                    operation.family(),
                    Family::Upscale,
                    "{} resolved to another family's operation",
                    variant.codename
                );
                assert_eq!(
                    operation.precision(),
                    *precision,
                    "{} resolved at a precision other than the one asked for",
                    variant.codename
                );

                resolved += 1;
            }
        }

        // Asserted as "at least as many as the models" rather than a literal, so a new model/precision
        // doesn't fail a count nobody meant to pin.
        assert!(resolved >= upscale.variants.len(), "a published variant resolved at no precision at all");
    }

    /// The same request, built from a [`Precision`] rather than from a string literal.
    fn upscale_of(codename: &str, precision: Precision) -> serde_json::Value {
        upscale(codename, precision.as_str(), 2.0)
    }

    #[test]
    fn a_model_the_library_publishes_needs_no_second_registration_here() {
        // The bug this closes. A model `opai` published and this crate had not been told about passed the
        // catalogue check, was offered by both choosers, and was then refused by the hardcoded match underneath -
        // the application refusing a model it publishes.
        //
        // What makes that unwritable now is that `resolve` adds nothing to the library's own answer but the clamp:
        // asserted against the seam rather than against a list of four models, so the statement is "whatever the
        // catalogue publishes" and not "whatever was published when this was written".
        for variant in &opai::catalogue()
            .iter()
            .find(|entry| entry.family == Family::Upscale)
            .expect("upscale is a family the library publishes")
            .variants
        {
            for precision in variant.precisions {
                let resolved = parse(upscale_of(variant.codename, *precision))
                    .resolve()
                    .unwrap_or_else(|error| panic!("{} at {precision}: {error}", variant.codename));

                let built = VariantEntry::published(Family::Upscale, variant.codename)
                    .expect("the catalogue published it a moment ago")
                    .build(*precision, &ParameterValues::new().with_scale(Scale::clamped(2.0)))
                    .expect("the catalogue published this precision a moment ago");

                assert_eq!(
                    Subject::Enhancement(resolved),
                    built,
                    "{} at {precision} resolved to something other than what the library builds",
                    variant.codename
                );
            }
        }
    }

    #[test]
    fn a_valid_pairing_resolves_to_the_operation_the_library_constructs() {
        let resolved = parse(upscale("kyoto", "fp32", 4.0)).resolve().expect("Kyoto is published at FP32");

        assert_eq!(resolved, Upscale::kyoto(FloatPrecision::Fp32, Scale::clamped(4.0)));
    }

    #[test]
    fn a_codename_the_family_does_not_publish_is_refused() {
        let refused = parse(upscale("berlin", "fp32", 2.0)).resolve().expect_err("Berlin is not an upscale model");

        assert_eq!(
            refused,
            UnknownOperation::Model { family: Family::Upscale, codename: "berlin".to_string() },
            "the refusal did not name what could not be served"
        );
    }

    #[test]
    fn a_precision_the_model_is_not_published_at_is_refused() {
        // No FP32 build of the diffusion transformer exists.
        let refused = parse(upscale("osaka", "fp32", 2.0)).resolve().expect_err("Osaka has no FP32 build");

        assert_eq!(
            refused,
            UnknownOperation::Precision { codename: "osaka".to_string(), precision: Precision::Fp32 }
        );

        // The mirror: no INT8 build of Kyoto either.
        let refused = parse(upscale("kyoto", "int8", 2.0)).resolve().expect_err("Kyoto has no INT8 build");

        assert_eq!(
            refused,
            UnknownOperation::Precision { codename: "kyoto".to_string(), precision: Precision::Int8 }
        );
    }

    #[test]
    fn a_scale_inside_the_range_runs_at_exactly_that_scale() {
        let resolved = parse(upscale("tokyo", "fp16", 3.5)).resolve().expect("Tokyo is published at FP16");

        assert_eq!(resolved, Upscale::tokyo(FloatPrecision::Fp16, Scale::clamped(3.5)));
    }

    #[test]
    fn a_scale_outside_the_range_runs_at_the_nearest_permitted_one() {
        let above = parse(upscale("tokyo", "fp16", 99.0)).resolve().expect("an out-of-range scale is not a refusal");
        let below = parse(upscale("tokyo", "fp16", 0.1)).resolve().expect("an out-of-range scale is not a refusal");

        assert_eq!(above, Upscale::tokyo(FloatPrecision::Fp16, Scale::clamped(Scale::MAX)));
        assert_eq!(below, Upscale::tokyo(FloatPrecision::Fp16, Scale::clamped(Scale::MIN)));
    }

    #[test]
    fn the_family_tag_is_the_one_the_library_spells() {
        let tagged = serde_json::to_value(Family::Upscale).expect("a family serializes");

        assert_eq!(tagged, serde_json::json!("upscale"));
        assert!(
            serde_json::from_value::<Requested>(upscale("tokyo", "fp32", 2.0)).is_ok(),
            "the wire shape does not parse under the tag the library publishes"
        );
    }

    /// Every face-recovery model, at every precision the catalogue says it is published at — driven from the
    /// catalogue rather than a hardcoded list, so a model added there and unnamed here fails this test.
    #[test]
    fn every_published_face_recovery_model_can_be_named_at_every_precision_it_is_published_at() {
        let faces = two_faces();
        let mut resolved = 0;

        for variant in &published_face_recovery().variants {
            for precision in variant.precisions {
                let operation = parse(recovery(variant.codename, precision.as_str(), &faces))
                    .resolve()
                    .unwrap_or_else(|error| panic!("{} at {precision} should resolve: {error}", variant.codename));

                assert_eq!(
                    operation.family(),
                    Family::FaceRecovery,
                    "{} resolved to another family's operation",
                    variant.codename
                );
                assert_eq!(
                    operation.precision(),
                    *precision,
                    "{} resolved at a precision other than the one asked for",
                    variant.codename
                );

                resolved += 1;
            }
        }

        assert!(
            resolved >= published_face_recovery().variants.len(),
            "a published variant resolved at no precision"
        );
    }

    #[test]
    fn athens_resolves_to_the_operation_the_library_constructs_at_maximum_fidelity() {
        // D7: the fidelity is the window's own decision, fixed at 1.0 and never on the wire — so what `resolve`
        // produces has to be what the library's constructor produces when handed `Fidelity::MAXIMUM`.
        let faces = two_faces();

        for precision in published_face_recovery()
            .variants
            .iter()
            .find(|variant| variant.codename == "athens")
            .expect("Athens is a face-recovery model the library publishes")
            .precisions
        {
            let resolved = parse(recovery("athens", precision.as_str(), &faces))
                .resolve()
                .unwrap_or_else(|error| panic!("Athens at {precision}: {error}"));

            assert_eq!(
                resolved,
                FaceRecovery::athens(float(*precision), Faces::new(faces.iter().copied()), Fidelity::MAXIMUM),
                "Athens at {precision} resolved to something other than what the library builds"
            );
        }
    }

    #[test]
    fn santorini_resolves_to_the_operation_the_library_constructs_and_carries_no_fidelity() {
        // The other half of D7: handing a fidelity to Santorini is not a value being ignored, it is a value that
        // model's constructor has no parameter for — so the operation carries `None` and its cache tag is
        // unaffected by the one the window fixed.
        let faces = two_faces();

        for precision in published_face_recovery()
            .variants
            .iter()
            .find(|variant| variant.codename == "santorini")
            .expect("Santorini is a face-recovery model the library publishes")
            .precisions
        {
            let resolved = parse(recovery("santorini", precision.as_str(), &faces))
                .resolve()
                .unwrap_or_else(|error| panic!("Santorini at {precision}: {error}"));

            assert_eq!(
                resolved,
                FaceRecovery::santorini(float(*precision), Faces::new(faces.iter().copied())),
                "Santorini at {precision} resolved to something other than what the library builds"
            );

            let opai::Operation::FaceRecovery(recovered) = &resolved else {
                panic!("Santorini resolved to another family's operation");
            };
            assert_eq!(recovered.fidelity(), None, "Santorini was built carrying a fidelity it takes no parameter for");
        }
    }

    #[test]
    fn the_faces_a_recovery_carries_are_the_ones_the_window_sent_in_the_order_it_sent_them() {
        // `Faces` keeps the order it is given and folds an order-sensitive signature of the boxes into the run
        // cache tag, so a reversal here would be a different stored result rather than a refusal.
        let faces = two_faces();
        let reversed = faces.iter().rev().copied().collect::<Vec<_>>();

        let forwards = parse(recovery("athens", "fp32", &faces)).resolve().expect("Athens is published at FP32");
        let backwards = parse(recovery("athens", "fp32", &reversed)).resolve().expect("Athens is published at FP32");

        let opai::Operation::FaceRecovery(recovered) = &forwards else {
            panic!("a face recovery resolved to another family's operation");
        };
        assert_eq!(recovered.faces().as_slice(), faces.as_slice(), "the faces sent are not the faces carried");
        assert_ne!(forwards, backwards, "two orderings of one selection resolved to the same operation");
    }

    #[test]
    fn a_codename_face_recovery_does_not_publish_is_refused() {
        let refused = parse(recovery("kyoto", "fp32", &two_faces()))
            .resolve()
            .expect_err("Kyoto is an upscale model, not a face-recovery one");

        assert_eq!(
            refused,
            UnknownOperation::Model { family: Family::FaceRecovery, codename: "kyoto".to_string() },
            "the refusal did not name what could not be served"
        );
    }

    #[test]
    fn a_precision_a_face_recovery_model_is_not_published_at_is_refused() {
        let refused = parse(recovery("athens", "int8", &two_faces())).resolve().expect_err("Athens has no INT8 build");

        assert_eq!(
            refused,
            UnknownOperation::Precision { codename: "athens".to_string(), precision: Precision::Int8 }
        );
    }

    #[test]
    fn a_face_recovery_carrying_no_faces_is_a_request_rather_than_a_refusal() {
        // D10: a detection that found nothing, or one that failed, still runs the chain — so an empty selection has
        // to resolve rather than be refused here.
        let resolved = parse(recovery("athens", "fp32", &[]))
            .resolve()
            .expect("an empty selection is a legitimate request");

        assert_eq!(resolved, FaceRecovery::athens(FloatPrecision::Fp32, Faces::empty(), Fidelity::MAXIMUM));
    }

    /// What the window would send for one light adjustment.
    fn light(codename: &str, precision: &str, bias: f64) -> serde_json::Value {
        serde_json::json!({ "family": "light_adjustment", "codename": codename, "precision": precision, "bias": bias })
    }

    /// The light-adjustment family as the catalogue publishes it.
    fn published_light_adjustment() -> &'static opai::FamilyEntry {
        opai::catalogue()
            .iter()
            .find(|entry| entry.family == Family::LightAdjustment)
            .expect("light adjustment is a family the library publishes")
    }

    /// The operation the library's own constructor builds for a light-adjustment codename.
    fn constructed_light(codename: &str, precision: FloatPrecision, bias: Bias) -> opai::Operation {
        match codename {
            "paris" => LightAdjustment::paris(precision, bias),
            "lyon" => LightAdjustment::lyon(precision, bias),
            other => panic!("the catalogue publishes a light adjustment `{other}` this test does not know"),
        }
    }

    /// Every light-adjustment model, at every precision the catalogue says it is published at, resolves to what the
    /// library's own constructor builds at the bias sent — driven from the catalogue, so a model added there and
    /// unknown here fails.
    #[test]
    fn every_published_light_adjustment_model_resolves_at_every_precision_to_what_the_library_constructs() {
        let mut resolved = 0;

        for variant in &published_light_adjustment().variants {
            for precision in variant.precisions {
                let operation = parse(light(variant.codename, precision.as_str(), 0.25))
                    .resolve()
                    .unwrap_or_else(|error| panic!("{} at {precision} should resolve: {error}", variant.codename));

                assert_eq!(
                    operation,
                    constructed_light(variant.codename, float(*precision), Bias::clamped(0.25)),
                    "{} at {precision} resolved to something other than what the library builds",
                    variant.codename
                );

                resolved += 1;
            }
        }

        assert!(
            resolved >= published_light_adjustment().variants.len(),
            "a published variant resolved at no precision"
        );
    }

    #[test]
    fn a_bias_outside_the_range_runs_at_the_nearest_permitted_one() {
        // `NaN` is not tested: JSON has no spelling for it, so the wire cannot carry one.
        let above = parse(light("paris", "fp32", 1.5)).resolve().expect("an out-of-range bias is not a refusal");
        let below = parse(light("paris", "fp32", -2.0)).resolve().expect("an out-of-range bias is not a refusal");

        assert_eq!(above, LightAdjustment::paris(FloatPrecision::Fp32, Bias::clamped(Bias::MAX)));
        assert_eq!(below, LightAdjustment::paris(FloatPrecision::Fp32, Bias::clamped(Bias::MIN)));
    }

    #[test]
    fn a_codename_light_adjustment_does_not_publish_is_refused() {
        let refused = parse(light("kyoto", "fp32", 0.5))
            .resolve()
            .expect_err("Kyoto is an upscale model, not a light-adjustment one");

        assert_eq!(
            refused,
            UnknownOperation::Model { family: Family::LightAdjustment, codename: "kyoto".to_string() },
            "the refusal did not name what could not be served"
        );
    }

    /// What the window would send for one colour balance.
    fn balance(codename: &str, precision: &str, bias: f64) -> serde_json::Value {
        serde_json::json!({ "family": "color_balance", "codename": codename, "precision": precision, "bias": bias })
    }

    /// The colour-balance family as the catalogue publishes it.
    fn published_color_balance() -> &'static opai::FamilyEntry {
        opai::catalogue()
            .iter()
            .find(|entry| entry.family == Family::ColorBalance)
            .expect("colour balance is a family the library publishes")
    }

    /// The operation the library's own constructor builds for a colour-balance codename.
    fn constructed_balance(codename: &str, precision: FloatPrecision, bias: Bias) -> opai::Operation {
        match codename {
            "rio" => ColorBalance::rio(precision, bias),
            "saopaulo" => ColorBalance::saopaulo(precision, bias),
            other => panic!("the catalogue publishes a colour balance `{other}` this test does not know"),
        }
    }

    /// Every colour-balance model, at every precision the catalogue says it is published at, resolves to what the
    /// library's own constructor builds at the bias sent — driven from the catalogue, so a model added there and
    /// unknown here fails.
    #[test]
    fn every_published_color_balance_model_resolves_at_every_precision_to_what_the_library_constructs() {
        let mut resolved = 0;

        for variant in &published_color_balance().variants {
            for precision in variant.precisions {
                let operation = parse(balance(variant.codename, precision.as_str(), -0.25))
                    .resolve()
                    .unwrap_or_else(|error| panic!("{} at {precision} should resolve: {error}", variant.codename));

                assert_eq!(
                    operation,
                    constructed_balance(variant.codename, float(*precision), Bias::clamped(-0.25)),
                    "{} at {precision} resolved to something other than what the library builds",
                    variant.codename
                );

                resolved += 1;
            }
        }

        assert!(
            resolved >= published_color_balance().variants.len(),
            "a published variant resolved at no precision"
        );
    }

    #[test]
    fn a_color_balance_bias_outside_the_range_runs_at_the_nearest_permitted_one() {
        let above = parse(balance("rio", "fp32", 1.5)).resolve().expect("an out-of-range bias is not a refusal");
        let below = parse(balance("rio", "fp32", -2.0)).resolve().expect("an out-of-range bias is not a refusal");

        assert_eq!(above, ColorBalance::rio(FloatPrecision::Fp32, Bias::clamped(Bias::MAX)));
        assert_eq!(below, ColorBalance::rio(FloatPrecision::Fp32, Bias::clamped(Bias::MIN)));
    }

    #[test]
    fn a_codename_color_balance_does_not_publish_is_refused() {
        let refused = parse(balance("paris", "fp32", 0.5))
            .resolve()
            .expect_err("Paris is a light-adjustment model, not a colour-balance one");

        assert_eq!(
            refused,
            UnknownOperation::Model { family: Family::ColorBalance, codename: "paris".to_string() },
            "the refusal did not name what could not be served"
        );
    }

    /// What the window would send for one denoise.
    fn denoise(codename: &str, precision: &str, strength: f64) -> serde_json::Value {
        serde_json::json!({ "family": "denoise", "codename": codename, "precision": precision, "strength": strength })
    }

    /// The denoise family as the catalogue publishes it.
    fn published_denoise() -> &'static opai::FamilyEntry {
        opai::catalogue()
            .iter()
            .find(|entry| entry.family == Family::Denoise)
            .expect("denoise is a family the library publishes")
    }

    /// The operation the library's own constructor builds for a denoise codename.
    fn constructed_denoise(codename: &str, precision: FloatPrecision, strength: Strength) -> opai::Operation {
        match codename {
            "stockholm" => Denoise::stockholm(precision, strength),
            "gothenburg" => Denoise::gothenburg(precision, strength),
            "malmo" => Denoise::malmo(precision, strength),
            other => panic!("the catalogue publishes a denoise `{other}` this test does not know"),
        }
    }

    /// Every denoise model, at every precision the catalogue says it is published at, resolves to what the library's
    /// own constructor builds at the strength sent — driven from the catalogue, so a model added there and unknown here
    /// fails.
    #[test]
    fn every_published_denoise_model_resolves_at_every_precision_to_what_the_library_constructs() {
        let mut resolved = 0;

        for variant in &published_denoise().variants {
            for precision in variant.precisions {
                let operation = parse(denoise(variant.codename, precision.as_str(), 1.5))
                    .resolve()
                    .unwrap_or_else(|error| panic!("{} at {precision} should resolve: {error}", variant.codename));

                assert_eq!(
                    operation,
                    constructed_denoise(variant.codename, float(*precision), Strength::clamped(1.5)),
                    "{} at {precision} resolved to something other than what the library builds",
                    variant.codename
                );

                resolved += 1;
            }
        }

        assert!(resolved >= published_denoise().variants.len(), "a published variant resolved at no precision");
    }

    #[test]
    fn a_strength_outside_the_range_runs_at_the_nearest_permitted_one() {
        let above = parse(denoise("stockholm", "fp32", 4.0))
            .resolve()
            .expect("an out-of-range strength is not a refusal");
        let below = parse(denoise("stockholm", "fp32", -1.0))
            .resolve()
            .expect("an out-of-range strength is not a refusal");

        assert_eq!(above, Denoise::stockholm(FloatPrecision::Fp32, Strength::clamped(Strength::MAX)));
        assert_eq!(below, Denoise::stockholm(FloatPrecision::Fp32, Strength::clamped(Strength::MIN)));
    }

    #[test]
    fn a_codename_denoise_does_not_publish_is_refused() {
        let refused = parse(denoise("rio", "fp32", 1.0))
            .resolve()
            .expect_err("Rio is a colour-balance model, not a denoise one");

        assert_eq!(
            refused,
            UnknownOperation::Model { family: Family::Denoise, codename: "rio".to_string() },
            "the refusal did not name what could not be served"
        );
    }

    /// What the window would send for one sharpen.
    fn sharpen(codename: &str, precision: &str, strength: f64) -> serde_json::Value {
        serde_json::json!({ "family": "sharpen", "codename": codename, "precision": precision, "strength": strength })
    }

    /// The sharpen family as the catalogue publishes it.
    fn published_sharpen() -> &'static opai::FamilyEntry {
        opai::catalogue()
            .iter()
            .find(|entry| entry.family == Family::Sharpen)
            .expect("sharpen is a family the library publishes")
    }

    /// The operation the library's own constructor builds for a sharpen codename.
    ///
    /// The codename is `petersburg`; only the constructor is spelled `stpetersburg`.
    fn constructed_sharpen(codename: &str, precision: FloatPrecision, strength: Strength) -> opai::Operation {
        match codename {
            "moscow" => Sharpen::moscow(precision, strength),
            "petersburg" => Sharpen::stpetersburg(precision, strength),
            "novgorod" => Sharpen::novgorod(precision, strength),
            other => panic!("the catalogue publishes a sharpen `{other}` this test does not know"),
        }
    }

    /// Every sharpen model, at every precision the catalogue says it is published at, resolves to what the library's
    /// own constructor builds at the strength sent — driven from the catalogue, so a model added there and unknown here
    /// fails.
    #[test]
    fn every_published_sharpen_model_resolves_at_every_precision_to_what_the_library_constructs() {
        let mut resolved = 0;

        for variant in &published_sharpen().variants {
            for precision in variant.precisions {
                let operation = parse(sharpen(variant.codename, precision.as_str(), 1.5))
                    .resolve()
                    .unwrap_or_else(|error| panic!("{} at {precision} should resolve: {error}", variant.codename));

                assert_eq!(
                    operation,
                    constructed_sharpen(variant.codename, float(*precision), Strength::clamped(1.5)),
                    "{} at {precision} resolved to something other than what the library builds",
                    variant.codename
                );

                resolved += 1;
            }
        }

        assert!(resolved >= published_sharpen().variants.len(), "a published variant resolved at no precision");
    }

    #[test]
    fn a_sharpen_strength_outside_the_range_runs_at_the_nearest_permitted_one() {
        let above = parse(sharpen("moscow", "fp32", 4.0))
            .resolve()
            .expect("an out-of-range strength is not a refusal");
        let below = parse(sharpen("moscow", "fp32", -1.0))
            .resolve()
            .expect("an out-of-range strength is not a refusal");

        assert_eq!(above, Sharpen::moscow(FloatPrecision::Fp32, Strength::clamped(Strength::MAX)));
        assert_eq!(below, Sharpen::moscow(FloatPrecision::Fp32, Strength::clamped(Strength::MIN)));
    }

    #[test]
    fn a_codename_sharpen_does_not_publish_is_refused() {
        let refused = parse(sharpen("stockholm", "fp32", 1.0))
            .resolve()
            .expect_err("Stockholm is a denoise model, not a sharpen one");

        assert_eq!(
            refused,
            UnknownOperation::Model { family: Family::Sharpen, codename: "stockholm".to_string() },
            "the refusal did not name what could not be served"
        );
    }

    /// What the window would send for one colorization.
    fn colorization(codename: &str, precision: &str) -> serde_json::Value {
        serde_json::json!({ "family": "colorization", "codename": codename, "precision": precision })
    }

    /// The colorization family as the catalogue publishes it.
    fn published_colorization() -> &'static opai::FamilyEntry {
        opai::catalogue()
            .iter()
            .find(|entry| entry.family == Family::Colorization)
            .expect("colorization is a family the library publishes")
    }

    /// The operation the library's own constructor builds for a colorization codename.
    fn constructed_colorization(codename: &str, precision: FloatPrecision) -> opai::Operation {
        match codename {
            "delhi" => Colorization::delhi(precision),
            "mumbai" => Colorization::mumbai(precision),
            "jaipur" => Colorization::jaipur(precision),
            other => panic!("the catalogue publishes a colorization `{other}` this test does not know"),
        }
    }

    /// Every colorization model, at every precision the catalogue says it is published at, resolves to what the
    /// library's own constructor builds — driven from the catalogue, so a model added there and unknown here fails.
    #[test]
    fn every_published_colorization_model_resolves_at_every_precision_to_what_the_library_constructs() {
        let mut resolved = 0;

        for variant in &published_colorization().variants {
            for precision in variant.precisions {
                let operation = parse(colorization(variant.codename, precision.as_str()))
                    .resolve()
                    .unwrap_or_else(|error| panic!("{} at {precision} should resolve: {error}", variant.codename));

                assert_eq!(
                    operation,
                    constructed_colorization(variant.codename, float(*precision)),
                    "{} at {precision} resolved to something other than what the library builds",
                    variant.codename
                );

                resolved += 1;
            }
        }

        assert!(
            resolved >= published_colorization().variants.len(),
            "a published variant resolved at no precision"
        );
    }

    #[test]
    fn a_codename_colorization_does_not_publish_is_refused() {
        let refused = parse(colorization("moscow", "fp32"))
            .resolve()
            .expect_err("Moscow is a sharpen model, not a colorization one");

        assert_eq!(
            refused,
            UnknownOperation::Model { family: Family::Colorization, codename: "moscow".to_string() },
            "the refusal did not name what could not be served"
        );
    }
}
