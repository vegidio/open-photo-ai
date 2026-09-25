//! Asking for an open image to be enhanced, stopping a run, and where the enhanced pixels live.
//!
//! [`mod@operation`] is the wire shape and what it resolves to. [`mod@slot`] is [`Runs`], which holds the
//! one run in flight and the one result. [`mod@run`] is [`enhance_with`], the seam the command below is a
//! thin wrapper over. [`mod@progress`] is the reporting the run is given.

// The commands stay here, because `tauri::generate_handler!` in `src/lib.rs` names them through this module.

use opai::Opai;
use tauri::State;

use crate::command::{Traceparent, command_span, traced, traced_sync};
use crate::images::{Crop, Opened};
use crate::setup::Setup;

mod operation;
mod progress;
mod run;
mod slot;
#[cfg(test)]
mod test_support;

// `RunProgress` is also what an export re-sends on its own event, wrapped — see `crate::export`.
pub(crate) use progress::{Reporting, RunProgress, reporting};
// Read by `faces`' tests, which assert on what the window receives for a detection.
#[cfg(test)]
pub(crate) use progress::Stage;
// Read by `export`'s tests, which pin that an export reports somewhere else.
#[cfg(test)]
pub(crate) use progress::PROGRESS_EVENT;
// `Enhancer`, `Requested` and `UnknownOperation` are also an export's: it runs the window's chain through the same
// seam, and refuses an operation it cannot serve with the same refusal — see `crate::export`.
pub(crate) use operation::{Requested, UnknownOperation};
pub(crate) use run::{EnhanceError, Enhancement, Enhancer, Processor, Request, enhance_with};
pub(crate) use slot::{Resident, Runs};

// The command's name is written once more, in `frontend/ipc/enhance.ts`.
/// Run a chain of enhancements over an open image.
///
/// `run` is the window's own name for this run, minted before the call — a stop and its run cross the
/// boundary independently and either may arrive first.
///
/// `crop` is how the user has framed that image, or absent to run over the whole photograph. The chain runs
/// over the framed pixels, so the result is the enhancement of what the window is drawing rather than of the
/// file — see [`enhance_with`].
///
/// Answers the enhanced result's identity and dimensions, or that the run was stopped — **not its pixels**,
/// which are served over the `opai://` scheme, addressed by the identity answered here.
///
/// Asking for a run stops whatever was in flight.
///
/// # Errors
///
/// [`EnhanceError`]. A stop is [`Enhancement::Stopped`] rather than an error.
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its wire shape plus its managed state"
)]
#[tauri::command]
pub(crate) async fn enhance(
    app: tauri::AppHandle,
    run: String,
    source: String,
    operations: Vec<Requested>,
    processor: Processor,
    crop: Option<Crop>,
    setup: State<'_, Setup<Opai>>,
    opened: State<'_, Opened>,
    runs: State<'_, Runs>,
    traceparent: Traceparent,
) -> Result<Enhancement, EnhanceError> {
    traced(command_span!("enhance", traceparent, run = run), async move {
        // A clone of the handle rather than the handle, and the lock released before the run starts — see
        // `Setup::peek`.
        let opai = setup.peek(Opai::clone).await.ok_or(EnhanceError::NotReady)?;
        let reporting = reporting(&app, &run);

        enhance_with(&opai, &opened, &runs, Some(reporting), Request { run, source, operations, processor, crop }).await
    })
    .await
}

// The command's name is written once more, in `frontend/ipc/enhance.ts`.
/// Stop a run the window asked for.
///
/// **Names the run**, and does nothing unless the slot still holds it — a stop for an already-displaced run is
/// the ordinary case when cleanup races the next request, and acting on it would stop the wrong run.
///
/// A stop that arrives **before** its own run still lands on it — see [`Runs`].
///
/// Answers nothing and fails at nothing; whether anything was stopped is only in the log.
#[tauri::command]
pub(crate) fn cancel_enhance(run: String, runs: State<'_, Runs>, traceparent: Traceparent) {
    traced_sync(command_span!("cancel_enhance", traceparent, run = run), || {
        if runs.stop(&run) {
            tracing::debug!("a run in flight was stopped from the window");
        } else {
            tracing::debug!("a stop named a run that is not the one in flight");
        }
    });
}

