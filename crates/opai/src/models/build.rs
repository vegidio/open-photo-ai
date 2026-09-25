//! The catalogue's inverse: turning a row a front end read back into the run it names.
//!
//! [`catalogue`](fn@super::catalogue) is descriptive — it publishes every family, every variant's codename and label,
//! every precision that variant is published at, and the parameters it takes. This file is what makes it
//! constructive, so that the only thing a front end needs in order to *run* a model is the same row it drew the
//! chooser from.
//!
//! Without it, a front end that can describe a model still has to name one to build it, and the hand-written table
//! that does the naming is a second record of which models exist. The two drift in one direction only: a model the
//! library publishes is offered by every chooser and then refused by the table that was never updated.
//!
//! # What can be wrong, and what cannot
//!
//! Exactly two things, and both are decided by data the entry holds: a codename the family does not publish, and a
//! precision this variant is not published at. Everything else is a value the caller already holds — a [`Scale`], a
//! [`Strength`], a [`Bias`], a [`Fidelity`] or a set of [`Faces`] — and each of those is bounded on construction, so
//! there is no third way for a request to be wrong and no third arm on [`BuildError`].

use super::artifact::Family;
use super::bias::Bias;
use super::catalogue::{VariantEntry, catalogue};
use super::color_balance::{ColorBalance, ColorBalanceVariant};
use super::colorization::{Colorization, ColorizationVariant};
use super::denoise::{Denoise, DenoiseVariant};
use super::detection::{Detection, DetectionVariant};
use super::face::Faces;
use super::face_recovery::{FaceRecovery, FaceRecoveryVariant};
use super::fidelity::Fidelity;
use super::light_adjustment::{LightAdjustment, LightAdjustmentVariant};
use super::precision::Precision;
use super::scale::Scale;
use super::sharpen::{Sharpen, SharpenVariant};
use super::strength::Strength;
use super::upscale::{Upscale, UpscaleVariant};

use super::subject::Subject;

/// Why a request naming a model could not be turned into a run.
///
/// The two ways a request can be wrong, and there is no third: what a caller supplies beyond the codename and the
/// precision is values that were bounded when they were built. See this module's header.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BuildError {
    /// No variant of that family carries that codename.
    #[error("{} publishes no model called `{codename}`", family.label())]
    UnknownModel {
        /// The family the codename was looked for in.
        family: Family,
        /// What was asked for. A [`String`] rather than a borrow, because unlike the field below it is the caller's
        /// word and not one the catalogue published.
        codename: String,
    },

    /// The model is published, but not at the precision asked for: Osaka has no FP32 build, and the three
    /// convolutional upscalers have no INT8 one.
    #[error("`{codename}` is not published at {precision}")]
    UnpublishedPrecision {
        /// The model that was found, borrowed from the row that publishes it.
        codename: &'static str,
        /// The precision it is not published at.
        precision: Precision,
    },
}

/// The per-run values a model is built with, as distinct from the bounds the catalogue publishes for them.
///
/// A [`ParameterEntry`](super::catalogue::ParameterEntry) carries a parameter's *range*; an operation carries a
/// *value*. This is where the values are handed over, and it is total rather than a list that could be missing one:
/// a caller that has not set the parameter its chosen variant takes gets that parameter's neutral value, not a
/// refusal it would have to handle and not a panic. Which parameters a variant reads is the variant's own business
/// and is published on its row, so a caller sets what the row lists and the rest is never looked at.
///
/// Built by [`new`](Self::new) and narrowed by the `with_*` methods, each taking the same bounded type the model's
/// own constructor takes — so a value that exists here is already a value in range and there is nothing for the
/// build to refuse.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterValues {
    /// What an upscale enlarges by.
    scale: Scale,
    /// What denoise and sharpen are applied at.
    strength: Strength,
    /// Which way light adjustment and colour balance lean.
    bias: Bias,
    /// How closely Athens keeps to the face it was given.
    fidelity: Fidelity,
    /// The faces a face-recovery run restores, as a detection run found them.
    faces: Faces,
}

