//! What to measure, and the rendering of `perftest list`.
//!
//! Both run entirely over [`catalogue`] and both run **before** anything is initialized.
//!
//! [`with_faces`] is the one thing here that happens later, once the sweep's own detection pass has run, and it is
//! still offline: it makes no inference call of its own and touches no row of any other family.

// What is deliberately absent is any notion of which models the library will actually run. This binary keeps no list,
// asks for no such thing, and marks nothing in `list`: which operations have a pipeline is the library's own answer
// and changes as it gains them, so a second copy of it here would be wrong the day after the next one lands. A sweep
// finds out by attempting one — see `sweep`.

use std::fmt::Write as _;

use opai::{
    Bias, ColorBalance, ColorBalanceVariant, Colorization, ColorizationVariant, Denoise, DenoiseVariant, FaceRecovery,
    FaceRecoveryVariant, Faces, Family, FamilyEntry, Fidelity, FloatPrecision, LightAdjustment, LightAdjustmentVariant,
    Operation, ParameterEntry, ParameterKind, Precision, Scale, Sharpen, SharpenVariant, Strength, Upscale,
    UpscaleVariant, VariantEntry, catalogue,
};

use crate::cli::{Options, UnknownParameter};
use crate::sweep::Detected;

/// One model a sweep will attempt: the operation to run, and what to call it in the report.
#[derive(Debug, Clone, PartialEq)]
pub struct Selected {
    /// The variant's codename, which is what a row of the report is labelled with and what names it on the command
    /// line.
    pub codename: &'static str,
    /// Which family it belongs to.
    pub family: Family,
    /// The operation to run, built from the catalogue row at the requested precision with the requested parameter.
    pub operation: Operation,
    // Failed rather than measured because a failure is visible and a plausible wrong number is not — see
    // `sweep::Detected` for the number an unblocked row would report.
    /// Why this row cannot be measured, where the auxiliary run that was to supply its input did not.
    ///
    /// `None` for every row of every sweep that made no auxiliary run, which is every sweep that selected no
    /// face-recovery model. A row carrying one is reported as failed **with this reason** rather than measured.
    pub blocked: Option<String>,
}

/// Why a selection could not be resolved.
///
/// Each of these is answerable from the catalogue alone, which is why they are separated from anything a run
/// reports: none of them needs the runtime, the network or a model file to be decided.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SelectionError {
    /// A name no family publishes.
    #[error("unknown model `{name}`; run \"perftest list\" to see the available models")]
    Unknown {
        /// The name as it was given.
        name: String,
    },

    // Reported as the catalogue publishing no such row rather than as a failure of the model, because that is what it
    // is: Osaka publishes `int8` and `fp16` and no `fp32`, and asking for one names a file that was never released.
    // The distinction is what stops somebody debugging a working model.
    /// A codename the catalogue publishes, at a precision that variant is not published at.
    #[error("the catalogue publishes no {codename} at {precision}")]
    Unpublished {
        /// The codename, as the catalogue spells it.
        codename: &'static str,
        /// The precision that was asked for.
        precision: Precision,
    },

    /// The catalogue publishes a parameter this binary has no flag for.
    #[error(transparent)]
    Parameter(#[from] UnknownParameter),

    // This harness measures one entry point — the enhancement chain — and an operation whose result is a set of faces
    // rather than a picture is not carried in what that entry point takes. It is not a model this binary declines to
    // run: it is a run of a different kind, on a path nothing here measures yet. Reported when named for the reason
    // `Unpublished` is: you asked for it by name, so you find out why you are not getting it.
    /// A codename the catalogue publishes for a family whose result is not an image.
    ///
    /// Reported rather than silently skipped when the codename was named explicitly.
    #[error("{codename} produces no image, and this harness measures the enhancement chain")]
    NotAnEnhancement {
        /// The codename, as the catalogue spells it.
        codename: &'static str,
    },
}

