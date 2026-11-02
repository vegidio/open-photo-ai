//! One run, from the checked arguments to the written files.
//!
//! ```text
//! the update check             --> started at launch, beside the checks below
//! output, inputs               --> refused here: nothing installed
//! the update line              --> 💡 and a blank line, when the check answered "outdated" within 1.5s of launch
//! initialize                   --> once; the 💾 line; a Ctrl-C on it abandons initialization; another instance ends it
//! check the processor          --> one this machine does not offer is refused here: no image loaded
//! the processor line           --> 🧠 naming the processor asked for and the quality, then a blank line; every mode
//! Ctrl-C handler               --> from here a SIGINT stops the run instead of killing the process (plain mode)
//! start the batch's view       --> one live session for every image: the batch's bar over the list
//! for each image:
//!   plan, start its row        --> 🖼️ [  0%] Preparing to enhance <path>; the bar is not touched. Under --ap the
//!                                  plan waits for the image itself
//!   load                       --> under --ap: 🖼️ [  0%] Choosing enhancements for <path> while the analysis runs,
//!                                  then the chain planned from its suggestions and the OPAI_AP_* models; a signal it
//!                                  could not read is logged and the image goes on
//!   find faces, process        --> the row's percentage and words, and the bar, move with each report over detection
//!                                  and chain, planned for the image's own size; a failed detection is logged and
//!                                  the image goes on
//!   claim, save, commit        --> the source's depth, the library's recommended settings; the log's `wrote` record
//!                                  names what was applied and what the run cache served
//!   end its row                --> Enhancements applied, Failed or Stopped, and the bar's [n/N] counts it; under
//!                                  --verbose a written one adds `: <applied>`; off a terminal, its one plain line
//! finish the view, summarize   --> ✨ naming the file for one image, or for more the output or "beside their
//!                                  sources" without one, then each failure's 🧨 line where the view hid it
//! ```
//!
//! A Ctrl-C stops the whole batch, keeping what was already written. The notes, the face report, what each image
//! executed on and every path written go to the log, not to the terminal; what each image received goes to both
//! under `--verbose`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use opai::{
    APP_NAME, CLI, CancellationToken, ChainFaces, Enhanced, ExecuteOptions, ExecutionProvider, FaceSelection,
    InitOptions, OnInference, Opai, OutputDepth, Picture, ProcessOptions, ProviderVerdict, Suggestions,
    default_face_selection, image,
};

use tracing::Instrument;

use crate::analytics;
use crate::args::Selection;
use crate::args::{Args, Processor, Quality};
use crate::autopilot::AutopilotModels;
use crate::chain::{self, Plan};
use crate::error::CliError;
use crate::events;
use crate::inputs::{self, Source};
use crate::look::{self, ImageEnd, Look};
use crate::output::{self, Target};
use crate::progress::{self, View};
use crate::update;

