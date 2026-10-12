//! One run, from the checked arguments to the written file.
//!
//! ```text
//! output format, input header  --> refused here: nothing installed
//! initialize                   --> the 💾 line; a Ctrl-C on it abandons initialization; another instance ends it
//! Ctrl-C handler               --> from here a SIGINT stops the run instead of killing the process
//! header, load                 --> 🖼️ and a blank line
//! find faces, process          --> one bar over both; a failed detection is logged and the run goes on
//! save                         --> the source's depth, the library's recommended settings; then ✨
//! ```
//!
//! The notes, the face report and what the run executed on go to the log, not to the terminal.

use opai::{
    APP_NAME, CLI, CancellationToken, ChainFaces, Enhanced, ExecuteOptions, FacedChain, Handover, InitOptions, Opai,
    OutputDepth, Picture, ProcessOptions, ProviderVerdict, default_face_selection, image,
};

use crate::args::Args;
use crate::error::CliError;
use crate::look::{self, Look};
use crate::progress::View;
use crate::{chain, output};

/// Runs what `args` asks for, drawing as `look` says, and writes its output.
///
/// The one [`Opai`] handle is owned here and dropped before this returns, which releases the single-instance claim and
/// waits for the run cache's pending writes. Every live line is finished before this returns, so the caller's failure
/// line lands below them.
///
/// # Errors
///
/// [`CliError`], whose [`exit_code`](CliError::exit_code) is the process's status. Nothing is written on any of them.
pub async fn run(args: Args, look: Look) -> Result<(), CliError> {
    // Everything that can be refused without the library, before it is initialized: a typo must not cost the first
    // run's download.
    let output = output::resolve(&args.output)?;
    let (width, height) = output::check_input(&args.input)?;
    let plan = chain::plan(&args, width, height);

    for note in &plan.notes {
        tracing::info!("{note}");
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

    // In plain mode, and between the live lines, a Ctrl-C is still a SIGINT.
    stop_on_ctrl_c(&cancel);

    eprintln!("{}", look::header(&args.input, look.color));
    if look.interactive {
        eprintln!();
    }

    // A file whose header read cleanly but whose pixels do not decode is still an unreadable input, so it ends as one.
    // The library's message names the path.
    let picture = image::load(&args.input).await.map_err(|error| CliError::Usage(error.to_string()))?;

    // Finished on every outcome, so the failure line lands below the bar and the blank line after it.
    view.start_bar(plan.operations.len());
    let enhanced = enhance(&opai, &picture, plan.operations, &view, &cancel).await;
    view.finish_bar();
    let (faced, enhanced) = enhanced?;

    // A Ctrl-C that landed after the last tile was checked still means "write nothing".
    if cancel.is_cancelled() {
        return Err(CliError::Cancelled);
    }

    image::save(enhanced.picture.shared_pixels(), &output.path, output.format, Some(output.options))
        .await
        .map_err(|source| CliError::Write(Box::new(source)))?;

    let (width, height) = enhanced.picture.dimensions();
    tracing::info!(
        "wrote {} ({width}x{height}, {}); {}",
        output.path.display(),
        output.format.name(),
        provenance(faced.operations.is_empty(), &enhanced.providers.verdict())
    );

    drop(opai);
    eprintln!("{}", look::success(&output.path, look.color));
    Ok(())
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
    fn an_empty_chain_is_not_reported_as_cached() {
        assert_eq!(provenance(true, &ProviderVerdict::NothingExecuted), "no enhancement ran");
    }
}
