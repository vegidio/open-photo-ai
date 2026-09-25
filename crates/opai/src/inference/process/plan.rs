//! What a chain will run, decided over the whole of it before anything is installed.

use crate::error::InferenceError;
use crate::models::Operation;
use crate::pipeline::{Backend, Shared};

use super::operation_ids;

/// What one operation of a validated chain will run.
///
/// Produced by [`plan`] before anything is installed, which is what makes the refusals below structural: a run that
/// reaches this type has a family with a pipeline and a pipeline that can be built, so neither can be a failure
/// further down where a model is already on disk.
pub(crate) struct Planned<B: Backend> {
    // The operation travels beside the pipeline rather than inside it, because the two are read by different things:
    // the pipeline answers what to install, open and run, and the operation is what the records, the cache keys and
    // the progress reports name the step by.
    /// The operation, for naming it in a report, a record, a cache key or a failure.
    pub(super) operation: Operation,
    /// What it runs.
    pub(super) pipeline: Shared<B>,
}

impl<B: Backend> Planned<B> {
    /// The operation this will run, for naming it in a report, a record or a failure.
    pub(crate) fn operation(&self) -> &Operation {
        &self.operation
    }
}

/// What every operation of `operations` will run, or the first one that cannot be served.
///
/// **Checked over the whole chain before anything is installed**, which is the point of it being a pass of its own: a
/// chain whose second operation cannot be served must not first transfer gigabytes on behalf of its first. It is a
/// cheap scan over values already in memory.
///
/// # Errors
///
/// Whatever the operation's own seam refuses, unchanged and already naming the operation:
/// [`InferenceError::Unsupported`] for a family with no pipeline and for a graph set that does not declare every role
/// its pipeline addresses, and [`InferenceError::NoPasses`] where no sequence of native passes covers the requested
/// scale. Which of them applies to which operation is [`models`](crate::models)' business, not this file's.
pub(crate) fn plan<B: Backend>(operations: &[Operation]) -> Result<Vec<Planned<B>>, InferenceError> {
    let planned: Result<Vec<Planned<B>>, InferenceError> = operations.iter().map(plan_one).collect();

    // Go `inference.go`'s "operation type not supported", at the one point the whole chain is judged rather than at
    // each of the refusals — the error already names which operation and why. Written whether or not the caller
    // reports the refusal to a user, because a front end that swallowed it is exactly the case a bug report arrives
    // for.
    if let Err(error) = &planned {
        tracing::warn!(ids = %operation_ids(operations), %error, "chain refused");
    }

    planned
}

/// [`plan`] for one operation.
fn plan_one<B: Backend>(operation: &Operation) -> Result<Planned<B>, InferenceError> {
    #[cfg(test)]
    if let Some(refusal) = refused::refusal_of(operation) {
        return Err(refusal);
    }

    Ok(Planned { operation: operation.clone(), pipeline: operation.pipeline::<B>()? })
}

/// A refusal a test can force, because no shipped operation can be refused.
///
/// Upscale's `NoPasses` and Osaka's `IncompleteGraphSet` are both unreachable (see
/// `every_scale_a_user_can_ask_for_is_covered_by_a_pass_sequence`), and `Operation` is a concrete enum a test cannot
/// extend. So what a refusal records, and where, would otherwise go unchecked until one first ships. Compiled out of
/// every build but the suite's, and per thread, so a test that forces one cannot refuse a chain in another.
#[cfg(test)]
pub(super) mod refused {
    use std::cell::RefCell;

    use crate::error::{InferenceError, UnsupportedReason};
    use crate::models::Operation;

    thread_local! {
        static REFUSED: RefCell<Option<Operation>> = const { RefCell::new(None) };
    }

    /// Makes [`plan`](super::plan) refuse `operation` on this thread until the returned guard is dropped.
    pub(crate) fn refusing(operation: Operation) -> Refusing {
        REFUSED.with_borrow_mut(|refused| *refused = Some(operation));

        Refusing
    }

    /// The refusal forced for `operation`, if one is.
    pub(super) fn refusal_of(operation: &Operation) -> Option<InferenceError> {
        REFUSED.with_borrow(|refused| {
            (refused.as_ref() == Some(operation)).then(|| InferenceError::Unsupported {
                operation: operation.cache_tag(),
                reason: UnsupportedReason::IncompleteGraphSet { missing: "decoder" },
            })
        })
    }

    /// Lifts the forced refusal when dropped.
    #[must_use = "the refusal is lifted as soon as this is dropped"]
    pub(crate) struct Refusing;

    impl Drop for Refusing {
        fn drop(&mut self) {
            REFUSED.with_borrow_mut(|refused| *refused = None);
        }
    }
}
