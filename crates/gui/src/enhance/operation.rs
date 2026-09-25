//! The operation on the wire: what the window names, and what it resolves to.
//!
//! An operation is named rather than spelled — see the module documentation in `mod.rs` for why, and
//! design.md D1.

use std::collections::BTreeMap;

use opai::{BuildError, Family, ParameterValues, Precision, Subject, VariantEntry};
use serde::{Deserialize, Serialize};

use crate::faces::FaceChoice;

/// One operation, as the window names it: a catalogue row, a precision, and the values of the row's parameters.
///
/// ```text
/// { "family": "upscale", "codename": "kyoto", "precision": "fp32", "parameters": { "scale": 2.0 } }
/// { "family": "face_recovery", "codename": "athens", "precision": "fp32", "parameters": { "fidelity": 1.0 },
///   "faces": { "skipped": ["10,20,110,140"], "restored": [] } }
/// ```
///
/// **One shape for every family**, spelled in the catalogue's own vocabulary: the family as [`Family`] spells it, the
/// codename and precision as [`VariantEntry`] publishes them, and each range parameter under the
/// [`ParameterEntry::name`](opai::ParameterEntry::name) it is published under. A chooser and a control built from
/// the catalogue therefore send back exactly what they read, and neither this crate nor the window keeps a per-family
/// table of which parameter a family takes — the row says so, and [`ParameterValues::set`] matches the name onto the
/// library's own bounded type.
///
/// **Not [`opai::Operation`] itself** — that type's `Deserialize` reads its internal enum shape, not the
/// catalogue's vocabulary.
///
/// Detection is named by the catalogue and refused here as a model with no image result: the window asks for faces
/// through [`crate::faces::detect_faces`], never as an operation in a chain.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Requested {
    /// Which family the codename is looked up in.
    pub(crate) family: Family,
    /// The model's developer-facing identifier, as [`VariantEntry::codename`] publishes it.
    pub(crate) codename: String,
    /// The precision to run at, as [`VariantEntry::precisions`] publishes them.
    pub(crate) precision: Precision,
    /// Each range parameter's value by its published name, in the library's own unit — a strength of 1 is the
    /// model's own output, not the percentage the window shows. Clamped into range rather than refused — see
    /// [`Requested::resolve`].
    ///
    /// A parameter left out runs at [`ParameterValues::new`]'s neutral value; the window sends every one its row
    /// publishes, starting from the catalogue's published defaults. Absent altogether for a family that takes none.
    #[serde(default)]
    pub(crate) parameters: BTreeMap<String, f64>,
    /// Which of the faces found a face recovery restores — the user's exceptions to the default, by key — and
    /// absent for every other family, or for a recovery nobody has chosen faces for.
    ///
    /// **Not the faces.** The run finds them itself, inside the same request — see [`crate::faces::for_chain`] — and
    /// [`resolve`](Self::resolve) builds every face recovery over an empty selection for it to fill in.
    #[serde(default)]
    pub(crate) faces: Option<FaceChoice>,
}

/// Why an operation the window named cannot be run.
///
/// Three cases: an unpublished codename, a precision the model was never released at (e.g. `up_osaka_fp32`), or a
/// parameter name nothing publishes.
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

    /// No parameter is published under that name — the window's own mistake, since it only sends names it read off
    /// the catalogue.
    #[error("no model takes a parameter called `{name}`")]
    Parameter {
        /// What was sent.
        name: String,
    },
}

impl From<BuildError> for UnknownOperation {
    /// The library's refusal in the words the window already knows.
    ///
    /// One arm each, and the two types say the same three things — which is why this is a mapping rather than a
    /// re-export: [`UnknownOperation`] is what crosses the wire, tagged and shaped for the window, and a library
    /// error is neither. Nothing is lost on the way through and the rendered sentences are the same.
    fn from(error: BuildError) -> Self {
        match error {
            BuildError::UnknownModel { family, codename } => Self::Model { family, codename },
            BuildError::UnpublishedPrecision { codename, precision } => {
                Self::Precision { codename: codename.to_string(), precision }
            }
            BuildError::UnknownParameter { name } => Self::Parameter { name },
        }
    }
}

