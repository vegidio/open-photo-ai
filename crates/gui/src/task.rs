//! Handing work to a blocking thread without leaving the command's trace.

use tauri::async_runtime::JoinHandle;

/// Runs `work` on a blocking thread, inside the span current where it was called.
///
/// `tracing`'s current span is thread-local, so without this a record emitted on the blocking thread loses the
/// command's `run` in the file, and a unit span `opai` opens there roots a trace of its own at the collector. With no
/// span current, as in the `opai://` scheme handler, the span is `Span::none()` and entering it does nothing.
///
/// `opai::task::spawn_blocking` does the same for the library's own hops. It is not reused: it is `opai`'s
/// crate-private plumbing over `tokio`, and this crate runs on Tauri's runtime handle.
pub(crate) fn spawn_blocking<F, T>(work: F) -> JoinHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    // The dispatcher is carried too, as `opai`'s helper does. In the application it is the global one on both
    // threads, so this changes nothing there; under a subscriber bound to the calling thread alone, as the tests'
    // are, a span opened on the blocking thread would otherwise go to a subscriber that never saw its parent.
    let span = tracing::Span::current();
    let dispatch = tracing::dispatcher::get_default(tracing::Dispatch::clone);

    tauri::async_runtime::spawn_blocking(move || tracing::dispatcher::with_default(&dispatch, || span.in_scope(work)))
}

#[cfg(test)]
mod tests {
    use tracing::Instrument;

    use super::*;
    use crate::test_support::recorded;

    #[test]
    fn work_on_the_blocking_thread_stays_beneath_the_callers_span() {
        let (recorded, ()) = recorded(|| {
            let caller = tracing::info_span!("caller");

            tauri::async_runtime::block_on(
                async {
                    spawn_blocking(|| {
                        let _unit = tracing::info_span!(target: "opai::unit", "image_load").entered();
                        tracing::info!("inside the unit");
                    })
                    .await
                    .expect("the work does not panic");
                }
                .instrument(caller),
            );
        });

        let caller = recorded.only("caller");
        let unit = recorded.only("image_load");
        assert_eq!(unit.parent, Some(caller.index), "the span opened on the blocking thread lost its parent");

        let [event] = recorded.events.as_slice() else {
            panic!("expected one record: {:#?}", recorded.events);
        };
        assert_eq!(event.span, Some(unit.index));
    }

    #[test]
    fn with_no_span_current_the_work_runs_as_before() {
        let (recorded, answer) = recorded(|| {
            tauri::async_runtime::block_on(async {
                spawn_blocking(|| {
                    tracing::info!("outside any span");
                    42
                })
                .await
                .expect("the work does not panic")
            })
        });

        assert_eq!(answer, 42);
        assert!(recorded.spans.is_empty());
        assert_eq!(recorded.events.len(), 1);
        assert_eq!(recorded.events[0].span, None);
    }
}