/// The models `options` selects.
///
/// With no names, every variant the catalogue publishes, in catalogue order — the ones the library declines are
/// dropped later, by the sweep, from the outcome of attempting them. With names, each one resolved by scanning the
/// catalogue for the codename, which is unambiguous because no codename is published by two families.
///
/// A name given twice is selected once, keeping the first occurrence, so `perftest kyoto kyoto` measures Kyoto once.
///
/// # Errors
///
/// [`SelectionError`], and in every case before anything has been initialized or transferred.
pub fn resolve(options: &Options) -> Result<Vec<Selected>, SelectionError> {
    if options.models.is_empty() {
        return every_published(options);
    }

    let mut selection: Vec<Selected> = Vec::with_capacity(options.models.len());

    for name in &options.models {
        let name = name.trim().to_lowercase();

        if selection.iter().any(|selected| selected.codename == name) {
            continue;
        }

        selection.push(named(&name, options)?);
    }

    Ok(selection)
}

/// Every variant the catalogue publishes, in catalogue order.
///
/// A variant the requested precision is not published at is **dropped** rather than failing the sweep, and so is a
/// family whose result is not an image.
fn every_published(options: &Options) -> Result<Vec<Selected>, SelectionError> {
    let precision = options.precision();
    let mut selection = Vec::new();

    for entry in catalogue() {
        // Dropped for the same reason and on the same terms as an unpublished precision below: this harness measures
        // the enhancement chain, a caller who named nothing did not ask for a detection run, and naming one explicitly
        // reports `SelectionError::NotAnEnhancement`.
        if entry.family == Family::Detection {
            continue;
        }

        for variant in &entry.variants {
            // The caller named nothing, so it asked for whatever this precision covers, and a `--precision int8`
            // sweep would otherwise fail on the first of the twenty variants that publish only the two float
            // precisions. A variant named explicitly is a different question and reports
            // `SelectionError::Unpublished`, which is how you find out.
            if !variant.precisions.contains(&precision) {
                continue;
            }

            selection.push(select(entry, variant, options)?);
        }
    }

    Ok(selection)
}

/// The model one codename names, whatever the library later makes of it.
fn named(name: &str, options: &Options) -> Result<Selected, SelectionError> {
    for entry in catalogue() {
        let Some(variant) = entry.variants.iter().find(|variant| variant.codename == name) else {
            continue;
        };

        return select(entry, variant, options);
    }

    Err(SelectionError::Unknown { name: name.to_string() })
}

/// One catalogue row as a model to measure.
fn select(entry: &FamilyEntry, variant: &VariantEntry, options: &Options) -> Result<Selected, SelectionError> {
    // The mapping from a published variant to a `Selected`, which both ways of choosing one arrive at: a family that
    // grows a second parameter, or a change to how `SelectionError::Unpublished` is decided, lands here rather than in
    // two places that have to be found.
    let precision = options.precision();

    if entry.family == Family::Detection {
        return Err(SelectionError::NotAnEnhancement { codename: variant.codename });
    }

    let operation = build(entry.family, variant.codename, precision, variant.parameters, options)?
        .ok_or(SelectionError::Unpublished { codename: variant.codename, precision })?;

    Ok(Selected { codename: variant.codename, family: entry.family, operation, blocked: None })
}

/// Whether any row of `selection` needs a result only another model can produce.
///
/// Face recovery, and nothing else: it restores the faces it is given and finds none of its own.
pub fn needs_faces(selection: &[Selected]) -> bool {
    // Decided on the **operation**, which is the same thing `with_faces` rebuilds and so cannot disagree with it: a
    // row this reports and that one could not rebuild would be measured over an empty selection, which is the whole
    // defect the pass exists to prevent. The catalogue states the same fact independently, as a published parameter of
    // kind `ParameterKind::Faces`, and a test below holds the two to each other.
    selection.iter().any(|selected| matches!(selected.operation, Operation::FaceRecovery(_)))
}

