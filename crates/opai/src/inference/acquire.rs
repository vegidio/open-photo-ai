//! Opening every session a planned operation needs, before any of it runs.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::progress::OperationProgress;
use crate::error::InferenceError;
use crate::pipeline::{Backend, Model};
use crate::providers::ExecutionProvider;
use crate::sessions::{Interest, SessionHandle};

/// Opens every session `model` needs, reporting whatever of it had to be installed over the head of `reporter`'s
/// range.
///
/// **Every session before any of the operation runs**, rather than one at a time. The handles come back in the order
/// [`Model::sessions`] named them, which is how a pipeline addresses them. **Ownership stays with the caller**: the
/// driver holds every handle for the whole of its call, which is what makes "a session a run holds cannot be
/// reclaimed" a property of the driver's own reference counts rather than a rule any code here follows.
///
/// # Errors
///
/// [`InferenceError`], folded from the session layer's own two outcomes: a model that could not be put on disk is
/// retried, and one that is on disk and would not open is a broken install.
pub(crate) async fn acquire<B: Backend>(
    backend: &B,
    model: &dyn Model<B>,
    reporter: &Arc<OperationProgress>,
    provider: ExecutionProvider,
    cancel: &CancellationToken,
) -> Result<Vec<SessionHandle<B::Session>>, InferenceError> {
    // Written once for both drivers, which would otherwise be identical line for line here, and it carries the
    // invariant worth writing once: no profile is ever in scope that is not already attached to the artifact it was
    // measured for. How many sessions, and what each is opened under, is the model's answer and arrives as pairs, so
    // this loop has no branch and cannot open one model under another's configuration; see `Model::sessions`.
    //
    // Every session before any of the operation runs, because an install reported after the first model had already
    // run would send the bar back to the head of the operation's range.
    //
    // In `inference` rather than beside the traits in `pipeline`, because both of its callers are drivers and it
    // reports through the chain's own `OperationProgress`: a model never acquires a session, it is handed them.
    let installing = reporter.installing(model.required());

    // What this run brings to each install: its reports, and its stop. See `sessions::Interest`.
    let interest = Interest { on_progress: installing, cancel: cancel.clone() };

    let needed = model.sessions();
    let mut sessions = Vec::with_capacity(needed.len());
    for (artifact, profile) in needed {
        let handle = backend.acquire(artifact, profile, provider, &interest).await.map_err(InferenceError::from)?;

        sessions.push(handle);
    }

    Ok(sessions)
}