impl Requested {
    /// The [`opai::Operation`] this names, or why it cannot be run.
    ///
    /// **No place in this crate names a model or a parameter.** The codename and the precision go to the library,
    /// which resolves them against the same catalogue this window built its chooser from; each parameter goes to
    /// [`ParameterValues::set`] under the name the catalogue published it with. A model or a parameter `opai`
    /// publishes is runnable by being published, and there is no table here to keep in step with it.
    ///
    /// A value out of range is clamped rather than refused, and logged once by the library: the control that drives
    /// it is already bounded, so an out-of-range value can only be a fault, and clamping is more useful to the user
    /// than a failed enhancement. See design.md D7.
    ///
    /// **No fidelity is fixed here any more.** Athens runs at the fidelity the window sends, which starts at the
    /// catalogue's published default — maximum fidelity, the value this crate used to pin — and at
    /// [`ParameterValues::new`]'s, the same maximum, where none is sent.
    ///
    /// # Errors
    ///
    /// [`UnknownOperation`]. Nothing is fetched or run until the whole chain has been checked.
    pub(crate) fn resolve(&self) -> Result<opai::Operation, UnknownOperation> {
        let row = VariantEntry::published(self.family, &self.codename)?;

        // No faces: they are found inside the run, after the whole chain has been judged here. See `faces`.
        let mut values = ParameterValues::new();
        for (name, value) in &self.parameters {
            values.set(name, *value)?;
        }

        match row.build(self.precision, &values)? {
            Subject::Enhancement(operation) => Ok(operation),
            // Detection, the one family whose result is not an image: published, and not something a chain can
            // hold. Answered rather than asserted, so it is a refusal rather than a panic reachable from the wire.
            Subject::Analysis(_) => {
                Err(UnknownOperation::Model { family: self.family, codename: self.codename.clone() })
            }
        }
    }

    /// The operation naming `codename` of `family` at `precision`, carrying `parameters` — what the window would send,
    /// for the tests that build a chain.
    #[cfg(test)]
    pub(crate) fn named(family: Family, codename: &str, precision: Precision, parameters: &[(&str, f64)]) -> Self {
        Self {
            family,
            codename: codename.to_string(),
            precision,
            parameters: parameters.iter().map(|(name, value)| ((*name).to_string(), *value)).collect(),
            faces: None,
        }
    }
}

#[cfg(test)]
mod tests {
    // The typed constructors the assertions compare against: what `resolve` produced has to be the operation the
    // library's own constructor produces, not merely something of the right family.
    use opai::{
        Bias, ColorBalance, Colorization, Denoise, FaceRecovery, Faces, Fidelity, FloatPrecision, LightAdjustment,
        Scale, Sharpen, Strength, Upscale,
    };

    use super::*;

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