/// Runs what `args` asks for over every image it names, drawing as `look` says, and writes their results.
///
/// The one [`Opai`] handle is owned here and dropped before this returns, which releases the single-instance claim and
/// waits for the run cache's pending writes. Every live line is finished before this returns, so the caller's failure
/// line lands below them.
///
/// # Errors
///
/// [`CliError`], whose [`exit_code`](CliError::exit_code) is the process's status: a refusal or an initialization
/// failure before any image, a stop, the image's own [`CliError::Image`] when the only image failed, or
/// [`CliError::Batch`] when any image of several failed. The results already written are kept on a stop or a failure.
pub async fn run(args: Args, look: Look) -> Result<(), CliError> {
    // Everything that can be refused without the library, before it is initialized: a typo in the fortieth path must
    // not cost the first run's download. The output is read first, so the cheap refusals do not wait for a walk.
    // The check for a newer release runs meanwhile, and only its answer waits.
    let launched = tokio::time::Instant::now();
    let update = update::start();

    let mut target = output::target(&args)?;
    let skip = match &target {
        // A directory that does not exist yet holds no earlier results to skip.
        Target::Directory { path, .. } => std::fs::canonicalize(path).ok(),
        // Beside its source, a result is in no directory of its own to skip.
        Target::File { .. } | Target::Beside { .. } => None,
    };
    let sources = inputs::expand(&args.inputs, args.recursive, skip.as_deref())?;
    output::keep_inputs(&mut target, sources.iter().map(|source| source.path.as_path()));
    events::files_added(&sources.iter().map(|source| source.path.as_path()).collect::<Vec<_>>());

    // Before the dependency line, the first thing that can draw. A refusal above prints its failure line alone.
    if update::outdated(update, launched + update::WAIT).await {
        eprintln!("{}", look::update_line(look.color));
        eprintln!();
    }

    let cancel = CancellationToken::new();
    let view = View::new(look, cancel.clone());

    // `initialize` has no stop of its own. While the dependency line is drawn, a Ctrl-C is a key press that wakes
    // `install_interrupted`, and dropping the future here is no worse than the kill it replaces: the claim's guard
    // releases it and a transfer resumes onto the bytes already on disk. With no line drawn, a Ctrl-C keeps its default
    // meaning and ends the process.
    let initialized = tokio::select! {
        initialized = Opai::initialize(
            APP_NAME,
            Some(InitOptions {
                app: Some(CLI.to_string()),
                on_progress: Some(view.on_install()),
                ..Default::default()
            }),
        ) => initialized,
        () = view.install_interrupted() => {
            view.finish_install();
            return Err(CliError::Cancelled);
        }
    };
    view.finish_install();
    let opai = initialized?;

    // Only now: CUDA, TensorRT and WebGPU are known once the install plan has run and the runtime has loaded.
    let supported = opai.providers();
    check_processor(args.processor, supported.supports(args.processor.into()), &supported.available())?;
    events::app_ready(&args.processor.name(), args.autopilot);

    // No session holds stderr here: the dependency line is frozen and the batch's view has not started.
    eprintln!("{}", look::processor_line(args.processor.into(), args.quality, look.color));
    eprintln!();

    // In plain mode a Ctrl-C is still a SIGINT. While the batch's view is drawn it is a key press, which the view
    // turns into the same cancellation.
    stop_on_ctrl_c(&cancel);

    let mut written = Vec::new();
    let mut failures = Vec::new();
    // Set only where the stop left an image unwritten or unstarted: a Ctrl-C landing after the last image was written
    // stopped nothing.
    let mut stopped = false;
    // `parse` fills the models whenever `--autopilot` is given; the default is only for an `Args` built otherwise.
    let autopilot = args.autopilot.then(|| args.autopilot_models.unwrap_or_default());

    events::export_started(sources.len(), &format_name(&target), &args.processor.name());
    let began = Instant::now();

    view.start_batch(sources.len());
    for (index, source) in (1..).zip(&sources) {
        // A Ctrl-C while an image was loaded or written, or between two images, lands here.
        if cancel.is_cancelled() {
            stopped = true;
            break;
        }

        let planner = match &autopilot {
            Some(models) => Planner::Autopilot(models),
            None => {
                let plan = chain::plan(&args.selection, args.quality, source.width, source.height);
                log_notes(&source.path, &plan);
                Planner::Fixed(plan)
            }
        };
        view.start_image(index, &source.path);

        // Each image's unit of work, under the name the GUI gives its command for one file's export. No path: the
        // span's records already name it in the file, and the collector is never sent one.
        let span = tracing::info_span!(
            target: analytics::TARGET,
            "export",
            outcome = tracing::field::Empty,
            error = tracing::field::Empty
        );
        let result = enhance_one(&opai, &args, &target, source, index, planner, &view, &cancel)
            .instrument(span.clone())
            .await;
        analytics::mark(&span, &result);
        drop(span);

        match result {
            Ok((path, applied)) => {
                view.end_image(index, &source.path, &ImageEnd::Enhanced(args.verbose.then_some(applied)));
                written.push(path);
            }
            Err(CliError::Cancelled) => {
                view.end_image(index, &source.path, &ImageEnd::Stopped);
                stopped = true;
                break;
            }
            Err(error) => {
                tracing::error!("{}: {}", source.path.display(), crate::full(&error));
                view.end_image(index, &source.path, &ImageEnd::Failed(error.to_string()));
                failures.push(CliError::Image { path: source.path.clone(), source: Box::new(error) });
            }
        }
    }
    let live = view.finish_batch();
    events::export_finished(sources.len(), written.len(), !stopped, began.elapsed().as_millis());

    for line in summary(&written, args.output.as_deref(), &failures, live, sources.len(), look.color) {
        eprintln!("{line}");
    }

    drop(opai);

    closing(stopped, failures, sources.len())
}

