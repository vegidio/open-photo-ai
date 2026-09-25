//! What a front end reads instead of restating the model vocabulary in its own code.
//!
//! In the reference implementation a parameter's bounds are literals in a React component — `min={0} max={300}` —
//! with nothing connecting them to the model code, so a control can offer a value the library refuses. Everything
//! here is built from the same rows and constants that drive resolution, which is what keeps the two from drifting.

use std::sync::LazyLock;

use serde::Serialize;

use super::artifact::Family;
use super::bias::Bias;
use super::fidelity::Fidelity;
use super::precision::{FloatPrecision, Precision};
use super::scale::Scale;
use super::simple::SimpleVariant;
use super::strength::Strength;
use super::upscale;
use super::{color_balance, colorization, denoise, detection, face_recovery, light_adjustment, sharpen};

/// One model a user can choose, and what building it takes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VariantEntry {
    /// Which family publishes this variant.
    ///
    /// Repeated from the [`FamilyEntry`] that lists it, and load-bearing rather than redundant: a variant is what
    /// [`VariantEntry::build`] is called on, and the family is half of what decides which constructor that reaches.
    /// Carried on the row so the pairing cannot be separated from it — a caller holding one variant holds the whole
    /// of what names a model, and there is no second argument to answer wrongly.
    ///
    /// **Not serialized.** It is here for the Rust callers of [`VariantEntry::build`], and a front end never reads
    /// a variant except through the [`FamilyEntry`] that lists it — so putting it on the wire would add a second
    /// place to read a variant's family from, which is the drift this whole seam exists to remove, in the one
    /// language where nothing would catch the two disagreeing. The wire shape is therefore unchanged by this field,
    /// and `frontend/ipc/catalogue.ts` still describes the whole of it.
    #[serde(skip)]
    pub family: Family,
    /// The developer-facing identifier, which is also what a chosen variant is serialized as.
    pub codename: &'static str,
    /// The display text to show. Never composed from — see [`VariantEntry::codename`] for the one that is.
    pub label: &'static str,
    /// Every precision this variant is published in, in the order a chooser should offer them.
    pub precisions: &'static [Precision],
    /// The parameters **this variant** takes.
    ///
    /// Per variant rather than per family, because two variants of one family may take different parameters: Athens
    /// carries a fidelity and Santorini does not, and a control built from a family-wide list would offer one of
    /// them a value its sibling refuses. It is also the more truthful home independently of that — a front end draws
    /// controls for a selected variant, never for a family, so it reads parameters where it reads the variant.
    ///
    /// The seven families whose variants agree repeat one entry across them, which is duplication in the published
    /// JSON and not in the source: these are `&'static` borrows of the same constants.
    pub parameters: &'static [ParameterEntry],
}

/// What kind of value a published parameter is.
///
/// A front end has to tell two cases apart that reporting nothing cannot: a variant with no parameter at all, and one
/// whose parameter is not something a user adjusts. Colorization takes nothing; face recovery **cannot be
/// constructed** without a set of faces, which comes from a detection run. Published as the same empty list, a front
/// end would have to know from somewhere else that choosing Athens means running a detection first — which is the one
/// thing the catalogue exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParameterKind {
    /// A value a user adjusts through a control, bounded by the range that control may offer.
    ///
    /// The bounds are the same constants the parameter's own constructor enforces, so a control built from this
    /// cannot offer a value that constructing one would refuse.
    Range {
        /// The lowest value that will be accepted.
        min: f64,
        /// The highest value that will be accepted.
        max: f64,
    },
    /// A set of faces, supplied by the result of a detection run rather than by a control.
    ///
    /// There is no range to publish and no slider to draw. What it tells a front end is that this variant needs
    /// another operation's output before it can be built at all.
    Faces,
}

/// One parameter a variant takes, and what kind of value it is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ParameterEntry {
    /// The parameter's name, spelled as its error message spells it.
    pub name: &'static str,
    /// What kind of value it is, and its bounds where it has any.
    #[serde(flatten)]
    pub kind: ParameterKind,
}