/// The precision the auxiliary detection run is made at: **the sweep's own**, read off the row that needs the faces.
///
/// `None` where no row needs faces, which is the same condition [`needs_faces`] answers.
pub fn detection_precision(selection: &[Selected]) -> Option<FloatPrecision> {
    // Rather than pinned: a run that restores at FP16 detects at FP16, which is what the application does, and pinning
    // would measure a tier combination it never produces. Every row of a sweep carries the one precision the flag
    // named, so the first is the answer.
    selection.iter().find_map(|selected| match &selected.operation {
        Operation::FaceRecovery(recovery) => Some(match recovery.variant() {
            FaceRecoveryVariant::Athens(precision) | FaceRecoveryVariant::Santorini(precision) => precision,
        }),
        _ => None,
    })
}

/// `selection` with every face-recovery row rebuilt over `faces`, or blocked with the reason there are none.
///
/// The rebuild goes through the family's **own constructors**, carrying the values the row was already selected
/// with — the variant, its precision and its fidelity — so a row measures the operation a caller would have built,
/// and nothing here reaches inside the operation to swap a field. That is the reference harness's `faceEntry`, using
/// only the public API.
///
/// Where the pass found nothing to work with, the rows it was made for are **blocked rather than rebuilt**: a sweep
/// of a photograph with no face in it has nothing to say about a face-recovery model, and saying it quickly is the
/// defect this exists to prevent. Every other row is returned exactly as it arrived — neither delayed by the pass
/// nor failed by its failure.
pub fn with_faces(selection: Vec<Selected>, detected: &Detected) -> Vec<Selected> {
    selection
        .into_iter()
        .map(|selected| {
            let Operation::FaceRecovery(recovery) = &selected.operation else {
                return selected;
            };

            match detected.usable() {
                Err(reason) => Selected { blocked: Some(reason), ..selected },
                Ok(faces) => {
                    let operation = match recovery.variant() {
                        // The fidelity the row was selected with, and `Fidelity::MAX` where the operation carries
                        // none — which is what the library itself binds for one that reaches a pipeline empty.
                        FaceRecoveryVariant::Athens(precision) => {
                            let fidelity = recovery.fidelity().unwrap_or(Fidelity::MAXIMUM);

                            FaceRecovery::athens(precision, faces.clone(), fidelity)
                        }
                        FaceRecoveryVariant::Santorini(precision) => FaceRecovery::santorini(precision, faces.clone()),
                    };

                    Selected { operation, ..selected }
                }
            }
        })
        .collect()
}

/// Every published parameter this variant takes, paired with the value the flag named after it holds.
///
/// Read off the **variant**'s published list rather than the family's, because two variants of one family may take
/// different parameters — Athens publishes a fidelity and Santorini does not, so a value read per family could only
/// have been right about one of them.
///
/// The whole list is asked, not just the entries the arm below reads, so that a parameter the catalogue publishes
/// and this binary cannot supply is a loud failure rather than something passed over because the arm that would have
/// used it did not ask.
///
/// Returning what it resolved is what lets [`parameter`] be infallible: the only thing that can produce a
/// [`SelectionError::Parameter`] is this call, and an arm cannot reach a value without holding its answer, so the
/// ordering is the type rather than a sentence asking the next reader to preserve it.
fn checked<'a>(parameters: &'a [ParameterEntry], options: &Options) -> Result<Resolved<'a>, SelectionError> {
    parameters
        .iter()
        .map(|published| Ok((published.name, options.parameter(published).map_err(SelectionError::from)?)))
        .collect()
}

/// What [`checked`] resolved: each published name against the value the flags hold for it, `None` where they hold
/// none. A list rather than a map because a variant publishes at most a handful.
type Resolved<'a> = Vec<(&'a str, Option<f64>)>;

/// The resolved value published under `name`, or `None` where this variant publishes none.
fn parameter(resolved: &Resolved<'_>, name: &str) -> Option<f64> {
    resolved.iter().find(|(published, _)| *published == name).and_then(|(_, value)| *value)
}