/// The lines printed once the batch's view is finished, before the closing line: the success line when anything was
/// written, then each failure's line. Those are listed only under a live view of more than one image: a failure can
/// have scrolled out of the view; off it, each image's own line already gave its reason; and a single image's failure
/// is the closing line.
fn summary(
    written: &[PathBuf],
    output: Option<&Path>,
    failures: &[CliError],
    live: bool,
    total: usize,
    color: bool,
) -> Vec<String> {
    let success = look::summary(written, output).map(|line| look::success(line, color));
    let listed = if live && total > 1 { failures } else { &[] };

    success.into_iter().chain(listed.iter().map(|failure| look::failure(failure, color))).collect()
}

/// How a run of `total` images ends: a stop over anything that failed before it, then the only image's own failure,
/// then how many of several failed.
fn closing(stopped: bool, mut failures: Vec<CliError>, total: usize) -> Result<(), CliError> {
    if stopped {
        Err(CliError::Cancelled)
    } else if total == 1 && failures.len() == 1 {
        Err(failures.remove(0))
    } else if failures.is_empty() {
        Ok(())
    } else {
        Err(CliError::Batch { failed: failures.len(), total })
    }
}

/// Refuses `requested` where this machine does not offer it, naming what it does offer. `supported` is the library's
/// answer for `requested` and `available` its list, so the rule about hardware stays the library's.
fn check_processor(requested: Processor, supported: bool, available: &[ExecutionProvider]) -> Result<(), CliError> {
    if supported {
        return Ok(());
    }

    // `available` only reports what this OS offers, so a provider with no value in this build cannot occur; it is
    // skipped rather than unwrapped all the same.
    let names: Vec<String> = std::iter::once(Processor::Auto)
        .chain(available.iter().filter_map(|provider| Processor::from_provider(*provider)))
        .map(Processor::name)
        .collect();
    let choices = match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        _ => names.join(""),
    };

    Err(CliError::Usage(format!(
        "the processor {} is not available on this machine; use {choices}",
        requested.name()
    )))
}

/// The format the run writes, as `export_started` names it: the naming's choice, or the format of the one file.
fn format_name(target: &Target) -> String {
    match target {
        Target::Directory { naming, .. } | Target::Beside { naming, .. } => events::format_name(naming.format),
        Target::File { format, .. } => events::export_format_of(*format)
            .map_or_else(|| format.name().to_string(), |format| format.name().to_string()),
    }
}

/// Where an image's chain comes from.
enum Planner<'a> {
    /// The flags': planned before the image is loaded, from its header's size.
    Fixed(Plan),
    /// Autopilot's: planned once the loaded image has been analysed, with these models.
    Autopilot(&'a AutopilotModels),
}

/// Enhances `source`, the image at `index`, with the chain `planner` gives for its own size, on the processor `args`
/// names, and writes it, answering the path written and what it received, in words.
#[allow(
    clippy::too_many_arguments,
    reason = "one run's state, threaded through rather than bundled for one caller"
)]
async fn enhance_one(
    opai: &Opai,
    args: &Args,
    target: &Target,
    source: &Source,
    index: usize,
    planner: Planner<'_>,
    view: &View,
    cancel: &CancellationToken,
) -> Result<(PathBuf, String), CliError> {
    // A file whose header read cleanly but whose pixels do not decode fails this image, not the batch. The library's
    // message names the path.
    let picture = image::load(&source.path).await.map_err(|error| CliError::Load(Box::new(error)))?;
    let provider = args.processor.into();

    let (plan, from) = match planner {
        Planner::Fixed(plan) => (plan, events::Source::Manual),
        Planner::Autopilot(models) => {
            let plan =
                choose(opai, &picture, &source.path, index, models, args.quality, provider, view, cancel).await?;
            (plan, events::Source::Autopilot)
        }
    };
    for operation in &plan.operations {
        events::enhancement_added(operation.family(), from);
    }

    let enhanced = enhance(opai, &picture, &plan.operations, provider, view, cancel).await?;

    // A Ctrl-C that landed after the last tile was checked still means "write nothing".
    if cancel.is_cancelled() {
        return Err(CliError::Cancelled);
    }

    let (destination, format) = output::destination(target, source);
    let overwrite = args.overwrite && output::may_overwrite(target, &source.path, &destination);
    if args.overwrite && !overwrite {
        tracing::warn!("{} is another input, so it is kept and the result numbered", destination.display());
    }
    let written = output::write(target, &enhanced.picture, &destination, format, overwrite).await?;

    let (width, height) = enhanced.picture.dimensions();
    let verdict = enhanced.providers.verdict();
    let applied = applied(&plan, view.cached(index), &verdict);
    tracing::info!(
        "wrote {} ({width}x{height}, {}); {}; applied {applied}",
        written.display(),
        format.name(),
        provenance(plan.operations.is_empty(), &verdict)
    );

    Ok((written, applied))
}