/// What one family offers: its variants, and what each of them takes.
///
/// Serializable but not deserializable, and deliberately so: the catalogue is what the library publishes about
/// itself, so a front end reads it and nothing ever sends one back. Its names are `&'static str` borrowed straight
/// from the variant rows rather than copies that could be edited into disagreeing with them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FamilyEntry {
    /// Which family this describes.
    pub family: Family,
    /// The variants available, in the order a chooser should offer them, each with the parameters it takes.
    pub variants: Vec<VariantEntry>,
}

/// The strength denoise and sharpen take, from the same constants `Strength::new` enforces.
pub(crate) const STRENGTH_PARAMETER: [ParameterEntry; 1] =
    [ParameterEntry { name: Strength::NAME, kind: ParameterKind::Range { min: Strength::MIN, max: Strength::MAX } }];

/// The bias light adjustment and colour balance take, from the same constants `Bias::new` enforces.
pub(crate) const BIAS_PARAMETER: [ParameterEntry; 1] =
    [ParameterEntry { name: Bias::NAME, kind: ParameterKind::Range { min: Bias::MIN, max: Bias::MAX } }];

/// The scale upscale takes, from the same constants `Scale::new` enforces, so a control built from this cannot offer
/// a scale that constructing one would refuse.
pub(crate) const SCALE_PARAMETER: [ParameterEntry; 1] =
    [ParameterEntry { name: Scale::NAME, kind: ParameterKind::Range { min: Scale::MIN, max: Scale::MAX } }];

/// What Santorini takes: the faces, and nothing a control can set.
pub(crate) const FACES_PARAMETER: [ParameterEntry; 1] =
    [ParameterEntry { name: FACES_PARAMETER_NAME, kind: ParameterKind::Faces }];

/// What Athens takes: the faces, and the fidelity its sibling has nowhere to put — the first per-variant parameter
/// in the library, and the reason `parameters` sits on the variant rather than on the family.
pub(crate) const FACES_AND_FIDELITY_PARAMETERS: [ParameterEntry; 2] = [
    ParameterEntry { name: FACES_PARAMETER_NAME, kind: ParameterKind::Faces },
    ParameterEntry { name: Fidelity::NAME, kind: ParameterKind::Range { min: Fidelity::MIN, max: Fidelity::MAX } },
];

/// How the set of faces a face-recovery run takes is named.
///
/// Spelled here rather than on a bounded type, because unlike every other parameter there is no bounded type to
/// spell it on: a selection is refused by nothing and has no range to publish.
const FACES_PARAMETER_NAME: &str = "faces";

/// Everything the library can be asked to run, as data.
///
/// Built once from the variant rows and the bounds constants, so there is nothing here to keep in step with them by
/// hand and nothing rebuilt per caller — a front end reads this on every redraw.
static CATALOGUE: LazyLock<Vec<FamilyEntry>> = LazyLock::new(|| {
    vec![
        simple_family(Family::Denoise, &denoise::ALL),
        simple_family(Family::Sharpen, &sharpen::ALL),
        simple_family(Family::LightAdjustment, &light_adjustment::ALL),
        simple_family(Family::ColorBalance, &color_balance::ALL),
        // Listed with no parameters rather than omitted: a front end that had to infer "colorization exists but has
        // no control" from the family's absence would be back to knowing something the library did not tell it. And
        // genuinely none — which is now distinguishable from face recovery's, below.
        simple_family(Family::Colorization, &colorization::ALL),
        upscale_family(),
        // Listed although the reference's frontend registry has no `dt` row and its face service hard-codes New York:
        // the catalogue's job is that a front end restates no model vocabulary of its own, and "which model detects
        // faces" is model vocabulary. No chooser gains a row — a family with no parameters is one a front end draws
        // no control for, which is already how colorization is read.
        simple_family(Family::Detection, &detection::ALL),
        simple_family(Family::FaceRecovery, &face_recovery::ALL),
    ]
});

