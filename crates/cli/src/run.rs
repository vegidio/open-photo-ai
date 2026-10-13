//! One run, from the checked arguments to the written files.
//!
//! ```text
//! output, inputs               --> refused here: nothing installed
//! initialize                   --> once; the 💾 line; a Ctrl-C on it abandons initialization; another instance ends it
//! Ctrl-C handler               --> from here a SIGINT stops the run instead of killing the process
//! for each image:
//!   header, load               --> 🖼️ and a blank line
//!   find faces, process        --> one bar over both, planned for the image's own size; a failed detection is
//!                                  logged and the image goes on
//!   claim, save, commit        --> the source's depth, the library's recommended settings; then ✨ naming the file
//!                                  written, or 🧨 naming the image, and the next image runs
//! ```
//!
//! A Ctrl-C stops the whole batch, keeping what was already written. The notes, the face report and what each image
//! executed on go to the log, not to the terminal.

use std::path::{Path, PathBuf};

use opai::{
    APP_NAME, CLI, CancellationToken, ChainFaces, Enhanced, ExecuteOptions, FacedChain, Handover, InitOptions, Opai,
    OutputDepth, Picture, ProcessOptions, ProviderVerdict, default_face_selection, image,
};

use crate::args::Args;
use crate::chain;
use crate::error::CliError;
use crate::inputs::{self, Source};
use crate::look::{self, Look};
use crate::output::{self, Target};
use crate::progress::View;

/// Runs what `args` asks for over every image it names, drawing as `look` says, and writes their results.
///
/// The one [`Opai`] handle is owned here and dropped before this returns, which releases the single-instance claim and
/// waits for the run cache's pending writes. Every live line is finished before this returns, so the caller's failure
/// line lands below them.
///
/// # Errors
///
/// [`CliError`], whose [`exit_code`](CliError::exit_code) is the process's status: a refusal or an initialization
/// failure before any image, a stop, or [`CliError::Batch`] when any image failed. The results already written are
/// kept on a stop or a failure.
pub async fn run(args: Args, look: Look) -> Result<(), CliError> {
    // Everything that can be refused without the library, before it is initialized: a typo in the fortieth path must
    // not cost the first run's download. The output is read first, so the cheap refusals do not wait for a walk.
    let target = output::target(&args)?;
    let skip = match &target {
        // A directory that does not exist yet holds no earlier results to skip.
        Target::Directory { path, .. } => std::fs::canonicalize(path).ok(),
        Target::File { .. } => None,
    };
    let sources = inputs::expand(&args.inputs, args.recursive, skip.as_deref())?;

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

    // In plain mode, and between the live lines, a Ctrl-C is still a SIGINT.
    stop_on_ctrl_c(&cancel);

    let mut failed = 0;
    // Set only where the stop left an image unwritten or unstarted: a Ctrl-C landing after the last image was written
    // stopped nothing.
    let mut stopped = false;
    for source in &sources {
        // A Ctrl-C between two images, with no bar to catch it, lands here.
        if cancel.is_cancelled() {
            stopped = true;
            break;
        }

        match enhance_one(&opai, &args, &target, source, &view, &cancel, look).await {
            Ok(written) => eprintln!("{}", look::success(&written, look.color)),
            Err(CliError::Cancelled) => {
                stopped = true;
                break;
            }
            Err(error) => {
                failed += 1;
                tracing::error!("{}: {}", source.path.display(), crate::full(&error));
                eprintln!("{}", look::failure(failure_reason(&source.path, &error), look.color));
            }
        }
    }

    drop(opai);

    if stopped {
        Err(CliError::Cancelled)
    } else if failed > 0 {
        Err(CliError::Batch { failed, total: sources.len() })
    } else {
        Ok(())
    }
}