/// What `plan` applied, in words, ending with how many of its steps the run cache served: `cached` as the progress
/// reports counted them, or every one where the verdict says nothing executed, so a missed report cannot hide it.
fn applied(plan: &Plan, cached: usize, verdict: &ProviderVerdict) -> String {
    let ran = plan.operations.len();
    let cached = if ran > 0 && matches!(verdict, ProviderVerdict::NothingExecuted) { ran } else { cached };

    format!("{}{}", chain::describe(&plan.applied), progress::cache_marker(cached, ran))
}

/// Analyses `picture`, the image at `index` read from `path`, for the families `models` leaves on, and plans the chain
/// its suggestions call for at `quality`, with the row reading `Choosing enhancements for` meanwhile.
///
/// A stop during the analysis is a stop. A signal it could not read is logged, and the image gets what the others
/// suggested.
#[allow(
    clippy::too_many_arguments,
    reason = "one run's state, threaded through rather than bundled for one caller"
)]
async fn choose(
    opai: &Opai,
    picture: &Picture,
    path: &Path,
    index: usize,
    models: &AutopilotModels,
    quality: Quality,
    provider: ExecutionProvider,
    view: &View,
    cancel: &CancellationToken,
) -> Result<Plan, CliError> {
    view.choosing(index);

    // No progress relayed: the only reporting work is the face detection, and the row has no chain yet to map it
    // onto. The chain's own detection is then served from the store this one warmed.
    let families = models.families();
    let options = ExecuteOptions { on_progress: None, cancel: cancel.clone(), provider, ..Default::default() };
    let suggestions = opai.suggest(picture, Some(&families), Some(options)).await?;
    view.chosen(index);

    let (width, height) = picture.dimensions();
    let (plan, warning) = plan_suggested(path, &suggestions, models, quality, width, height);
    events::autopilot_run(plan.operations.len());
    if let Some(warning) = warning {
        tracing::warn!("{warning}");
    }
    log_notes(path, &plan);

    Ok(plan)
}

/// The chain `suggestions` calls for over the `width` x `height` image at `path`, and the warning to log when a signal
/// of the analysis could not be read. What the other signals suggested is planned either way.
fn plan_suggested(
    path: &Path,
    suggestions: &Suggestions,
    models: &AutopilotModels,
    quality: Quality,
    width: u32,
    height: u32,
) -> (Plan, Option<String>) {
    let warning = suggestions
        .incomplete
        .as_ref()
        .map(|error| format!("{}: the analysis could not read every signal: {error}", path.display()));
    let plan = chain::plan(&Selection::from_suggestions(&suggestions.suggested, models), quality, width, height);

    (plan, warning)
}

/// Records `plan`'s notes about the image at `path` in the log.
fn log_notes(path: &Path, plan: &Plan) {
    for note in &plan.notes {
        tracing::info!("{}: {note}", path.display());
    }
}

/// Runs `operations` on `provider`, finding the faces its face recoveries restore first, reporting onto the image's
/// row and the batch's bar.
async fn enhance(
    opai: &Opai,
    picture: &Picture,
    operations: &[opai::Operation],
    provider: ExecutionProvider,
    view: &View,
    cancel: &CancellationToken,
) -> Result<Enhanced, CliError> {
    let enhanced = opai
        .process(picture, operations, Some(chain_options(provider, view.on_inference(), cancel)))
        .await?;

    if let Some(report) = face_report(&enhanced.faces) {
        if matches!(enhanced.faces, ChainFaces::Failed(_)) {
            tracing::warn!("{report}");
        } else {
            tracing::info!("{report}");
        }
    }

    Ok(enhanced)
}