/// What a front end reads instead of restating the model vocabulary in its own code.
pub fn catalogue() -> &'static [FamilyEntry] {
    &CATALOGUE
}

/// The entry for a family whose variants are all published as one graph per precision, at both float precisions.
///
/// One function for seven families rather than seven near-identical ones: what differs between them is the row
/// table, and it is the argument. What each variant takes is read off the row, which is why face recovery — the one
/// family whose variants disagree about that — needs nothing special here. The precision list is not an argument,
/// because all sixteen of these variants admit exactly [`FloatPrecision::ALL`] — a family that did not could not
/// use this.
fn simple_family(family: Family, rows: &[SimpleVariant]) -> FamilyEntry {
    FamilyEntry {
        family,
        variants: rows
            .iter()
            .map(|row| VariantEntry {
                family,
                codename: row.codename,
                label: row.label,
                precisions: &FloatPrecision::PRECISIONS,
                parameters: row.parameters,
            })
            .collect(),
    }
}

/// The upscale family's entry.
///
/// The one family whose variants do not all publish the same precisions, and therefore the one where a list beside
/// the models would be a second place for that fact to live. It asks each model instead — for its parameters as well
/// as its precisions — which is what makes this file name no model at all, so deleting one is its directory and its
/// two arms rather than an edit here.
fn upscale_family() -> FamilyEntry {
    FamilyEntry {
        family: Family::Upscale,
        variants: upscale::MODELS
            .iter()
            .map(|model| VariantEntry {
                family: Family::Upscale,
                codename: model.codename(),
                label: model.label(),
                precisions: model.precisions(),
                parameters: model.parameters(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The shipped inverse, called the way a front end calls it, rather than a listing of all twenty variants kept
    // here: what is exercised is then what a consumer would run, not a second implementation that could agree with
    // the catalogue while the shipped one did not. `published_values` is the caller's half of that - the values a
    // control built from one row would hand back - and is the only thing below that is a fixture.
    use crate::models::build::ParameterValues;
    use crate::models::test_support::published_values;

    /// The families published as one graph per variant per precision, all sharing one shape. Derived from
    /// [`Family::ALL`] rather than transcribed, so a family added later is covered here without being remembered:
    /// upscale is the one left out, because Osaka's precisions are its own.
    fn single_graph_families() -> Vec<Family> {
        Family::ALL.into_iter().filter(|family| *family != Family::Upscale).collect()
    }

    fn family_entry(family: Family) -> &'static FamilyEntry {
        catalogue()
            .iter()
            .find(|entry| entry.family == family)
            .unwrap_or_else(|| panic!("{family:?} is published"))
    }

    fn upscale() -> &'static FamilyEntry {
        family_entry(Family::Upscale)
    }

    fn variant(codename: &str) -> &'static VariantEntry {
        upscale()
            .variants
            .iter()
            .find(|entry| entry.codename == codename)
            .expect("the variant is published")
    }

    /// The codename and label of every variant `family` lists, in the order it lists them.
    fn variants(family: Family) -> Vec<(&'static str, &'static str)> {
        family_entry(family).variants.iter().map(|entry| (entry.codename, entry.label)).collect()
    }

    /// The name and kind of every parameter `variant` of `family` lists.
    fn variant_parameters(family: Family, codename: &str) -> Vec<(&'static str, ParameterKind)> {
        family_entry(family)
            .variants
            .iter()
            .find(|entry| entry.codename == codename)
            .unwrap_or_else(|| panic!("{family:?} publishes {codename}"))
            .parameters
            .iter()
            .map(|entry| (entry.name, entry.kind))
            .collect()
    }

    /// The name and bounds of every parameter `family`'s **first** variant lists.
    ///
    /// The seven families whose variants all take the same parameters are read through this; face recovery, the one
    /// whose two variants disagree, is read per variant through [`variant_parameters`] — which is the whole reason
    /// `parameters` moved onto the variant.
    fn parameters(family: Family) -> Vec<(&'static str, f64, f64)> {
        family_entry(family)
            .variants
            .first()
            .expect("a published family offers a variant")
            .parameters
            .iter()
            .map(|entry| match entry.kind {
                ParameterKind::Range { min, max } => (entry.name, min, max),
                ParameterKind::Faces => panic!("{} is not a range", entry.name),
            })
            .collect()
    }

    /// Every parameter every variant of every family publishes, paired with the family and variant that published it.
    fn every_published_parameter() -> Vec<(Family, &'static str, &'static ParameterEntry)> {
        catalogue()
            .iter()
            .flat_map(|entry| {
                entry.variants.iter().flat_map(move |variant| {
                    variant.parameters.iter().map(move |parameter| (entry.family, variant.codename, parameter))
                })
            })
            .collect()
    }

    #[test]
    fn the_catalogue_lists_the_upscale_variants_and_their_labels() {
        let listed: Vec<(&str, &str)> = upscale().variants.iter().map(|entry| (entry.codename, entry.label)).collect();

        assert_eq!(listed, vec![("tokyo", "Tokyo"), ("kyoto", "Kyoto"), ("saitama", "Saitama"), ("osaka", "Osaka")]);
    }

    #[test]
    fn the_catalogue_reports_each_variants_published_precisions() {
        for codename in ["tokyo", "kyoto", "saitama"] {
            assert_eq!(variant(codename).precisions, vec![Precision::Fp32, Precision::Fp16], "{codename}");
        }

        assert_eq!(variant("osaka").precisions, vec![Precision::Fp16, Precision::Int8]);
    }

    #[test]
    fn the_catalogue_never_offers_osaka_at_fp32() {
        assert!(
            !variant("osaka").precisions.contains(&Precision::Fp32),
            "a chooser could offer an unpublished model"
        );
    }

    #[test]
    fn the_catalogue_reports_the_bounds_every_constructor_enforces() {
        // The property that matters is not the numbers but that they are the same ones: a control built from these
        // bounds cannot offer a value the matching constructor refuses, or stop short of one it accepts. Each arm
        // names the constructor the published range belongs to, so a family whose parameter is published under one
        // name and enforced by another fails here.
        /// A published parameter paired with the constructor that enforces it: name, bounds, and the acceptance test.
        type Enforced = (&'static str, f64, f64, fn(f64) -> bool);

        let accepts: [Enforced; 4] = [
            (Scale::NAME, Scale::MIN, Scale::MAX, |value| Scale::new(value).is_ok()),
            (Strength::NAME, Strength::MIN, Strength::MAX, |value| Strength::new(value).is_ok()),
            (Bias::NAME, Bias::MIN, Bias::MAX, |value| Bias::new(value).is_ok()),
            (Fidelity::NAME, Fidelity::MIN, Fidelity::MAX, |value| Fidelity::new(value).is_ok()),
        ];

        for (family, codename, parameter) in every_published_parameter() {
            // A parameter that is not a control has no range to check, and nothing enforces one: what it publishes
            // is that another operation's output supplies it. See `ParameterKind::Faces`.
            let ParameterKind::Range { min: published_min, max: published_max } = parameter.kind else {
                continue;
            };

            let (_, min, max, accepts) =
                accepts.iter().find(|(name, ..)| *name == parameter.name).unwrap_or_else(|| {
                    panic!("{family:?}'s {codename} publishes a parameter nothing enforces: {}", parameter.name)
                });

            assert_eq!(
                (published_min, published_max),
                (*min, *max),
                "{family:?}'s {codename} publishes {} over a range its constructor does not",
                parameter.name
            );
            assert!(
                accepts(published_min),
                "{family:?}'s {codename} publishes a {} minimum its constructor refuses",
                parameter.name
            );
            assert!(
                accepts(published_max),
                "{family:?}'s {codename} publishes a {} maximum its constructor refuses",
                parameter.name
            );
            assert!(
                !accepts(published_min - 0.001),
                "{family:?}'s {codename} accepts a {} below the range it publishes",
                parameter.name
            );
            assert!(
                !accepts(published_max + 0.001),
                "{family:?}'s {codename} accepts a {} above the range it publishes",
                parameter.name
            );
        }
    }

    /// The same values as [`published_values`], with the one parameter `name` publishes moved off its neutral
    /// value to the far end of the range the library enforces for it.
    ///
    /// Distinct from what [`ParameterValues::new`] supplies in every case, which is what lets "the constructor read
    /// this" be asked as "the operation changed".
    fn detected() -> crate::models::Faces {
        crate::models::test_support::every_selection()
            .into_iter()
            .find(|faces| !faces.is_empty())
            .expect("the spread offers a non-empty selection")
    }

    fn moved(name: &str, values: ParameterValues) -> ParameterValues {
        match name {
            Scale::NAME => values.with_scale(Scale::clamped(Scale::MAX)),
            Strength::NAME => values.with_strength(Strength::clamped(Strength::MAX)),
            Bias::NAME => values.with_bias(Bias::clamped(Bias::MAX)),
            Fidelity::NAME => values.with_fidelity(Fidelity::clamped(Fidelity::MIN)),
            other => panic!("nothing here moves a {other}"),
        }
    }

    #[test]
    fn every_variant_publishes_exactly_the_parameters_its_constructor_reads() {
        // Asked by building the variant twice and comparing: once with every value at the neutral one, and once
        // with a single parameter moved. The operation changes exactly when its constructor read that parameter,
        // because every field is in an operation's identity.
        //
        // Both directions in one traversal. A variant that publishes a range its constructor never reads is a
        // control a front end would draw for nothing, and a variant whose constructor reads a range it does not
        // publish is a control the front end was never told to draw - neither can be seen from one direction alone.
        //
        // Per **variant**, which is what the per-variant parameters are for: Athens reads a fidelity and Santorini
        // has nowhere to put one, so a check per family could only have been right about one of them.
        for entry in catalogue() {
            for variant in &entry.variants {
                let precision = *variant.precisions.first().expect("a published variant offers a precision");
                let neutral = ParameterValues::new();
                let baseline = variant
                    .build(precision, &neutral)
                    .unwrap_or_else(|error| panic!("{} at {precision}: {error}", variant.codename));

                for name in [Scale::NAME, Strength::NAME, Bias::NAME, Fidelity::NAME] {
                    let published = variant.parameters.iter().any(|parameter| parameter.name == name);
                    let built = variant
                        .build(precision, &moved(name, neutral.clone()))
                        .unwrap_or_else(|error| panic!("{} at {precision}: {error}", variant.codename));

                    assert_eq!(
                        built != baseline,
                        published,
                        "{:?}'s {} {} a {name} its constructor {} it",
                        entry.family,
                        variant.codename,
                        if published { "publishes" } else { "does not publish" },
                        if published { "never reads from" } else { "reads from" }
                    );
                }

                // The faces, the one parameter that is not a control. It is exempt from nothing here: a set a
                // detection run supplied is in the operation's identity exactly as a scalar is, so publishing one
                // and reading one are the same two directions asked the same way.
                let published = variant.parameters.iter().any(|parameter| parameter.kind == ParameterKind::Faces);
                let built = variant
                    .build(precision, &neutral.clone().with_faces(detected()))
                    .unwrap_or_else(|error| panic!("{} at {precision}: {error}", variant.codename));

                assert_eq!(
                    built != baseline,
                    published,
                    "{:?}'s {} and the faces it publishes disagree about whether it restores any",
                    entry.family,
                    variant.codename
                );
            }
        }
    }

    #[test]
    fn the_catalogue_reports_the_ranges_the_two_enhancement_parameters_take() {
        assert_eq!(parameters(Family::Denoise), vec![("strength", 0.0, 3.0)]);
        assert_eq!(parameters(Family::Sharpen), vec![("strength", 0.0, 3.0)]);
        assert_eq!(parameters(Family::LightAdjustment), vec![("bias", -1.0, 1.0)]);
        assert_eq!(parameters(Family::ColorBalance), vec![("bias", -1.0, 1.0)]);
        assert_eq!(parameters(Family::Upscale), vec![("scale", 1.0, 8.0)]);
    }

    #[test]
    fn the_catalogue_lists_every_family_the_library_offers() {
        // All eight, with the six published before detection and face recovery in the order they were already
        // published in. That order is pinned by this test and by nothing else — it is not a presentation order, and
        // the reference's own frontend order differs from it — so the two new entries are appended rather than
        // inserted in `Family` declaration order, which would churn a list for no gain.
        let listed: Vec<Family> = catalogue().iter().map(|entry| entry.family).collect();

        assert_eq!(
            listed,
            vec![
                Family::Denoise,
                Family::Sharpen,
                Family::LightAdjustment,
                Family::ColorBalance,
                Family::Colorization,
                Family::Upscale,
                Family::Detection,
                Family::FaceRecovery,
            ]
        );

        // The literal above pins the ORDER, which is a presentation decision. It cannot catch a family that was added
        // to `Family` and never published here, because such a family is absent from both sides. Checking the set
        // against `Family::ALL` is what makes that a failure rather than a silent omission.
        let mut published = listed.clone();
        let mut declared = Family::ALL.to_vec();
        published.sort_unstable_by_key(|family| format!("{family:?}"));
        declared.sort_unstable_by_key(|family| format!("{family:?}"));

        assert_eq!(published, declared, "a family the library names is missing from the catalogue, or listed twice");
    }

    #[test]
    fn the_catalogue_lists_each_enhancement_familys_variants_and_labels() {
        // The labels are the ones the reference application shows, including the three this project's prose spells
        // differently: `Petersburg` rather than "St. Petersburg", and `São Paulo` and `Malmö` with their accents.
        // Each of those three pairs an accented or spaced label with a plain-ASCII codename, which is the split
        // `SimpleVariant` exists to make - only the codename is ever composed into anything.
        assert_eq!(
            variants(Family::Denoise),
            vec![("stockholm", "Stockholm"), ("gothenburg", "Gothenburg"), ("malmo", "Malmö")]
        );
        assert_eq!(
            variants(Family::Sharpen),
            vec![("moscow", "Moscow"), ("petersburg", "Petersburg"), ("novgorod", "Novgorod")]
        );
        assert_eq!(variants(Family::LightAdjustment), vec![("paris", "Paris"), ("lyon", "Lyon")]);
        assert_eq!(variants(Family::ColorBalance), vec![("rio", "Rio"), ("saopaulo", "São Paulo")]);
        assert_eq!(
            variants(Family::Colorization),
            vec![("delhi", "Delhi"), ("mumbai", "Mumbai"), ("jaipur", "Jaipur")]
        );
    }

    #[test]
    fn the_catalogue_names_the_model_that_detects_faces_and_the_two_that_restore_them() {
        // The reason detection is published at all: a front end that pre-detects reads the variant and its precisions
        // from here rather than naming that model itself, which is what the reference's frontend has to do.
        assert_eq!(variants(Family::Detection), vec![("newyork", "New York")]);
        assert_eq!(family_entry(Family::Detection).variants[0].precisions, vec![Precision::Fp32, Precision::Fp16]);

        assert_eq!(variants(Family::FaceRecovery), vec![("athens", "Athens"), ("santorini", "Santorini")]);
    }

    #[test]
    fn the_catalogue_reports_that_detection_takes_no_parameter() {
        // Because a detection run is its variant and precision alone.
        assert!(parameters(Family::Detection).is_empty(), "detection published a parameter it does not take");
    }

    #[test]
    fn face_recovery_and_colorization_are_told_apart() {
        // The gap this change closes. Both entries used to be an empty list, and only colorization's was true:
        // colorization takes nothing, while face recovery **cannot be constructed** without a set of faces. A front
        // end reading two identical entries had to know from somewhere else that choosing Athens means running a
        // detection first — which is the one thing the catalogue exists to prevent.
        for codename in ["delhi", "mumbai", "jaipur"] {
            assert!(
                variant_parameters(Family::Colorization, codename).is_empty(),
                "{codename} published a parameter colorization does not take"
            );
        }

        for codename in ["athens", "santorini"] {
            assert!(
                variant_parameters(Family::FaceRecovery, codename)
                    .iter()
                    .any(|(name, kind)| *name == "faces" && *kind == ParameterKind::Faces),
                "{codename} did not publish the faces it cannot be built without"
            );
        }
    }

    #[test]
    fn two_variants_of_one_family_publish_different_parameters() {
        // The requirement `parameters` moved onto the variant for: Athens carries a fidelity and Santorini has
        // nowhere to put one, so a control drawn from the catalogue is drawn for one and not the other. Bounded 0 to
        // 1, from the same constants `Fidelity::new` enforces.
        assert_eq!(
            variant_parameters(Family::FaceRecovery, "athens"),
            vec![("faces", ParameterKind::Faces), ("fidelity", ParameterKind::Range { min: 0.0, max: 1.0 }),]
        );
        assert_eq!(variant_parameters(Family::FaceRecovery, "santorini"), vec![("faces", ParameterKind::Faces)]);

        assert!(
            !variant_parameters(Family::FaceRecovery, "santorini")
                .iter()
                .any(|(name, _)| *name == Fidelity::NAME),
            "Santorini published a fidelity it has nowhere to put"
        );
    }

    #[test]
    fn a_parameter_that_is_not_a_control_publishes_no_range_and_no_family_publishes_one_it_does_not_enforce() {
        // Every published range is a range some constructor enforces, and the faces are the one parameter that is
        // not a range at all — which is what `ParameterKind` exists to say.
        for (family, codename, parameter) in every_published_parameter() {
            match parameter.kind {
                ParameterKind::Range { min, max } => {
                    assert!(min < max, "{family:?}'s {codename} published an empty range for {}", parameter.name)
                }
                ParameterKind::Faces => assert_eq!(
                    (family, parameter.name),
                    (Family::FaceRecovery, "faces"),
                    "{codename} published a set of faces outside the family that consumes one"
                ),
            }
        }
    }

    #[test]
    fn the_catalogue_offers_every_single_graph_variant_at_both_float_precisions_and_at_no_other() {
        for family in single_graph_families() {
            for entry in &family_entry(family).variants {
                assert_eq!(
                    entry.precisions,
                    [Precision::Fp32, Precision::Fp16],
                    "{family:?}'s {} is not offered at exactly the precisions it is published in",
                    entry.codename
                );
                assert!(
                    !entry.precisions.contains(&Precision::Int8),
                    "a chooser could offer {}'s unpublished INT8 build",
                    entry.codename
                );
            }
        }
    }

    #[test]
    fn the_catalogue_reports_that_colorization_takes_no_parameter() {
        // Listed, with its three variants, and with nothing to configure — so a front end knows to draw no control
        // rather than having to know it itself, and is not left inferring the family from its absence.
        let entry = family_entry(Family::Colorization);

        assert_eq!(entry.variants.len(), 3);
        assert!(
            entry.variants.iter().all(|variant| variant.parameters.is_empty()),
            "colorization published a parameter it does not take"
        );
    }

    #[test]
    fn every_listed_variant_is_one_that_can_actually_be_built() {
        // Reading the catalogue and constructing what it offers has to reach every published pairing of all eight
        // families, which is what makes "a front end needs nothing of its own" true rather than merely intended.
        //
        // A traversal rather than a count of how many rows were reached. The count it replaces was a literal that
        // would be bumped rather than investigated the first time it disagreed; every assertion below names the row
        // it failed on, and the row a family stopped publishing is caught in that family's own tests, where
        // `every_variant` is checked against the rows the catalogue is built from.
        let mut published = std::collections::HashSet::new();

        for entry in catalogue() {
            assert!(!entry.variants.is_empty(), "{:?} is published with nothing to choose", entry.family);

            for variant in &entry.variants {
                assert_eq!(variant.family, entry.family, "{} is filed under another family", variant.codename);
                assert!(!variant.precisions.is_empty(), "{} is listed at no precision", variant.codename);

                for &precision in variant.precisions {
                    let operation = variant.build(precision, &published_values(variant)).unwrap_or_else(|error| {
                        panic!(
                            "{:?}'s {} at {precision} is listed but cannot be built: {error}",
                            entry.family, variant.codename
                        )
                    });

                    // What was built is what was asked for. A row wired to another family's constructor would still
                    // produce an operation, and every artifact it then resolved to would be the wrong model.
                    assert_eq!(operation.family(), entry.family, "{}'s row built another family", variant.codename);
                    assert_eq!(
                        operation.precision(),
                        precision,
                        "{}'s row at {precision} built an operation running at another precision",
                        variant.codename
                    );

                    let artifacts = operation.required_artifacts();
                    assert!(
                        !artifacts.is_empty(),
                        "{:?}'s {} at {precision} is listed but resolves to nothing",
                        entry.family,
                        variant.codename
                    );
                    for artifact in &artifacts {
                        assert!(
                            artifact.as_str().contains(variant.codename),
                            "{}'s row at {precision} resolved to {artifact}, which is another variant's file",
                            variant.codename
                        );
                    }

                    assert!(
                        published.insert(operation.cache_tag()),
                        "{:?}'s {} at {precision} names an operation another row already named",
                        entry.family,
                        variant.codename
                    );
                }
            }
        }

        // Over `Family::ALL` rather than over the entries, so a family the library names and publishes nothing for
        // fails here by name instead of being absent from both sides of the traversal above.
        let listed: std::collections::HashSet<Family> = catalogue().iter().map(|entry| entry.family).collect();
        for family in Family::ALL {
            assert!(listed.contains(&family), "{family:?} is named by the library but has nothing to build from");
        }
    }

    #[test]
    fn the_catalogue_serializes_for_a_front_end_to_read() {
        let json = serde_json::to_string(&catalogue()).expect("the catalogue serializes");

        assert!(json.contains("\"osaka\""), "{json}");
        assert!(json.contains("\"scale\""), "{json}");
    }

    #[test]
    fn a_serialized_variant_carries_exactly_the_fields_the_frontend_mirror_declares() {
        // The mirror is `VariantEntry` in `crates/gui/frontend/ipc/catalogue.ts`, and nothing in either toolchain
        // notices when one of the two gains a field alone: an added key is not a type error on the TypeScript side,
        // it is a key silently ignored. So the wire shape is pinned here, where the field would be added.
        //
        // `family` is the case this was written for. It is on the row for Rust's sake and `#[serde(skip)]`ped, so a
        // front end keeps reading a variant's family from the entry that lists it and has only one place to read it.
        let value = serde_json::to_value(catalogue()).expect("the catalogue serializes");
        let variant = value
            .get(0)
            .and_then(|entry| entry.get("variants"))
            .and_then(|variants| variants.get(0))
            .and_then(serde_json::Value::as_object)
            .expect("the first family publishes a first variant");

        let mut fields: Vec<&str> = variant.keys().map(String::as_str).collect();
        fields.sort_unstable();

        assert_eq!(fields, ["codename", "label", "parameters", "precisions"], "the frontend mirror is now behind");
    }
}