impl ParameterValues {
    /// Every parameter at its neutral value: no enlargement, no strength, no lean, Athens' own default fidelity and
    /// no faces.
    ///
    /// Neutral rather than arbitrary, so that a caller which forgot to set the parameter its variant takes gets the
    /// run that does the least rather than one it did not ask for. [`Fidelity`] is the exception and is
    /// [`Fidelity::MAXIMUM`], which is not a neutral value but the one the reference implementation hard-codes and
    /// the one a front end offers before a user touches the control.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scale: Scale::clamped(Scale::MIN),
            strength: Strength::clamped(Strength::MIN),
            bias: Bias::clamped(0.0),
            fidelity: Fidelity::MAXIMUM,
            faces: Faces::empty(),
        }
    }

    /// What an upscale enlarges by.
    #[must_use]
    pub fn with_scale(mut self, scale: Scale) -> Self {
        self.scale = scale;
        self
    }

    /// What a denoise or a sharpen is applied at.
    #[must_use]
    pub fn with_strength(mut self, strength: Strength) -> Self {
        self.strength = strength;
        self
    }

    /// Which way a light adjustment or a colour balance leans.
    #[must_use]
    pub fn with_bias(mut self, bias: Bias) -> Self {
        self.bias = bias;
        self
    }

    /// How closely Athens keeps to the face it was given.
    #[must_use]
    pub fn with_fidelity(mut self, fidelity: Fidelity) -> Self {
        self.fidelity = fidelity;
        self
    }

    /// The faces a face-recovery run restores, as a detection run found them.
    #[must_use]
    pub fn with_faces(mut self, faces: Faces) -> Self {
        self.faces = faces;
        self
    }
}

impl Default for ParameterValues {
    fn default() -> Self {
        Self::new()
    }
}

