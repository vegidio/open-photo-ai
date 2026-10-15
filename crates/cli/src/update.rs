//! Whether a newer release of the application is published, which the run answers with the `💡` line before anything
//! else it prints.

// The same check the GUI's navbar makes (`gui/src/update.rs`), asked once per run. Started at launch, beside the
// output and input checks and the directory walk, and awaited under a deadline, so a slow or absent network costs at
// most `WAIT` and never the run.

use std::fmt::Display;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time::Instant;
use tracing::Instrument;

use crate::analytics;

/// The GitHub account the project's releases are published under.
const OWNER: &str = "vegidio";

/// The repository whose releases the running build is compared against.
const REPO: &str = "open-photo-ai";

/// How long after launch an answer is still waited for. Later, the run goes on without the line.
pub const WAIT: Duration = Duration::from_millis(1500);

/// Starts asking GitHub whether its latest release is newer than the running build.
///
/// Never an error: a check that cannot answer - no network, a rate limit, a tag that is not a version - is `false`,
/// "no newer release known", and the reason goes to the log.
pub fn start() -> JoinHandle<bool> {
    // Under the run's span, as the GUI's command of the same name is under its window's.
    let span = tracing::info_span!(
        target: analytics::TARGET,
        "is_outdated",
        outcome = tracing::field::Empty,
        error = tracing::field::Empty
    );

    tokio::spawn(
        async move {
            let outdated = answered(rust_sak::github::is_outdated_release(OWNER, REPO, opai::version()).await);
            tracing::Span::current().record("outcome", "finished");

            outdated
        }
        .instrument(span),
    )
}

/// Whether `check` answered that a newer release is published by `deadline`. A check still running then, or one that
/// ended without an answer, is `false`, and why goes to the log.
pub async fn outdated<E: Display>(check: impl Future<Output = Result<bool, E>>, deadline: Instant) -> bool {
    match tokio::time::timeout_at(deadline, check).await {
        Ok(Ok(outdated)) => outdated,
        Ok(Err(error)) => {
            tracing::warn!("the check for a newer release of {OWNER}/{REPO} ended without an answer: {error}");
            false
        }
        Err(_) => {
            tracing::warn!("the check for a newer release of {OWNER}/{REPO} did not answer within {WAIT:?}");
            false
        }
    }
}

/// The check's result as the run reads it, logging why a failed one could not answer.
fn answered(result: Result<bool, impl Display>) -> bool {
    result.unwrap_or_else(|error| {
        tracing::warn!("could not check for a newer release of {OWNER}/{REPO}: {error}");

        false
    })
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use tracing::Level;

    use super::*;
    use crate::test_support::recorded;

    fn message(recorded: &crate::test_support::Recorded) -> String {
        let [warning] = recorded.events.as_slice() else {
            panic!("expected one record: {:#?}", recorded.events)
        };
        assert_eq!(warning.level, Level::WARN);

        warning.field("message").unwrap_or_default().to_string()
    }

    #[test]
    fn the_releases_compared_against_are_the_projects() {
        assert_eq!(OWNER, "vegidio");
        assert_eq!(REPO, "open-photo-ai");
    }

    #[test]
    fn an_answer_crosses_as_it_is() {
        assert!(answered(Ok::<_, String>(true)));
        assert!(!answered(Ok::<_, String>(false)));
    }

    #[test]
    fn a_failed_check_is_no_newer_release_known_and_is_logged() {
        let (recorded, outdated) = recorded(|| answered(Err("GitHub API request failed: 403 rate limit exceeded")));

        assert!(!outdated);

        let line = message(&recorded);
        assert!(line.contains("vegidio/open-photo-ai"), "the warning did not name the repository: {line}");
        assert!(line.contains("403 rate limit exceeded"), "the warning did not say why the check failed: {line}");
    }

    #[test]
    fn being_current_is_not_logged() {
        let (recorded, _) = recorded(|| answered(Ok::<_, String>(false)));

        assert!(recorded.events.is_empty(), "{:#?}", recorded.events);
    }

    #[test]
    fn an_answer_in_time_is_the_answer() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().expect("runtime");
        let deadline = || Instant::now() + WAIT;

        assert!(runtime.block_on(outdated(async { Ok::<_, Infallible>(true) }, deadline())));
        assert!(!runtime.block_on(outdated(async { Ok::<_, Infallible>(false) }, deadline())));
    }

    #[test]
    fn a_check_past_the_deadline_is_no_newer_release_known_and_is_logged() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().expect("runtime");

        let (recorded, outdated) = recorded(|| {
            runtime.block_on(outdated(
                std::future::pending::<Result<bool, Infallible>>(),
                Instant::now() + Duration::from_millis(10),
            ))
        });

        assert!(!outdated);
        assert!(message(&recorded).contains("did not answer"), "{:#?}", recorded.events);
    }

    #[test]
    fn a_check_that_ended_without_an_answer_is_no_newer_release_known_and_is_logged() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().expect("runtime");

        let (recorded, outdated) =
            recorded(|| runtime.block_on(outdated(async { Err::<bool, _>("task panicked") }, Instant::now() + WAIT)));

        assert!(!outdated);
        assert!(message(&recorded).contains("task panicked"), "{:#?}", recorded.events);
    }
}
