//! Running blocking work without stalling the async runtime.

use std::future::{Future, poll_fn};
use std::pin::pin;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::task::Poll;

use thiserror::Error;

// A marker rather than a variant of any one error type: every module that runs work off the runtime can be cancelled
// this way, and each has its own error to report it as. It is the bound `spawn_blocking` places on that error, so a
// caller writes the conversion once and the helper stays independent of who calls it.
/// The async runtime shut down before work handed to a blocking thread could run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("cancelled: the runtime shut down before the work could run")]
pub(crate) struct Cancelled;

/// Runs `work` on a blocking thread, re-raising a panic there as a panic here rather than turning it into an error.
///
/// The error type is the caller's, chosen by inference from where the result goes, and constrained only by being able
/// to represent [`Cancelled`].
///
/// # Errors
///
/// Returns `E::from(Cancelled)` if the runtime shut down before the work could run.
pub(crate) async fn spawn_blocking<T, E, F>(work: F) -> Result<T, E>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
    E: From<Cancelled>,
{
    // Everything this crate does off the async runtime goes through here. In the GUI that runtime is shared with the
    // window and the IPC plumbing, so work that blocks it does not merely slow this crate down — it stops the event
    // loop. The error type is left to the caller because fixing it to one would mean every module off the runtime
    // reporting a failure in initialization's words.
    //
    // `tracing`'s current span is thread-local, so the caller's is carried across by hand: without it a record emitted
    // on the blocking thread loses its span's fields in the file and its trace at the collector. With no span or no
    // subscriber this is `Span::none()`, and entering it does nothing.
    //
    // The dispatcher is carried too. In an application that is the global one on both threads, so this changes
    // nothing there. Under a subscriber bound to the calling thread alone, as the suite's recordings are, a unit span
    // opened on the blocking thread would otherwise go to a subscriber that never saw its parent.
    let span = tracing::Span::current();
    let dispatch = tracing::dispatcher::get_default(tracing::Dispatch::clone);
    let work = move || tracing::dispatcher::with_default(&dispatch, || span.in_scope(work));
    match tokio::task::spawn_blocking(work).await {
        Ok(value) => Ok(value),
        Err(err) if err.is_panic() => std::panic::resume_unwind(err.into_panic()),
        // The only non-panic `JoinError`: the runtime was dropped or shut down before the task could be polled. Its own
        // case rather than an I/O error over the `JoinError`: nothing on disk failed, and a caller that can tell the
        // two apart can stop quietly instead of reporting a broken install to a user already closing the application.
        Err(_) => Err(E::from(Cancelled)),
    }
}

/// Takes `mutex`, treating a poisoned lock as readable.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // One rule for the whole crate, because every mutex in it guards the same kind of thing: a counter, a map the
    // holder owns outright, or an ordering. A caller that panicked while holding one leaves none of those in a state
    // the next caller cannot use, and propagating the panic instead would turn one failed build — or one panicking
    // progress callback — into an application that can no longer open any model or report any progress at all.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// Written here rather than taken from `tokio::join!`, which needs the `macros` feature this crate's runtime dependency
// does not enable, or from `futures`, which is not in its dependency graph.
/// Runs `a` and `b` concurrently on the current task and returns both outputs once both have finished.
pub(crate) async fn join<A: Future, B: Future>(a: A, b: B) -> (A::Output, B::Output) {
    let (mut a, mut b) = (pin!(a), pin!(b));
    let (mut first, mut second) = (None, None);

    poll_fn(|cx| {
        if first.is_none()
            && let Poll::Ready(output) = a.as_mut().poll(cx)
        {
            first = Some(output);
        }
        if second.is_none()
            && let Poll::Ready(output) = b.as_mut().poll(cx)
        {
            second = Some(output);
        }

        match (first.take(), second.take()) {
            (Some(a), Some(b)) => Poll::Ready((a, b)),
            (a, b) => {
                (first, second) = (a, b);
                Poll::Pending
            }
        }
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    // The helper names no error type, so the test has to choose one. `InitError` makes this also the check that its
    // `From<Cancelled>` reaches the right variant.
    use crate::error::InitError;

    #[test]
    fn work_handed_to_a_runtime_that_is_shutting_down_is_cancelled_rather_than_an_io_failure() {
        // A runtime that is already gone, which is what a process on its way down leaves behind. Nothing on disk
        // failed, so reporting an I/O error against a path that was never touched would be an invention.
        //
        // The runtime is shut down *before* the work is handed to it rather than racing a shutdown against a task
        // already in flight: the outcome then does not depend on which won.
        let shut_down = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
        let handle = shut_down.handle().clone();
        shut_down.shutdown_timeout(std::time::Duration::ZERO);

        // `spawn_blocking` targets whichever runtime is current, so the call is made in the dead one's context and
        // the resulting future is driven on a live one — a dead runtime cannot poll anything itself.
        let outcome = tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
            let _entered = handle.enter();
            spawn_blocking(|| 42).await
        });

        match outcome {
            Err(InitError::Cancelled) => {}
            Ok(value) => panic!("the work ran anyway and returned {value}"),
            Err(other) => panic!("expected Cancelled, got {other:?}"),
        }
    }

    #[tokio::test]
    #[should_panic(expected = "the blocking work panicked")]
    async fn a_panicking_closure_still_panics_here_rather_than_becoming_an_error() {
        // Neither `Cancelled` nor `Io`: a panic is a defect in this crate, and turning it into a value a caller can
        // ignore would hide it behind a message about the disk or the shutdown.
        // Annotated only because the result is discarded and nothing else constrains the error type; which one it is
        // does not matter here, since the panic path never builds one.
        let _: Result<(), InitError> = spawn_blocking(|| panic!("the blocking work panicked")).await;
    }

    #[tokio::test]
    async fn a_record_from_the_blocking_thread_carries_the_span_it_was_handed_off_under() {
        use tracing::Instrument as _;

        use crate::logging::{records, records_of};

        // Through `records_of`, and so under the suite's recording lock, like every other test that binds the
        // two-layer subscriber: otherwise its records could reach the collector while `telemetry`'s export test runs.
        let (log, _) = records_of("info", || async {
            // The blocking thread is bound to the recording's subscriber, as every thread is to the one
            // `logging::init` installs globally: what is under test is whether the span crossed, not which subscriber
            // is current.
            let recording = tracing::dispatcher::get_default(tracing::Dispatch::clone);
            let _: Result<(), InitError> = spawn_blocking(move || {
                tracing::dispatcher::with_default(&recording, || tracing::info!("decoded on the blocking thread"));
            })
            .instrument(tracing::info_span!("decode", picture = "a.jpg"))
            .await;
        })
        .await;

        let lines = records(&log, "decoded on the blocking thread");
        assert_eq!(lines.len(), 1, "{log}");
        assert!(lines[0].contains(" picture=a.jpg"), "the record lost the span it was handed off under: {log}");
    }

    #[tokio::test]
    async fn join_returns_both_outputs_when_the_first_finishes_last() {
        let slow = async {
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
            1
        };

        assert_eq!(join(slow, async { "b" }).await, (1, "b"));
    }
}