/// Enhances `source` with the chain planned for its own size and writes it, answering the path written.
async fn enhance_one(
    opai: &Opai,
    args: &Args,
    target: &Target,
    source: &Source,
    view: &View,
    cancel: &CancellationToken,
    look: Look,
) -> Result<PathBuf, CliError> {
    let plan = chain::plan(args, source.width, source.height);
    for note in &plan.notes {
        tracing::info!("{}: {note}", source.path.display());
    }

    eprintln!("{}", look::header(&source.path, look.color));
    if look.interactive {
        eprintln!();
    }

    // A file whose header read cleanly but whose pixels do not decode fails this image, not the batch. The library's
    // message names the path.
    let picture = image::load(&source.path).await.map_err(|error| CliError::Load(Box::new(error)))?;

    // Finished on every outcome, so the result line lands below the bar and the blank line after it.
    view.start_bar(plan.operations.len());
    let enhanced = enhance(opai, &picture, plan.operations, view, cancel).await;
    view.finish_bar();
    let (faced, enhanced) = enhanced?;

    // A Ctrl-C that landed after the last tile was checked still means "write nothing".
    if cancel.is_cancelled() {
        return Err(CliError::Cancelled);
    }

    let (destination, format) = output::destination(target, source);
    let written = output::write(target, &enhanced.picture, &destination, format, args.overwrite).await?;

    let (width, height) = enhanced.picture.dimensions();
    tracing::info!(
        "wrote {} ({width}x{height}, {}); {}",
        written.display(),
        format.name(),
        provenance(faced.operations.is_empty(), &enhanced.providers.verdict())
    );

    Ok(written)
}

/// The failure line's reason for the image at `path`: the error as it is where it already names the image, as a failed
/// decode does, and otherwise prefixed with it, since an inference failure names nothing and a write names the
/// destination.
fn failure_reason(path: &Path, error: &CliError) -> String {
    let message = error.to_string();
    let image = path.display().to_string();

    if message.contains(&image) { message } else { format!("could not enhance {image}: {message}") }
}

/// Finds the faces `operations` needs and runs the chain, both reporting onto the view's one bar.
async fn enhance(
    opai: &Opai,
    picture: &Picture,
    operations: Vec<opai::Operation>,
    view: &View,
    cancel: &CancellationToken,
) -> Result<(FacedChain, Enhanced), CliError> {
    // Built before the operations move into `find_chain_faces`; a face selection changes no family, so it is the same
    // chain `process` runs.
    let handover = Handover::new(&view.on_bar(), &operations);

    let faced = opai
        .find_chain_faces(
            picture,
            operations,
            default_face_selection,
            Some(ExecuteOptions {
                on_progress: Some(handover.detection()),
                cancel: cancel.clone(),
                ..Default::default()
            }),
        )
        .await?;

    if let Some(report) = face_report(&faced.faces) {
        if matches!(faced.faces, ChainFaces::Failed(_)) {
            tracing::warn!("{report}");
        } else {
            tracing::info!("{report}");
        }
    }

    let enhanced = opai
        .process(
            picture,
            &faced.operations,
            Some(ProcessOptions {
                depth: OutputDepth::Source,
                on_progress: Some(handover.chain()),
                cancel: cancel.clone(),
                ..Default::default()
            }),
        )
        .await?;

    Ok((faced, enhanced))
}

/// Cancels `cancel` on the first SIGINT, which arrives only while no live line holds the terminal in raw mode.
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

    // `AsRequested` and `Downgraded` cannot happen under `Auto`, which is all this command asks for; they are said
    // anyway so the match stays total.
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
    use opai::{Confidence, ExecutionProvider, Face, Faces, InferenceError, Point, Rect};

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
    fn a_failure_line_names_the_image_once() {
        let path = Path::new("photos/a.jpg");

        let decode = CliError::Load(Box::new(opai::ImageIoError::Read {
            path: path.into(),
            source: std::io::Error::other("corrupt"),
        }));
        let reason = failure_reason(path, &decode);
        assert_eq!(reason.matches("photos/a.jpg").count(), 1, "{reason}");

        let write = CliError::Claim {
            path: "out/a.jpg".into(),
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        };
        let reason = failure_reason(path, &write);
        assert!(reason.starts_with("could not enhance photos/a.jpg: cannot write out/a.jpg: "), "{reason}");

        let reason = failure_reason(path, &CliError::Inference(InferenceError::Shutdown));
        assert!(reason.starts_with("could not enhance photos/a.jpg: "), "{reason}");
    }

    #[test]
    fn an_empty_chain_is_not_reported_as_cached() {
        assert_eq!(provenance(true, &ProviderVerdict::NothingExecuted), "no enhancement ran");
    }
}
