//! Helpers the model tests share, compiled only under `cfg(test)`.
//!
//! A file directly under `models/` because it is cross-family: every family's tests hash an operation to prove the
//! identity they hand a registry is the one they meant, and a hasher that differed between them would make those
//! results incomparable. The crate already keeps a shared test-only module this way in `deps/test_server.rs`.
//!
//! [`every_operation`] is here for the same reason: what it is used to check cannot be seen from inside one family —
//! a cache-tag collision needs two families to see it, and the set of names the catalogue can produce is the union
//! across all eight. `deps/model` reads it too, to prove the manifest compiled into the binary names nothing this
//! build cannot ask for.

use super::{
    Analysis, Bias, ColorBalance, Colorization, Denoise, Detection, FaceRecovery, Faces, Fidelity, LightAdjustment,
    Operation, ParameterValues, Scale, Sharpen, Strength, Upscale, catalogue, color_balance, colorization, denoise,
    detection, face, light_adjustment, sharpen, upscale,
};

/// The values a front end would hand back for the controls one published row told it to draw.
///
/// Every range the row publishes at its midpoint, and - where the row says a detection run supplies one - a
/// non-empty set of faces, so a selection reaching the operation is visible in what it builds. Everything the row
/// does not publish is left at the neutral value [`ParameterValues::new`] supplies.
///
/// Read off the row rather than chosen per test, so a range that moved is built against its new bounds instead of a
/// literal a test remembered. It names no model and no family: which parameters a variant takes is the row's answer,
/// and which bounded type each published name belongs to is the library's.
pub(crate) fn published_values(variant: &catalogue::VariantEntry) -> ParameterValues {
    use catalogue::ParameterKind;

    let mut values = ParameterValues::new();

    for parameter in variant.parameters {
        values = match parameter.kind {
            ParameterKind::Faces => values.with_faces(Faces::new([face::tests::face_at(12.34, 56.78, 90.12, 34.56)])),
            ParameterKind::Range { min, max } => {
                let midpoint = f64::midpoint(min, max);

                match parameter.name {
                    Scale::NAME => values.with_scale(Scale::clamped(midpoint)),
                    Strength::NAME => values.with_strength(Strength::clamped(midpoint)),
                    Bias::NAME => values.with_bias(Bias::clamped(midpoint)),
                    Fidelity::NAME => values.with_fidelity(Fidelity::clamped(midpoint)),
                    other => panic!("{} publishes a range no value here is supplied for: {other}", variant.codename),
                }
            }
        };
    }

    values
}