/// The operation a catalogue row names, built through the model's own constructor.
///
/// The library's inverse is each family's `from_codename`, which resolves a codename and a precision to that
/// family's variant; what follows is a match over that variant calling the model's constructor with the values the
/// row published. One arm per model, which is what a consumer of this library writes — there is no
/// `Operation::from_catalogue` to route around it any more, and nothing left for a second entry point to be the
/// other of.
///
/// `Ok(None)` where the pairing names nothing: a codename the family does not publish, or a precision it is not
/// published at.
///
/// **Face recovery is built over an empty selection here, and does not stay that way.** The catalogue publishes its
/// faces as a parameter whose kind says another operation's result supplies them, and no command line can hold a set
/// of detected boxes — and this runs before the runtime exists, so there is nothing to detect with yet. The sweep
/// makes one detection pass once the picture is loaded and [`with_faces`] rebuilds these rows over what it found. A
/// selection reaches the operation through [`FaceRecovery::athens`] exactly as a caller's would.
///
/// # Errors
///
/// [`SelectionError::Parameter`] where the row publishes a range this binary has no flag for.
fn build(
    family: Family,
    codename: &str,
    precision: Precision,
    parameters: &[ParameterEntry],
    options: &Options,
) -> Result<Option<Operation>, SelectionError> {
    let resolved = checked(parameters, options)?;

    let strength = || parameter(&resolved, Strength::NAME).and_then(|value| Strength::new(value).ok());
    let bias = || parameter(&resolved, Bias::NAME).and_then(|value| Bias::new(value).ok());

    let built = match family {
        // Its result is a set of faces rather than a picture, so it is not carried in what this harness runs — see
        // `SelectionError::NotAnEnhancement`, which is reported before this is reached.
        Family::Detection => None,
        Family::Denoise => {
            let Some(strength) = strength() else { return Ok(None) };

            DenoiseVariant::from_codename(codename, precision)
                .map(|variant| Operation::Denoise(Denoise::new(variant, strength)))
        }
        Family::Sharpen => {
            let Some(strength) = strength() else { return Ok(None) };

            SharpenVariant::from_codename(codename, precision)
                .map(|variant| Operation::Sharpen(Sharpen::new(variant, strength)))
        }
        Family::LightAdjustment => {
            let Some(bias) = bias() else { return Ok(None) };

            LightAdjustmentVariant::from_codename(codename, precision)
                .map(|variant| Operation::LightAdjustment(LightAdjustment::new(variant, bias)))
        }
        Family::ColorBalance => {
            let Some(bias) = bias() else { return Ok(None) };

            ColorBalanceVariant::from_codename(codename, precision)
                .map(|variant| Operation::ColorBalance(ColorBalance::new(variant, bias)))
        }
        Family::Colorization => ColorizationVariant::from_codename(codename, precision)
            .map(|variant| Operation::Colorization(Colorization::new(variant))),
        Family::FaceRecovery => {
            let Some(variant) = FaceRecoveryVariant::from_codename(codename, precision) else {
                return Ok(None);
            };

            match variant {
                FaceRecoveryVariant::Athens(precision) => {
                    let Some(value) = parameter(&resolved, Fidelity::NAME) else { return Ok(None) };
                    let Ok(fidelity) = Fidelity::new(value) else { return Ok(None) };

                    Some(FaceRecovery::athens(precision, Faces::empty(), fidelity))
                }
                FaceRecoveryVariant::Santorini(precision) => Some(FaceRecovery::santorini(precision, Faces::empty())),
            }
        }
        Family::Upscale => {
            let Some(value) = parameter(&resolved, Scale::NAME) else { return Ok(None) };
            let Ok(scale) = Scale::new(value) else { return Ok(None) };

            UpscaleVariant::from_codename(codename, precision)
                .map(|variant| Operation::Upscale(Upscale::new(variant, scale)))
        }
    };

    Ok(built)
}