// The command's name is written once more, in `frontend/ipc/enhance.ts`.
/// Release the enhanced result made from an image the window has closed.
///
/// **Names the image, not the result.** The identity is the one on the record the window is removing, so
/// nothing has to be looked up to make the call. A release naming a photograph the slot holds no result for
/// does nothing.
///
/// A released result is reported as **displaced** rather than unknown, so a pane still drawing its address
/// reads "this is gone" rather than "you asked for something that never was".
///
/// A run in flight is not stopped by this — [`cancel_enhance`] does that when the pane goes. What it will
/// not do is hand its result to the slot afterwards.
///
/// Answers nothing and fails at nothing; whether anything was released is only in the log.
#[tauri::command]
pub(crate) fn release_enhanced(identity: String, runs: State<'_, Runs>, traceparent: Traceparent) {
    traced_sync(command_span!("release_enhanced", traceparent), || {
        if runs.release(&identity) {
            tracing::debug!(identity, "an enhanced result was released; the image it was made from was closed");
        } else {
            tracing::debug!(identity, "a release named an image no held result was made from");
        }
    });
}

// The command's name is written once more, in `frontend/ipc/enhance.ts`.
/// Release the enhanced result, because every image has been closed.
///
/// The counterpart to [`release_enhanced`] for closing everything at once, on the same terms: the run in
/// flight is left to its own stop, and the identity stays distinguishable from one that never existed.
///
/// Answers nothing and fails at nothing; whether anything was released is only in the log.
#[tauri::command]
pub(crate) fn release_all_enhanced(runs: State<'_, Runs>, traceparent: Traceparent) {
    traced_sync(command_span!("release_all_enhanced", traceparent), || {
        if runs.release_all() {
            tracing::debug!("the held enhanced result was released; every image was closed");
        } else {
            tracing::debug!("every image was closed with no enhanced result held");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use slot::Resident;
    use tauri::Manager;

    /// A run that has finished, holding a result made from `source`.
    fn held(runs: &Runs, run: &str, source: &str, identity: &str) {
        runs.start(run, source);

        assert!(
            runs.finish(
                run,
                opai::Picture::new("/pictures/holiday.jpg", image::DynamicImage::new_rgb8(2, 2), identity)
            ),
            "the run that was just started did not keep what it produced"
        );
    }

    /// The commands take their slot off managed state, so a mock application is what makes them callable —
    /// the same shape `images::files`'s command tests use.
    fn app() -> tauri::App<tauri::test::MockRuntime> {
        tauri::test::mock_builder()
            .manage(Runs::default())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("the mock app should build")
    }

    #[test]
    fn the_release_command_reaches_the_slot_and_names_the_image() {
        let app = app();
        let runs = app.state::<Runs>();

        held(&runs, "run-1", "5ec0ffee5ec0ffee", "aaaaaaaaaaaaaaaa");

        // A photograph other than the one the result was made from: the command has to carry the name through,
        // not drop whatever is held.
        release_enhanced("facefeedfacefeed".to_string(), app.state::<Runs>(), Traceparent::default());
        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Picture(_))),
            "closing one photograph released the result belonging to another"
        );

        release_enhanced("5ec0ffee5ec0ffee".to_string(), app.state::<Runs>(), Traceparent::default());
        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Displaced)),
            "the command did not reach the slot, or the released result stopped being distinguishable from one never produced"
        );

        // Fails at nothing: a second release, and one naming a photograph nothing was ever made from.
        release_enhanced("5ec0ffee5ec0ffee".to_string(), app.state::<Runs>(), Traceparent::default());
        release_enhanced("0123456789abcdef".to_string(), app.state::<Runs>(), Traceparent::default());
    }

    #[test]
    fn the_release_everything_command_reaches_the_slot() {
        let app = app();
        let runs = app.state::<Runs>();

        held(&runs, "run-1", "5ec0ffee5ec0ffee", "aaaaaaaaaaaaaaaa");

        release_all_enhanced(app.state::<Runs>(), Traceparent::default());

        assert!(
            matches!(runs.resolve("aaaaaaaaaaaaaaaa"), Some(Resident::Displaced)),
            "the command did not reach the slot"
        );

        // Fails at nothing with nothing held, which is what closing an empty window does.
        release_all_enhanced(app.state::<Runs>(), Traceparent::default());
    }
}