/// The hash of `value` under the default hasher.
///
/// What the operation tests compare: two values that are one identity must hash alike, and two that are not must not.
/// The absolute number is never asserted — it is not stable across releases — only whether two of them agree.
pub(crate) fn hash_of(value: &impl std::hash::Hash) -> u64 {
    use std::hash::Hasher as _;

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// The spread of face selections face recovery is built over: none, one, several, and the same several
/// reordered.
///
/// The four shapes whose tags have to stay apart from every other family's — the empty one because its `f0` is
/// the shortest, and the reordered one because it is the pair most easily collapsed into one tag by mistake.
pub(crate) fn every_selection() -> Vec<Faces> {
    let first = face::tests::face_at(12.34, 56.78, 90.12, 34.56);
    let second = face::tests::face_at(200.0, 100.5, 260.25, 170.75);
    let third = face::tests::face_at(0.0, 0.0, 40.0, 40.0);

    vec![
        Faces::empty(),
        Faces::new([first]),
        Faces::new([first, second, third]),
        Faces::new([third, second, first]),
    ]
}

/// Every operation whose result is an image, at a spread of parameter values, carried as [`Operation`].
///
/// Built through the seam rather than as a list of tags and artifact names collected per family: what its callers
/// check holds *across* families, and the value that crosses them is an operation of any family. Each family is
/// still written out once here, because the spread of parameter values is this helper's own choice — what the seam
/// removes is the second listing of what each family reports.
///
/// **Seven families, not eight.** Detection is carried in [`Analysis`] and is [`every_analysis`]'s to spread. A
/// caller that has to reach all eight — a cache-tag collision is only visible in a set holding every family — uses
/// [`every_cache_tag`] or [`every_artifact_name`], which cross both carriers.
pub(crate) fn every_operation() -> Vec<Operation> {
    let strengths = [0.0, 0.5, 1.0, 2.5, 3.0];
    let biases = [-1.0, -0.5, 0.0, 0.5, 1.0];
    let scales = [1.0, 2.0, 2.5, 4.0, 8.0];
    let strength = |value: f64| Strength::new(value).expect("the test supplied a strength in range");
    let bias = |value: f64| Bias::new(value).expect("the test supplied a bias in range");
    let scale = |value: f64| Scale::new(value).expect("the test supplied a scale in range");

    let mut operations = Vec::new();

    for value in strengths {
        for variant in denoise::variant::tests::every_variant() {
            operations.push(Operation::Denoise(Denoise::new(variant, strength(value))));
        }
        for variant in sharpen::variant::tests::every_variant() {
            operations.push(Operation::Sharpen(Sharpen::new(variant, strength(value))));
        }
    }

    for value in biases {
        for variant in light_adjustment::variant::tests::every_variant() {
            operations.push(Operation::LightAdjustment(LightAdjustment::new(variant, bias(value))));
        }
        for variant in color_balance::variant::tests::every_variant() {
            operations.push(Operation::ColorBalance(ColorBalance::new(variant, bias(value))));
        }
    }

    for variant in colorization::variant::tests::every_variant() {
        operations.push(Operation::Colorization(Colorization::new(variant)));
    }

    // Built through the two constructors rather than through the family's own `new`, which is what a caller has:
    // Athens carries a fidelity and Santorini has nowhere to put one. Athens is spread over its fidelities as well
    // as its selections, because the `-w` segment those produce is the newest thing in the shared tag namespace —
    // and Santorini is spread only over selections, since no fidelity can distinguish two of its runs.
    for selection in every_selection() {
        for precision in super::FloatPrecision::ALL {
            operations.push(FaceRecovery::santorini(precision, selection.clone()));

            for value in [0.0, 0.5, 1.0] {
                let fidelity = Fidelity::new(value).expect("the test supplied a fidelity in range");

                operations.push(FaceRecovery::athens(precision, selection.clone(), fidelity));
            }
        }
    }

    for value in scales {
        for variant in upscale::variant::tests::every_variant() {
            operations.push(Operation::Upscale(Upscale::new(variant, scale(value))));
        }
    }

    operations
}

/// Every operation whose result is not an image, carried as [`Analysis`].
///
/// [`every_operation`]'s counterpart on the other run path, and the other half of "all eight families" for the
/// checks that need it. One family and no per-run parameter, so the spread is its published precisions.
pub(crate) fn every_analysis() -> Vec<Analysis> {
    detection::variant::tests::every_variant()
        .into_iter()
        .map(|variant| Analysis::Detection(Detection::new(variant)))
        .collect()
}

/// Every family the library names, reached through whichever carrier holds it.
///
/// What the cross-family checks assert coverage against: a family absent from both spreads fails by name rather than
/// weakening every traversal silently.
pub(crate) fn every_carried_family() -> Vec<super::Family> {
    every_operation()
        .iter()
        .map(Operation::family)
        .chain(every_analysis().iter().map(Analysis::family))
        .collect()
}

/// Every run cache tag the library can report, across both carriers and therefore all eight families.
///
/// The run cache is one namespace shared by every family, so a collision between two of them is only visible in a
/// set that holds both — which is why this crosses the carriers rather than being asked of either.
pub(crate) fn every_cache_tag() -> Vec<String> {
    every_operation()
        .iter()
        .map(Operation::cache_tag)
        .chain(every_analysis().iter().map(Analysis::cache_tag))
        .collect()
}

/// Every artifact name the catalogue can produce, across all eight families.
///
/// The set the manifest compiled into the binary is checked against: an entry naming an artifact absent from this is
/// one no build can ask for, which is either a stale checked-in file or a variant renamed out from under it. The
/// converse is deliberately not checked — a nameable artifact need not be published, and refusing those is the whole
/// point of the resolution's last step.
pub(crate) fn every_artifact_name() -> std::collections::BTreeSet<String> {
    every_operation()
        .iter()
        .flat_map(Operation::required_artifacts)
        .chain(every_analysis().iter().flat_map(Analysis::required_artifacts))
        .map(|id| id.as_str().to_string())
        .collect()
}

/// The artifacts one upscale operation requires, which is how an [`ArtifactId`] is come by at all — there is no
/// constructor a caller reaches for.
///
/// Here rather than in each module that needs one because four of them do: the session cache, the session builder's
/// directory resolution, the model install and the descriptor all key on an artifact, and four spellings of "how you
/// obtain one" is four things to keep agreeing about a type whose whole point is that it cannot be written by hand.
pub(crate) fn artifacts(variant: upscale::UpscaleVariant, scale: f64) -> Vec<super::ArtifactId> {
    Upscale::new(variant, Scale::new(scale).expect("the test supplied a scale in range")).required_artifacts()
}

/// The one artifact `variant` needs at `scale`.
///
/// Asserts rather than silently taking the first, because an operation resolving to two artifacts is not the thing a
/// caller asking for "the" artifact meant.
pub(crate) fn artifact(variant: upscale::UpscaleVariant, scale: f64) -> super::ArtifactId {
    let mut required = artifacts(variant, scale);
    assert_eq!(required.len(), 1, "the test wanted a single-artifact operation");
    required.remove(0)
}

/// The 4x Kyoto graph, `up_kyoto_4x_fp32`.
pub(crate) fn kyoto() -> super::ArtifactId {
    artifact(upscale::UpscaleVariant::Kyoto(super::FloatPrecision::Fp32), 4.0)
}

/// A second, different artifact, for the checks that need two.
pub(crate) fn tokyo() -> super::ArtifactId {
    artifact(upscale::UpscaleVariant::Tokyo(super::FloatPrecision::Fp32), 4.0)
}