impl VariantEntry {
    /// The row `family` publishes under `codename`.
    ///
    /// The lookup half of the seam, against [`fn@catalogue`] and nothing else — so what exists is decided in exactly
    /// one place. A codename another family publishes is refused here as firmly as one no family publishes: the
    /// pairing is what is looked up, not the name.
    ///
    /// # Errors
    ///
    /// [`BuildError::UnknownModel`], naming the family it was looked for in.
    pub fn published(family: Family, codename: &str) -> Result<&'static Self, BuildError> {
        catalogue()
            .iter()
            .filter(|entry| entry.family == family)
            .flat_map(|entry| entry.variants.iter())
            .find(|variant| variant.codename == codename)
            .ok_or_else(|| BuildError::UnknownModel { family, codename: codename.to_string() })
    }

    /// The run this row names, at `precision`, carrying `values`.
    ///
    /// The construction half of the seam. It is a method on the row rather than a function over strings because the
    /// row is what *knows*: the precisions it is published at and the parameters it takes are fields of it, so the
    /// check and the construction read the same data and there is nothing for a second table to disagree with.
    ///
    /// A [`Subject`] rather than an [`Operation`](super::Operation), because the eight families are carried in two
    /// values — the seven whose result is an image and the one whose result is not — and a caller walking the
    /// catalogue crosses both. A caller that knows it asked an enhancement family matches
    /// [`Subject::Enhancement`] and has the operation [`Opai::process`](crate::Opai::process) takes.
    ///
    /// There is no per-model precision mapping here, and deliberately none: each family's own arm calls the
    /// constructor for the variant its `from_codename` resolved, so the precision domain is known statically at the
    /// one place that calls it. Osaka having no FP32 build is stated once, by the row, and is refused below by the
    /// same `contains` every other variant is refused by.
    ///
    /// # Errors
    ///
    /// [`BuildError::UnpublishedPrecision`] where this row does not list `precision`.
    pub fn build(&self, precision: Precision, values: &ParameterValues) -> Result<Subject, BuildError> {
        // The one check, asked of the row's own list. D3: `osaka()`'s "no FP32 build" special case is already
        // stated as data here, so there is no arm for the variant that differs.
        if !self.precisions.contains(&precision) {
            return Err(BuildError::UnpublishedPrecision { codename: self.codename, precision });
        }

        // Unreachable once the check above has passed — each family narrows the precision to the same per-row list
        // the catalogue is built from — and answered rather than asserted, so a row and its model drifting apart is
        // a refusal rather than a panic reachable from whatever sent the request.
        let unpublished = || BuildError::UnpublishedPrecision { codename: self.codename, precision };
        let codename = self.codename;

        let enhancement = match self.family {
            // The one family on the other run path, and the only arm that does not produce an `Operation`.
            Family::Detection => {
                let variant = DetectionVariant::from_codename(codename, precision).ok_or_else(unpublished)?;

                return Ok(Subject::Analysis(match variant {
                    DetectionVariant::NewYork(precision) => Detection::newyork(precision),
                }));
            }
            Family::Denoise => match DenoiseVariant::from_codename(codename, precision).ok_or_else(unpublished)? {
                DenoiseVariant::Stockholm(precision) => Denoise::stockholm(precision, values.strength),
                DenoiseVariant::Gothenburg(precision) => Denoise::gothenburg(precision, values.strength),
                DenoiseVariant::Malmo(precision) => Denoise::malmo(precision, values.strength),
            },
            Family::Sharpen => match SharpenVariant::from_codename(codename, precision).ok_or_else(unpublished)? {
                SharpenVariant::Moscow(precision) => Sharpen::moscow(precision, values.strength),
                SharpenVariant::Petersburg(precision) => Sharpen::stpetersburg(precision, values.strength),
                SharpenVariant::Novgorod(precision) => Sharpen::novgorod(precision, values.strength),
            },
            Family::LightAdjustment => {
                match LightAdjustmentVariant::from_codename(codename, precision).ok_or_else(unpublished)? {
                    LightAdjustmentVariant::Paris(precision) => LightAdjustment::paris(precision, values.bias),
                    LightAdjustmentVariant::Lyon(precision) => LightAdjustment::lyon(precision, values.bias),
                }
            }
            Family::ColorBalance => {
                match ColorBalanceVariant::from_codename(codename, precision).ok_or_else(unpublished)? {
                    ColorBalanceVariant::Rio(precision) => ColorBalance::rio(precision, values.bias),
                    ColorBalanceVariant::SaoPaulo(precision) => ColorBalance::saopaulo(precision, values.bias),
                }
            }
            Family::Colorization => {
                match ColorizationVariant::from_codename(codename, precision).ok_or_else(unpublished)? {
                    ColorizationVariant::Delhi(precision) => Colorization::delhi(precision),
                    ColorizationVariant::Mumbai(precision) => Colorization::mumbai(precision),
                    ColorizationVariant::Jaipur(precision) => Colorization::jaipur(precision),
                }
            }
            // The family the per-variant parameters exist for: Athens carries the fidelity its row publishes and
            // Santorini has nowhere to put one, so the value is read by one arm and not by the other.
            Family::FaceRecovery => {
                match FaceRecoveryVariant::from_codename(codename, precision).ok_or_else(unpublished)? {
                    FaceRecoveryVariant::Athens(precision) => {
                        FaceRecovery::athens(precision, values.faces.clone(), values.fidelity)
                    }
                    FaceRecoveryVariant::Santorini(precision) => {
                        FaceRecovery::santorini(precision, values.faces.clone())
                    }
                }
            }
            Family::Upscale => match UpscaleVariant::from_codename(codename, precision).ok_or_else(unpublished)? {
                UpscaleVariant::Tokyo(precision) => Upscale::tokyo(precision, values.scale),
                UpscaleVariant::Kyoto(precision) => Upscale::kyoto(precision, values.scale),
                UpscaleVariant::Saitama(precision) => Upscale::saitama(precision, values.scale),
                UpscaleVariant::Osaka(precision) => Upscale::osaka(precision, values.scale),
            },
        };

        Ok(Subject::Enhancement(enhancement))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::catalogue::FamilyEntry;
    use crate::models::precision::FloatPrecision;
    use crate::models::test_support::published_values as values_for;

    fn family_entry(family: Family) -> &'static FamilyEntry {
        catalogue()
            .iter()
            .find(|entry| entry.family == family)
            .unwrap_or_else(|| panic!("{family:?} is published"))
    }

    fn upscale(codename: &str) -> &'static VariantEntry {
        VariantEntry::published(Family::Upscale, codename).expect("upscale publishes this variant")
    }

    #[test]
    fn a_published_codename_names_the_row_that_publishes_it() {
        let kyoto = VariantEntry::published(Family::Upscale, "kyoto").expect("upscale publishes Kyoto");

        assert_eq!(kyoto.family, Family::Upscale);
        assert_eq!((kyoto.codename, kyoto.label), ("kyoto", "Kyoto"));
        // The same row the catalogue lists, not a copy of it: a front end that found the variant by walking the
        // catalogue and one that looked it up here are holding the same `&'static`.
        assert!(std::ptr::eq(kyoto, &family_entry(Family::Upscale).variants[1]));
    }

    #[test]
    fn a_codename_no_family_publishes_is_refused_by_name() {
        let refused = VariantEntry::published(Family::Upscale, "berlin").expect_err("Berlin is not a model");

        assert_eq!(refused, BuildError::UnknownModel { family: Family::Upscale, codename: "berlin".to_string() });
        assert_eq!(refused.to_string(), "Upscale publishes no model called `berlin`");
    }

    #[test]
    fn a_codename_another_family_publishes_is_refused_by_the_family_it_was_asked_of() {
        // The pairing is what is looked up, not the name. Stockholm is a real model and is not an upscale one, so
        // asking upscale for it is as wrong as asking for a codename nobody publishes — and the refusal says which
        // family was asked.
        let refused = VariantEntry::published(Family::Upscale, "stockholm").expect_err("Stockholm denoises");

        assert_eq!(refused, BuildError::UnknownModel { family: Family::Upscale, codename: "stockholm".to_string() });
        assert!(VariantEntry::published(Family::Denoise, "stockholm").is_ok(), "the family that does publish it");
    }

    #[test]
    fn every_upscale_variant_builds_at_every_precision_it_publishes() {
        // Driven from the catalogue rather than from a list of the four models, which is the property this seam
        // exists for: a fifth upscale model is covered here by being published.
        for variant in &family_entry(Family::Upscale).variants {
            for &precision in variant.precisions {
                let built = variant
                    .build(precision, &values_for(variant))
                    .unwrap_or_else(|error| panic!("{} at {precision}: {error}", variant.codename));

                assert_eq!(built.family(), Family::Upscale, "{} built another family", variant.codename);
                assert_eq!(built.precision(), precision, "{} built another precision", variant.codename);
                assert!(
                    built.required_artifacts().iter().all(|artifact| artifact.as_str().contains(variant.codename)),
                    "{} at {precision} resolved to another variant's files",
                    variant.codename
                );
            }
        }
    }

    #[test]
    fn the_scale_a_caller_supplies_is_the_scale_the_operation_carries() {
        let built = upscale("kyoto")
            .build(Precision::Fp32, &ParameterValues::new().with_scale(Scale::clamped(4.0)))
            .expect("Kyoto is published at FP32");

        assert_eq!(built, Subject::Enhancement(Upscale::kyoto(FloatPrecision::Fp32, Scale::clamped(4.0))));
    }

    #[test]
    fn osaka_refuses_fp32_through_its_own_published_precisions() {
        // D3: the "no FP32 build" special case is gone. What refuses this is the same `precisions.contains` every
        // other variant is checked by, reading the list the row publishes — so the fact lives in one place.
        let osaka = upscale("osaka");

        assert!(!osaka.precisions.contains(&Precision::Fp32), "the row stopped stating the fact this rests on");
        assert_eq!(
            osaka.build(Precision::Fp32, &values_for(osaka)).expect_err("Osaka has no FP32 build"),
            BuildError::UnpublishedPrecision { codename: "osaka", precision: Precision::Fp32 }
        );
        assert_eq!(
            osaka.build(Precision::Fp32, &values_for(osaka)).expect_err("refused").to_string(),
            "`osaka` is not published at fp32"
        );

        // And the precisions it *is* published at are not refused, so the check is narrow rather than a blanket.
        for precision in [Precision::Fp16, Precision::Int8] {
            assert!(osaka.build(precision, &values_for(osaka)).is_ok(), "Osaka at {precision}");
        }
    }

    #[test]
    fn the_three_convolutional_upscalers_refuse_int8_the_same_way() {
        // The mirror of Osaka's case, and refused by the same line: no arm here tells the two kinds of model apart.
        for codename in ["tokyo", "kyoto", "saitama"] {
            let variant = upscale(codename);

            assert!(!variant.precisions.contains(&Precision::Int8), "{codename} publishes an INT8 build now");
            assert_eq!(
                variant.build(Precision::Int8, &values_for(variant)).expect_err("no INT8 build"),
                BuildError::UnpublishedPrecision { codename: variant.codename, precision: Precision::Int8 }
            );
        }
    }

    #[test]
    fn a_precision_no_variant_of_a_family_publishes_is_refused_before_any_constructor_is_reached() {
        // The two families outside upscale whose INT8 pairing was never published, refused by the same `contains`
        // the two upscale cases above are - which is what says the check is the row's and not a per-family arm.
        for (family, codename) in [(Family::Detection, "newyork"), (Family::Denoise, "stockholm")] {
            let variant = VariantEntry::published(family, codename).expect("the family publishes this variant");

            assert_eq!(
                variant.build(Precision::Int8, &values_for(variant)).expect_err("no INT8 build"),
                BuildError::UnpublishedPrecision { codename: variant.codename, precision: Precision::Int8 }
            );
        }
    }

    #[test]
    fn the_family_whose_result_is_not_an_image_builds_the_carrier_for_its_own_path() {
        // The reason the seam answers with a `Subject`: seven families are carried one way and detection the other,
        // and a caller walking the catalogue crosses both without being asked which path a model runs on.
        let newyork = VariantEntry::published(Family::Detection, "newyork").expect("detection publishes New York");
        let built = newyork.build(Precision::Fp32, &values_for(newyork)).expect("New York is published at FP32");

        assert!(matches!(built, Subject::Analysis(_)), "a detection was carried as an enhancement");
        assert_eq!(built, Detection::newyork(FloatPrecision::Fp32).into());

        let kyoto = upscale("kyoto").build(Precision::Fp32, &values_for(upscale("kyoto"))).expect("published");
        assert!(matches!(kyoto, Subject::Enhancement(_)), "an upscale was carried as an analysis");
    }

    #[test]
    fn a_caller_that_sets_nothing_gets_the_run_that_does_the_least() {
        // The reason the values are total rather than a list one could be missing from: there is no third refusal
        // for a parameter nobody set, and what a caller gets instead is the neutral value rather than an arbitrary
        // one. Fidelity is the documented exception — the reference implementation's hard-coded maximum.
        let neutral = ParameterValues::new();

        assert_eq!(neutral, ParameterValues::default());
        assert_eq!(
            upscale("kyoto").build(Precision::Fp32, &neutral).expect("published"),
            Subject::Enhancement(Upscale::kyoto(FloatPrecision::Fp32, Scale::clamped(Scale::MIN)))
        );

        let athens = VariantEntry::published(Family::FaceRecovery, "athens").expect("published");
        assert_eq!(
            athens.build(Precision::Fp16, &neutral).expect("published"),
            FaceRecovery::athens(FloatPrecision::Fp16, Faces::empty(), Fidelity::MAXIMUM).into()
        );
    }
}