/// What the chain is asked for: `provider` at the source's depth, restoring the faces [`default_face_selection`] keeps,
/// reporting to `on_progress`, stopped by `cancel`.
fn chain_options(provider: ExecutionProvider, on_progress: OnInference, cancel: &CancellationToken) -> ProcessOptions {
    ProcessOptions {
        depth: OutputDepth::Source,
        on_progress: Some(on_progress),
        cancel: cancel.clone(),
        provider,
        faces: FaceSelection::restorable(),
        ..Default::default()
    }
}

/// Cancels `cancel` on the first SIGINT, which arrives only while no live session holds the terminal in raw mode.
fn stop_on_ctrl_c(cancel: &CancellationToken) {
    let cancel = cancel.clone();

    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            tracing::info!("stopping…");
            cancel.cancel();
        }
    });
}

/// What finding the faces came to, as the log records it, or `None` where no face recovery asked for it.
///
/// The count restored is the faces [`default_face_selection`] keeps, which is exactly what the step handed the
/// recovery.
fn face_report(faces: &ChainFaces) -> Option<String> {
    match faces {
        ChainFaces::NotAsked => None,
        ChainFaces::Found(faces) if faces.is_empty() => Some("no faces were found; none will be restored".to_string()),
        ChainFaces::Found(faces) => {
            let restoring =
                faces.iter().enumerate().filter(|(index, face)| default_face_selection(*index, face)).count();
            Some(format!("found {} {}, restoring {restoring}", faces.len(), plural(faces.len(), "face", "faces")))
        }
        ChainFaces::Failed(error) => Some(format!("face detection failed, so no face will be restored: {error}")),
    }
}

/// What the run executed on, as the log records it. `empty` is a chain with nothing in it — an upscale that
/// was skipped and nothing else asked for — which the library reports exactly as a run served from the cache.
fn provenance(empty: bool, verdict: &ProviderVerdict) -> String {
    if empty {
        return "no enhancement ran".to_string();
    }

    let names = |providers: &[opai::ExecutionProvider]| {
        providers.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
    };

    // `Resolved` is an `auto` run's; `AsRequested` and `Downgraded` are a named processor's, the latter where it could
    // not build a session for some model and that model fell back to the CPU.
    match verdict {
        ProviderVerdict::Resolved(actual) => format!("ran on {}", names(actual)),
        ProviderVerdict::NothingExecuted => "served from cache".to_string(),
        ProviderVerdict::AsRequested(provider) => format!("ran on {provider}"),
        ProviderVerdict::Downgraded { requested, actual } => {
            format!("ran on {} ({requested} could not open every model)", names(actual))
        }
    }
}