/// The catalogue as `perftest list` prints it.
///
/// Unannotated, deliberately: every row is what the library publishes, and nothing here says whether a given row can
/// be run today. Finding that out means naming one and reading what comes back, which is the one place that answer
/// exists.
pub fn listing() -> String {
    let mut out = String::new();

    for entry in catalogue() {
        let _ = writeln!(out, "{} ({})", entry.family.label(), entry.family.prefix());

        for variant in &entry.variants {
            let precisions: Vec<&str> = variant.precisions.iter().map(|precision| precision.as_str()).collect();

            // Per variant, because that is where the catalogue publishes them: two variants of one family may take
            // different parameters, and a note printed per family could only have been right about one of them. A
            // parameter whose kind is not a range prints its name alone — there is no flag to offer, because another
            // operation's result supplies it.
            let parameters: Vec<String> = variant
                .parameters
                .iter()
                .map(|parameter| match parameter.kind {
                    ParameterKind::Range { min, max } => format!("--{} {min}..{max}", parameter.name),
                    ParameterKind::Faces => format!("{} (from a detection run)", parameter.name),
                })
                .collect();
            let note = if parameters.is_empty() { "no parameter".to_string() } else { parameters.join(", ") };

            let _ =
                writeln!(out, "  {:<12} {:<16} {:<12} {note}", variant.codename, variant.label, precisions.join(", "));
        }

        let _ = writeln!(out);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    /// A face at a plausible box, with landmarks a detector could have reported.
    fn face(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> opai::Face {
        use opai::{Confidence, Point, Rect};

        opai::Face::new(
            Rect::new(Point::new(min_x, min_y), Point::new(max_x, max_y)),
            [
                Point::new(min_x + 10.0, min_y + 10.0),
                Point::new(max_x - 10.0, min_y + 10.0),
                Point::new((min_x + max_x) / 2.0, (min_y + max_y) / 2.0),
                Point::new(min_x + 12.0, max_y - 10.0),
                Point::new(max_x - 12.0, max_y - 10.0),
            ],
            Confidence::new(0.9).expect("0.9 is in range"),
        )
    }

    /// Two faces, as one detection pass would report them.
    fn two_faces() -> Faces {
        Faces::new([face(10.0, 20.0, 90.0, 110.0), face(200.0, 40.0, 280.0, 130.0)])
    }

    #[test]
    fn a_sweep_selecting_no_face_recovery_row_needs_no_auxiliary_run() {
        // So a sweep of upscalers neither runs one nor can be failed by one. Answered off the selection rather than
        // off a flag, because it is a property of what was selected.
        let upscalers = resolve(&options(&["kyoto", "tokyo"])).expect("two upscalers resolve");
        assert!(!needs_faces(&upscalers), "a sweep of upscalers wanted a detection pass");
        assert_eq!(detection_precision(&upscalers), None);

        let mixed = resolve(&options(&["kyoto", "athens"])).expect("an upscaler and a restorer resolve");
        assert!(needs_faces(&mixed), "a sweep including a restorer made no detection pass");
    }

    #[test]
    fn the_rows_that_need_an_auxiliary_run_are_the_ones_the_catalogue_says_need_one() {
        // Two independent statements of the same fact, held to each other: this binary decides on the operation,
        // because that is what it has to rebuild, and the library declares it as a published parameter whose kind
        // says another operation's result supplies it. A family published with such a parameter and no rebuild here
        // would be measured over an empty selection — a sub-millisecond success for a model that does nothing.
        for entry in catalogue() {
            for variant in &entry.variants {
                let published = variant.parameters.iter().any(|parameter| parameter.kind == ParameterKind::Faces);

                // Detection is not an enhancement and never reaches a selection at all, so there is no row of it to
                // compare against.
                if entry.family == Family::Detection {
                    continue;
                }

                let Ok(selection) = resolve(&options(&[variant.codename])) else {
                    continue;
                };

                assert_eq!(
                    needs_faces(&selection),
                    published,
                    "{} and the catalogue disagree about whether it needs another model's result",
                    variant.codename
                );
            }
        }
    }

    #[test]
    fn the_auxiliary_run_is_made_at_the_precision_the_rows_were_selected_at() {
        // A run that restores at FP16 detects at FP16, which is what the application does. Pinning it would
        // benchmark a tier combination the application never produces.
        for (flag, expected) in [("fp32", FloatPrecision::Fp32), ("fp16", FloatPrecision::Fp16)] {
            let selection = resolve(&options(&["athens", "--precision", flag])).expect("Athens resolves");

            assert_eq!(detection_precision(&selection), Some(expected), "the pass would be made at {flag}");
        }
    }

    #[test]
    fn two_face_recovery_rows_are_rebuilt_over_one_passs_faces() {
        // Obtained once and supplied to both, which is what keeps a second detection session build out of a second
        // model's measurement: every model boundary drains the session cache, so a pass per model would pay for a
        // full detection build inside each row.
        let selection = resolve(&options(&["athens", "santorini"])).expect("both restorers resolve");
        assert_eq!(selection.len(), 2);

        let faces = two_faces();
        let rebuilt = with_faces(selection, &Detected::Found(faces.clone()));

        for row in &rebuilt {
            let Operation::FaceRecovery(recovery) = &row.operation else {
                panic!("{} is not a face recovery row", row.codename);
            };

            assert_eq!(recovery.faces(), &faces, "{} was not rebuilt over the pass's faces", row.codename);
            assert_eq!(row.blocked, None, "{} was blocked although the pass found faces", row.codename);
        }
    }

    #[test]
    fn a_rebuilt_row_keeps_the_variant_precision_and_fidelity_it_was_selected_with() {
        // The faces are the only thing the pass supplies. A rebuild that lost the fidelity would measure a
        // restoration the operator did not ask for, and would report it under the flag they did.
        let selection =
            resolve(&options(&["athens", "--precision", "fp16", "--fidelity", "0.25"])).expect("Athens resolves");

        let rebuilt = with_faces(selection, &Detected::Found(two_faces()));
        let Operation::FaceRecovery(recovery) = &rebuilt[0].operation else {
            panic!("the rebuilt row is not a face recovery operation");
        };

        assert_eq!(recovery.variant(), FaceRecoveryVariant::Athens(FloatPrecision::Fp16));
        assert_eq!(recovery.fidelity().map(Fidelity::get), Some(0.25));
        assert_eq!(recovery.faces(), &two_faces());

        // And Santorini, which publishes no fidelity, is rebuilt with none rather than with one invented for it.
        let santorini = resolve(&options(&["santorini", "--precision", "fp16"])).expect("Santorini resolves");
        let rebuilt = with_faces(santorini, &Detected::Found(two_faces()));
        let Operation::FaceRecovery(recovery) = &rebuilt[0].operation else {
            panic!("the rebuilt row is not a face recovery operation");
        };

        assert_eq!(recovery.variant(), FaceRecoveryVariant::Santorini(FloatPrecision::Fp16));
        assert_eq!(recovery.fidelity(), None);
    }

    #[test]
    fn a_pass_that_supplied_nothing_blocks_exactly_the_rows_that_needed_it() {
        // Failing is the required outcome rather than measuring the empty case: a face-recovery run over no face
        // composites nothing and returns in under a millisecond, which a sweep would report as a success. Every
        // other row is unaffected — neither delayed by the pass nor failed by its failure — so the sweep carries on
        // and reports the rest.
        let selection =
            resolve(&options(&["kyoto", "athens", "tokyo", "santorini"])).expect("two upscalers and two restorers");

        for outcome in [Detected::Empty, Detected::Failed { reason: "the graph would not load".to_string() }] {
            let rebuilt = with_faces(selection.clone(), &outcome);

            for row in &rebuilt {
                if row.family == Family::FaceRecovery {
                    let reason = row.blocked.as_deref().unwrap_or_else(|| panic!("{} was not blocked", row.codename));

                    // The pass's own reason rather than a generic failure, so the row says why it has no number.
                    assert_eq!(Some(reason), outcome.usable().err().as_deref());
                    continue;
                }

                assert_eq!(row.blocked, None, "{} was blocked by a pass it did not need", row.codename);
                // And untouched, not merely unblocked: an upscaler's operation is the one that was selected.
                let original = selection.iter().find(|other| other.codename == row.codename).expect("the same row");
                assert_eq!(&row.operation, &original.operation, "{} was rebuilt", row.codename);
            }
        }
    }

    #[test]
    fn a_selection_with_no_row_to_rebuild_is_returned_as_it_arrived() {
        // The pass is never made for such a sweep, so this is the shape the code has rather than a case it meets —
        // and it is checked because `with_faces` walking every row is what makes "every other row is unaffected" a
        // property of the function rather than of its caller.
        let selection = resolve(&options(&["kyoto", "tokyo"])).expect("two upscalers resolve");

        assert_eq!(with_faces(selection.clone(), &Detected::Found(two_faces())), selection);
        assert_eq!(with_faces(selection.clone(), &Detected::Empty), selection);
    }

    /// The options one command line resolves to.
    fn options(arguments: &[&str]) -> Options {
        Cli::try_parse_from(std::iter::once("perftest").chain(arguments.iter().copied()))
            .expect("the arguments parse")
            .options
    }

    /// The selection one command line resolves to.
    fn selection(arguments: &[&str]) -> Vec<Selected> {
        resolve(&options(arguments)).expect("the selection resolves")
    }

    /// Every codename the catalogue publishes at `precision` whose result is an image, in catalogue order —
    /// computed here from the catalogue so the assertion cannot pass by agreeing with the resolution's own traversal.
    ///
    /// Detection is excluded because its result is not a picture, so it is not carried in what this harness's one
    /// entry point takes. That is a property of the *path*, decided by the library's type, and not the "which models
    /// will actually run" this module keeps no opinion about.
    fn published_at(precision: Precision) -> Vec<&'static str> {
        catalogue()
            .iter()
            .filter(|entry| entry.family != Family::Detection)
            .flat_map(|entry| &entry.variants)
            .filter(|variant| variant.precisions.contains(&precision))
            .map(|variant| variant.codename)
            .collect()
    }

    #[test]
    fn no_names_selects_every_published_variant_at_the_requested_precision_in_catalogue_order() {
        let selected: Vec<&str> = selection(&[]).iter().map(|selected| selected.codename).collect();

        assert_eq!(selected, published_at(Precision::Fp32));
        // Not one model: the resolution knows nothing about which of them the library will actually run, and drops
        // nothing on that account.
        assert!(selected.len() > 1, "{selected:?}");
        // And face recovery is in it although its pipeline is unwritten, which is what "no opinion about what will
        // run" means: the sweep finds that out by attempting one.
        assert!(selected.contains(&"athens"), "{selected:?}");
    }

    #[test]
    fn a_family_whose_result_is_not_an_image_is_reported_when_it_is_named() {
        // Dropped from an unnamed sweep and reported when asked for by name, which is how `Unpublished` behaves and
        // for the same reason: you named it, so you find out why you are not getting it.
        let error = resolve(&options(&["newyork"])).expect_err("detection is not an enhancement");

        assert_eq!(error, SelectionError::NotAnEnhancement { codename: "newyork" });
        assert!(error.to_string().contains("produces no image"), "{error}");
    }

    #[test]
    fn a_precision_only_some_variants_publish_selects_only_those() {
        let selected: Vec<&str> = selection(&["--precision", "int8"]).iter().map(|s| s.codename).collect();

        // Osaka alone publishes int8, so an unnamed int8 sweep is Osaka — the other twenty are not failures, they
        // are rows the catalogue does not publish at this precision.
        assert_eq!(selected, published_at(Precision::Int8));
        assert_eq!(selected, vec!["osaka"]);
    }

    #[test]
    fn every_selected_operation_carries_the_requested_precision_and_parameter() {
        for selected in selection(&["--precision", "fp16", "--scale", "2"]) {
            assert_eq!(selected.operation.precision(), Precision::Fp16, "{}", selected.codename);
        }

        let kyoto = selection(&["kyoto", "--scale", "2"]).pop().expect("kyoto resolves");
        // The scale reaches the operation, which is what the display name is composed from.
        assert!(kyoto.operation.display_name().contains('2'), "{}", kyoto.operation.display_name());
    }

    #[test]
    fn a_name_no_family_publishes_fails_naming_it_and_pointing_at_the_listing() {
        let error = resolve(&options(&["kyot0"])).expect_err("an unknown name is refused");

        assert_eq!(error, SelectionError::Unknown { name: "kyot0".to_string() });

        let message = error.to_string();
        assert!(message.contains("kyot0"), "{message}");
        assert!(message.contains("perftest list"), "{message}");
    }

    #[test]
    fn a_name_is_resolved_without_a_family_qualifier_and_case_and_space_insensitively() {
        // No codename is published by two families, which is what makes the bare name unambiguous — asserted here
        // rather than assumed.
        let mut seen: Vec<&str> = catalogue().iter().flat_map(|entry| &entry.variants).map(|v| v.codename).collect();
        let published = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), published, "a codename is published by two families");

        assert_eq!(selection(&["  KYOTO "]).first().map(|s| s.codename), Some("kyoto"));
    }

    #[test]
    fn a_name_given_twice_is_selected_once() {
        let selected: Vec<&str> = selection(&["kyoto", "kyoto", "stockholm"]).iter().map(|s| s.codename).collect();

        assert_eq!(selected, vec!["kyoto", "stockholm"]);
    }

    #[test]
    fn a_codename_at_a_precision_its_variant_does_not_publish_is_the_catalogue_publishing_no_such_row() {
        // Osaka publishes int8 and fp16 and no fp32. Naming it at fp32 is not a failure of the model — nothing was
        // ever released under that name — and the distinction is what stops somebody debugging a working model.
        let error = resolve(&options(&["osaka", "--precision", "fp32"])).expect_err("fp32 osaka is not published");

        assert_eq!(error, SelectionError::Unpublished { codename: "osaka", precision: Precision::Fp32 });
        assert_eq!(error.to_string(), "the catalogue publishes no osaka at fp32");
    }

    #[test]
    fn the_parameter_reaching_an_operation_is_the_one_the_published_name_selects() {
        // Denoise publishes `strength` and upscale publishes `scale`, so the two flags reach different families and
        // neither reads the other's — which is the whole reason there are three flags rather than one intensity.
        let selected = selection(&["stockholm", "kyoto", "--strength", "2", "--scale", "8"]);

        let named = |codename: &str| {
            selected.iter().find(|s| s.codename == codename).expect("selected").operation.display_name()
        };

        assert!(named("kyoto").contains('8'), "{}", named("kyoto"));
        // Built through `from_catalogue_with` at the strength, not at the scale: an 8 here would mean the upscale
        // flag had leaked into the denoise family, and 8 is outside what `Strength` accepts at all.
        assert!(!named("stockholm").contains('8'), "{}", named("stockholm"));
    }

    #[test]
    fn the_listing_is_every_published_row_and_says_nothing_about_what_can_be_run() {
        let listing = listing();

        for entry in catalogue() {
            assert!(listing.contains(entry.family.label()), "{:?} is missing from the listing", entry.family);

            for variant in &entry.variants {
                assert!(listing.contains(variant.codename), "{} is missing from the listing", variant.codename);
                assert!(listing.contains(variant.label), "{} is missing from the listing", variant.label);

                for precision in variant.precisions {
                    assert!(listing.contains(precision.as_str()), "{precision} is missing from the listing");
                }
            }
        }

        // No judgement, in either direction. A row marked runnable or not would be a second copy of a decision the
        // library makes, and wrong the day after the next pipeline lands.
        for judgement in ["unsupported", "not supported", "runnable", "unavailable", "skipped", "no pipeline"] {
            assert!(!listing.to_lowercase().contains(judgement), "the listing claims `{judgement}`");
        }
    }

    #[test]
    fn a_familys_published_parameter_and_its_range_are_shown_beside_it() {
        let listing = listing();

        // So that `perftest list` answers "which flag moves this family" without a second table saying so.
        assert!(listing.contains("--scale"), "{listing}");
        assert!(listing.contains("--strength"), "{listing}");
        assert!(listing.contains("--bias"), "{listing}");
    }
}