    /// What the window would send for one face recovery, with one face skipped by key.
    fn recovery(codename: &str, precision: &str) -> serde_json::Value {
        serde_json::json!({
            "family": "face_recovery",
            "codename": codename,
            "precision": precision,
            "faces": { "skipped": ["10,20,50,70"], "restored": [] },
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
        serde_json::json!({
            "family": "upscale", "codename": codename, "precision": precision, "parameters": { "scale": scale },
        })
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
        let mut resolved = 0;

        for variant in &published_face_recovery().variants {
            for precision in variant.precisions {
                let operation = parse(recovery(variant.codename, precision.as_str()))
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
        // A face recovery sent with no fidelity runs at `ParameterValues::new`'s, which is `Fidelity::MAXIMUM` — the
        // value this crate used to pin, and the catalogue's published default.

        for precision in published_face_recovery()
            .variants
            .iter()
            .find(|variant| variant.codename == "athens")
            .expect("Athens is a face-recovery model the library publishes")
            .precisions
        {
            let resolved = parse(recovery("athens", precision.as_str()))
                .resolve()
                .unwrap_or_else(|error| panic!("Athens at {precision}: {error}"));

            assert_eq!(
                resolved,
                FaceRecovery::athens(float(*precision), Faces::empty(), Fidelity::MAXIMUM),
                "Athens at {precision} resolved to something other than what the library builds"
            );
        }
    }

    #[test]
    fn santorini_resolves_to_the_operation_the_library_constructs_and_carries_no_fidelity() {
        // Handing a fidelity to Santorini is not a value being ignored, it is a value that model's constructor has no
        // parameter for — so the operation carries `None` and its cache tag is unaffected by any the window sends.

        for precision in published_face_recovery()
            .variants
            .iter()
            .find(|variant| variant.codename == "santorini")
            .expect("Santorini is a face-recovery model the library publishes")
            .precisions
        {
            let resolved = parse(recovery("santorini", precision.as_str()))
                .resolve()
                .unwrap_or_else(|error| panic!("Santorini at {precision}: {error}"));

            assert_eq!(
                resolved,
                FaceRecovery::santorini(float(*precision), Faces::empty()),
                "Santorini at {precision} resolved to something other than what the library builds"
            );

            let opai::Operation::FaceRecovery(recovered) = &resolved else {
                panic!("Santorini resolved to another family's operation");
            };
            assert_eq!(recovered.fidelity(), None, "Santorini was built carrying a fidelity it takes no parameter for");
        }
    }

    #[test]
    fn the_choice_a_recovery_carries_is_read_and_it_resolves_over_no_faces_for_the_run_to_fill() {
        // The faces are found inside the run — `crate::faces::for_chain` — so what the window names resolves to the
        // model over an empty selection, and the choice travels beside it.
        let requested = parse(recovery("athens", "fp32"));

        assert_eq!(
            requested.faces,
            Some(FaceChoice { skipped: vec!["10,20,50,70".to_string()], restored: Vec::new() })
        );
        assert_eq!(
            requested.resolve().expect("Athens is published at FP32"),
            FaceRecovery::athens(FloatPrecision::Fp32, Faces::empty(), Fidelity::MAXIMUM)
        );
    }

    #[test]
    fn a_codename_face_recovery_does_not_publish_is_refused() {
        let refused = parse(recovery("kyoto", "fp32"))
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
        let refused = parse(recovery("athens", "int8")).resolve().expect_err("Athens has no INT8 build");

        assert_eq!(
            refused,
            UnknownOperation::Precision { codename: "athens".to_string(), precision: Precision::Int8 }
        );
    }

    #[test]
    fn a_face_recovery_nobody_chose_faces_for_is_a_request_rather_than_a_refusal() {
        // A recovery added a moment ago carries no choice at all, and every face then follows the default.
        let mut sent = recovery("athens", "fp32");
        sent.as_object_mut().expect("an object").remove("faces");

        let requested = parse(sent);
        assert_eq!(requested.faces, None);
        assert_eq!(
            requested.resolve().expect("a recovery with no choice is a legitimate request"),
            FaceRecovery::athens(FloatPrecision::Fp32, Faces::empty(), Fidelity::MAXIMUM)
        );
    }

    /// What the window would send for one light adjustment.
    fn light(codename: &str, precision: &str, bias: f64) -> serde_json::Value {
        serde_json::json!({
            "family": "light_adjustment", "codename": codename, "precision": precision, "parameters": { "bias": bias },
        })
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
        serde_json::json!({
            "family": "color_balance", "codename": codename, "precision": precision, "parameters": { "bias": bias },
        })
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
        serde_json::json!({
            "family": "denoise", "codename": codename, "precision": precision, "parameters": { "strength": strength },
        })
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
        serde_json::json!({
            "family": "sharpen", "codename": codename, "precision": precision, "parameters": { "strength": strength },
        })
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

    #[test]
    fn athens_runs_at_the_fidelity_the_window_sends_and_at_maximum_where_it_sends_none() {
        let mut sent = recovery("athens", "fp32");
        sent["parameters"] = serde_json::json!({ "fidelity": 0.25 });

        assert_eq!(
            parse(sent).resolve().expect("Athens is published at FP32"),
            FaceRecovery::athens(FloatPrecision::Fp32, Faces::empty(), Fidelity::clamped(0.25))
        );

        // The catalogue's published default is the maximum the window used to pin, so a new Athens runs as before.
        let default = published_face_recovery().variants[0]
            .parameters
            .iter()
            .find_map(|parameter| match parameter.kind {
                opai::ParameterKind::Range { default, .. } if parameter.name == Fidelity::NAME => Some(default),
                _ => None,
            })
            .expect("Athens publishes a fidelity");
        assert_eq!(Fidelity::clamped(default), Fidelity::MAXIMUM);
    }

    #[test]
    fn a_parameter_name_nothing_publishes_is_refused_by_name() {
        let mut sent = upscale("kyoto", "fp32", 2.0);
        sent["parameters"]["radius"] = serde_json::json!(3.0);

        assert_eq!(
            parse(sent).resolve().expect_err("no model takes a radius"),
            UnknownOperation::Parameter { name: "radius".to_string() }
        );
    }

    #[test]
    fn a_family_taking_no_parameter_needs_no_parameters_field_and_detection_is_not_a_chain_step() {
        assert!(parse(colorization("delhi", "fp32")).parameters.is_empty());

        let refused = parse(serde_json::json!({ "family": "detection", "codename": "newyork", "precision": "fp32" }))
            .resolve()
            .expect_err("a detection has no image result");
        assert_eq!(refused, UnknownOperation::Model { family: Family::Detection, codename: "newyork".to_string() });
    }

    #[test]
    fn the_refusals_keep_their_wire_shape() {
        // Pinned before the operation's own shape changed, so the refusal the window already maps did not move with
        // it.
        let json = |refusal: UnknownOperation| serde_json::to_value(refusal).expect("a refusal serializes");

        assert_eq!(
            json(UnknownOperation::Model { family: Family::Upscale, codename: "berlin".to_string() }),
            serde_json::json!({ "kind": "model", "family": "upscale", "codename": "berlin" })
        );
        assert_eq!(
            json(UnknownOperation::Precision { codename: "osaka".to_string(), precision: Precision::Fp32 }),
            serde_json::json!({ "kind": "precision", "codename": "osaka", "precision": "fp32" })
        );
        assert_eq!(
            json(UnknownOperation::Parameter { name: "radius".to_string() }),
            serde_json::json!({ "kind": "parameter", "name": "radius" })
        );
    }
}