/// `singular` for one, `plural` otherwise.
const fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::Quality;
    use crate::progress::tests::{inference, upscale};
    use clap::ValueEnum;
    use opai::{Confidence, Face, Faces, InferenceError, Point, Rect, Stage};

    /// A face whose box is `side` pixels square: restorable up to 512, too large above it.
    fn face(side: f32) -> Face {
        let landmarks = [Point::new(side / 2.0, side / 2.0); Face::LANDMARKS];
        Face::new(
            Rect::new(Point::new(0.0, 0.0), Point::new(side, side)),
            landmarks,
            Confidence::new(0.9).expect("in range"),
        )
    }

    #[test]
    fn no_face_recovery_reports_nothing() {
        assert_eq!(face_report(&ChainFaces::NotAsked), None);
    }

    #[test]
    fn faces_found_report_how_many_the_default_selection_restores() {
        let faces = Faces::new([face(100.0), face(1000.0), face(512.0)]);

        assert_eq!(face_report(&ChainFaces::Found(faces)).as_deref(), Some("found 3 faces, restoring 2"));
        assert_eq!(
            face_report(&ChainFaces::Found(Faces::new([face(100.0)]))).as_deref(),
            Some("found 1 face, restoring 1")
        );
    }

    #[test]
    fn no_faces_found_says_so() {
        assert_eq!(
            face_report(&ChainFaces::Found(Faces::empty())).as_deref(),
            Some("no faces were found; none will be restored")
        );
    }

    #[test]
    fn a_failed_detection_warns_carrying_the_failure() {
        let failure = InferenceError::Shutdown;
        let expected = failure.to_string();

        let report = face_report(&ChainFaces::Failed(failure)).expect("a failure is reported");

        assert!(report.starts_with("face detection failed"), "{report}");
        assert!(report.ends_with(&expected), "{report}");
    }

    #[test]
    fn the_completion_line_names_the_providers_or_the_cache() {
        let resolved = ProviderVerdict::Resolved(vec![ExecutionProvider::CoreMl, ExecutionProvider::Cpu]);
        assert_eq!(provenance(false, &resolved), "ran on CoreML, CPU");

        assert_eq!(provenance(false, &ProviderVerdict::NothingExecuted), "served from cache");

        assert_eq!(provenance(false, &ProviderVerdict::AsRequested(ExecutionProvider::Cuda)), "ran on CUDA");

        let downgraded = ProviderVerdict::Downgraded {
            requested: ExecutionProvider::TensorRt,
            actual: vec![ExecutionProvider::Cpu],
        };
        assert_eq!(provenance(false, &downgraded), "ran on CPU (TensorRT could not open every model)");
    }

    #[test]
    fn plain_output_is_the_processor_line_then_one_line_per_image_then_the_summary_then_the_closing_line() {
        // The spec's "Output piped to a file": three images on auto at hd, the second failing at 42%.
        let view = View::new(Look { interactive: false, color: false }, CancellationToken::new());
        let on_inference = view.on_inference();
        let [a, b, c] = ["photos/a.jpg", "photos/b.jpg", "photos/c.jpg"].map(Path::new);
        let mut lines = vec![look::processor_line(ExecutionProvider::Auto, Quality::Hd, false), String::new()];

        view.start_batch(3);
        view.start_image(1, a);
        on_inference(&inference(upscale(), Stage::Running, 1.0));
        lines.extend(view.end_line(1, a, &ImageEnd::Enhanced(None)));
        view.start_image(2, b);
        on_inference(&inference(upscale(), Stage::Running, 0.42));
        lines.extend(view.end_line(2, b, &ImageEnd::Failed("disk full".into())));
        view.start_image(3, c);
        on_inference(&inference(upscale(), Stage::Running, 1.0));
        lines.extend(view.end_line(3, c, &ImageEnd::Enhanced(None)));
        let live = view.finish_batch();

        let written = [PathBuf::from("out/a.jpg"), PathBuf::from("out/c.jpg")];
        let failures = vec![failed("photos/b.jpg")];
        lines.extend(summary(&written, Some(Path::new("out")), &failures, live, 3, false));
        let closing = closing(false, failures, 3).expect_err("an image failed");
        lines.push(look::failure(closing, false));

        assert_eq!(
            lines,
            [
                "🧠 Running inference using the processor Auto; quality HD",
                "",
                "🖼️ [100%] Enhancements applied to       photos/a.jpg",
                "🧨 [ 42%] Failed to enhance             photos/b.jpg: disk full",
                "🖼️ [100%] Enhancements applied to       photos/c.jpg",
                "✨ 2 images enhanced successfully; files saved under out",
                "🧨 1 of 3 images failed",
            ]
        );
    }

    #[test]
    fn a_live_view_of_several_images_lists_each_failure_under_the_summary() {
        let failures = [failed("a.jpg"), failed("c.jpg")];
        let lines = summary(&["out/b.jpg".into()], Some(Path::new("out")), &failures, true, 3, false);

        assert_eq!(lines.len(), 3, "{lines:?}");
        assert_eq!(lines[0], "✨ Image enhanced successfully; file saved at out/b.jpg");
        assert!(lines[1].starts_with("🧨 could not enhance a.jpg: "), "{lines:?}");
        assert!(lines[2].starts_with("🧨 could not enhance c.jpg: "), "{lines:?}");
    }

    #[test]
    fn a_single_image_or_plain_output_does_not_list_failures() {
        let failures = [failed("a.jpg")];

        assert!(summary(&[], Some(Path::new("out.png")), &failures, true, 1, false).is_empty());
        assert!(summary(&[], Some(Path::new("out")), &failures, false, 3, false).is_empty());
        assert!(summary(&[], None, &failures, false, 3, false).is_empty());
    }

    fn failed(path: &str) -> CliError {
        CliError::Image { path: path.into(), source: Box::new(CliError::Inference(InferenceError::Shutdown)) }
    }

    #[test]
    fn a_run_with_nothing_failed_succeeds() {
        assert!(closing(false, Vec::new(), 1).is_ok());
        assert!(closing(false, Vec::new(), 5).is_ok());
    }

    #[test]
    fn a_stop_ends_as_a_stop_even_after_a_failure() {
        assert!(matches!(closing(true, Vec::new(), 5), Err(CliError::Cancelled)));
        assert!(matches!(closing(true, vec![failed("a.jpg")], 5), Err(CliError::Cancelled)));
        assert!(matches!(closing(true, vec![failed("a.jpg")], 1), Err(CliError::Cancelled)));
    }

    #[test]
    fn a_single_image_ends_with_its_own_failure() {
        let error = closing(false, vec![failed("a.jpg")], 1).expect_err("the image failed");

        assert!(matches!(&error, CliError::Image { path, .. } if path.as_path() == Path::new("a.jpg")));
        assert!(error.to_string().starts_with("could not enhance a.jpg: "), "{error}");
    }

    #[test]
    fn several_images_end_with_how_many_failed() {
        let error = closing(false, vec![failed("a.jpg")], 3).expect_err("an image failed");
        assert!(matches!(error, CliError::Batch { failed: 1, total: 3 }));

        let error = closing(false, vec![failed("a.jpg"), failed("b.jpg")], 3).expect_err("two images failed");
        assert!(matches!(error, CliError::Batch { failed: 2, total: 3 }));
    }

    #[test]
    fn the_chain_and_its_face_detection_are_asked_for_the_chosen_processor() {
        let cancel = CancellationToken::new();

        for processor in Processor::value_variants() {
            let provider = ExecutionProvider::from(*processor);

            let chain = chain_options(provider, std::sync::Arc::new(|_| {}), &cancel);
            assert_eq!(chain.provider, provider);
            assert_eq!(chain.depth, OutputDepth::Source);
            assert!(matches!(chain.faces, FaceSelection::Detect(_)), "the chain finds no faces");
        }
    }

    #[test]
    fn check_processor_passes_auto_and_cpu_on_a_cpu_only_machine() {
        let cpu_only = [ExecutionProvider::Cpu];

        assert!(check_processor(Processor::Auto, true, &cpu_only).is_ok());
        assert!(check_processor(Processor::Cpu, true, &cpu_only).is_ok());
    }

    #[test]
    fn check_processor_passes_a_supported_accelerator() {
        assert!(check_processor(Processor::WebGpu, true, &[ExecutionProvider::Cpu, ExecutionProvider::WebGpu]).is_ok());
    }

    #[test]
    fn check_processor_refuses_an_unsupported_one_naming_what_this_machine_offers() {
        let error = check_processor(Processor::WebGpu, false, &[ExecutionProvider::Cpu]).expect_err("no WebGPU");

        assert!(matches!(error, CliError::Usage(_)), "{error:?}");
        assert_eq!(error.exit_code(), 2);
        assert_eq!(error.to_string(), "the processor webgpu is not available on this machine; use auto or cpu");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn check_processor_joins_a_longer_list_with_or() {
        let available = [ExecutionProvider::Cpu, ExecutionProvider::CoreMl];
        let error = check_processor(Processor::WebGpu, false, &available).expect_err("no WebGPU");

        assert_eq!(
            error.to_string(),
            "the processor webgpu is not available on this machine; use auto, cpu or coreml"
        );
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn check_processor_joins_a_longer_list_with_or() {
        let error = check_processor(Processor::TensorRt, false, &[ExecutionProvider::Cpu]).expect_err("no TensorRT");
        assert_eq!(error.to_string(), "the processor tensorrt is not available on this machine; use auto or cpu");

        let available = [ExecutionProvider::Cpu, ExecutionProvider::Cuda, ExecutionProvider::WebGpu];
        let error = check_processor(Processor::TensorRt, false, &available).expect_err("no TensorRT");

        assert_eq!(
            error.to_string(),
            "the processor tensorrt is not available on this machine; use auto, cpu, cuda or webgpu"
        );
    }

    fn fixed(line: &str) -> Plan {
        let args = crate::args::parse_from(line.split_whitespace()).expect("parses");
        chain::plan(&args.selection, args.quality, 1000, 800)
    }

    #[test]
    fn the_applied_text_names_the_chain_and_what_the_cache_served() {
        let plan = fixed("opai photo.jpg --dn --la --sh -o out.png");
        let ran = ProviderVerdict::Resolved(vec![ExecutionProvider::Cpu]);
        let base = "Denoise Stockholm strength 100, Light Adjust Paris bias 50, Sharpen Moscow strength 100";

        assert_eq!(applied(&plan, 0, &ran), base);
        assert_eq!(applied(&plan, 2, &ran), format!("{base} (2 of 3 cached)"));
        assert_eq!(applied(&plan, 3, &ran), format!("{base} (cached)"));
    }

    #[test]
    fn a_run_nothing_executed_for_is_cached_whatever_the_reports_counted() {
        let plan = fixed("opai photo.jpg --dn --up --scale 2 -o out.png");

        assert_eq!(
            applied(&plan, 0, &ProviderVerdict::NothingExecuted),
            "Denoise Stockholm strength 100, Upscale Tokyo 2x (cached)"
        );
    }

    #[test]
    fn an_empty_chain_needs_no_enhancement_and_is_not_cached() {
        let args = crate::args::parse_from("opai large.jpg --up -o out.png".split_whitespace()).expect("parses");
        let plan = chain::plan(&args.selection, args.quality, 4000, 3000);

        assert_eq!(applied(&plan, 0, &ProviderVerdict::NothingExecuted), "no enhancement needed");
    }

    #[test]
    fn a_verbose_plain_line_carries_the_text_and_a_failure_only_its_reason() {
        let view = View::new(Look { interactive: false, color: false }, CancellationToken::new());
        let [a, b] = ["photos/a.jpg", "photos/b.jpg"].map(Path::new);

        view.start_batch(2);
        view.start_image(1, a);
        let written = ImageEnd::Enhanced(Some("Upscale Tokyo 2x".to_owned()));
        let first = view.end_line(1, a, &written).expect("a plain line");
        view.start_image(2, b);
        let second = view.end_line(2, b, &ImageEnd::Failed("disk full".into())).expect("a plain line");

        assert_eq!(first, "🖼️ [100%] Enhancements applied to       photos/a.jpg: Upscale Tokyo 2x");
        assert_eq!(second, "🧨 [  0%] Failed to enhance             photos/b.jpg: disk full");
    }

    #[test]
    fn an_empty_chain_is_not_reported_as_cached() {
        assert_eq!(provenance(true, &ProviderVerdict::NothingExecuted), "no enhancement ran");
    }

    #[test]
    fn a_detector_that_could_not_be_obtained_leaves_the_other_suggestions_and_a_warning() {
        let scale = opai::Scale::new(4.0).expect("in range");
        let detector = InferenceError::Open {
            artifact: "dt_newyork_fp32".to_owned(),
            provider: ExecutionProvider::Cpu,
            source: std::sync::Arc::new(std::io::Error::other("corrupt model")),
        };
        let suggestions =
            Suggestions { suggested: vec![opai::Suggestion::Upscale { scale }], incomplete: Some(detector) };

        let (plan, warning) =
            plan_suggested(Path::new("portrait.jpg"), &suggestions, &AutopilotModels::default(), Quality::Hd, 800, 600);

        assert_eq!(plan.operations, vec![opai::Upscale::tokyo(opai::FloatPrecision::Fp32, scale)]);
        let warning = warning.expect("the failed signal is warned about");
        assert!(warning.starts_with("portrait.jpg: "), "{warning}");
        assert!(warning.contains("dt_newyork_fp32") && warning.contains("corrupt model"), "{warning}");
    }

    #[test]
    fn a_complete_analysis_warns_about_nothing() {
        let suggestions = Suggestions { suggested: Vec::new(), incomplete: None };

        let (plan, warning) =
            plan_suggested(Path::new("large.jpg"), &suggestions, &AutopilotModels::default(), Quality::Hd, 4000, 3000);

        assert!(plan.operations.is_empty());
        assert_eq!(warning, None);
    }
}
