//! Whether a newer release of the application is published, which the navbar answers with an "Update Available"
//! button.

// Here rather than in `opai`: only this front end asks. The reference's CLI never checks, and its check lives beside
// its GUI services, not in its library.
//
// Asked once per launch, by the navbar on mount; `frontend/ipc/app.ts` holds the answer so a remount does not spend a
// second request of GitHub's sixty an hour for an unauthenticated caller.

use crate::command::{Traceparent, command_span, traced};

/// The GitHub account the project's releases are published under.
const OWNER: &str = "vegidio";

/// The repository whose releases the running build is compared against. The rewrite lives here temporarily, so the
/// releases are the reference's own.
const REPO: &str = "open-photo-ai";

// The command's name is written once more, in `frontend/ipc/app.ts`.
/// Whether the latest release published on GitHub is newer than the running build.
///
/// Never an error: a check that cannot answer - no network, a rate limit, a tag that is not a version - is `false`,
/// "no newer release known", and the reason goes to the log rather than to the window, which has nothing to draw
/// for it.
#[tauri::command]
pub(crate) async fn is_outdated(traceparent: Traceparent) -> bool {
    traced(command_span!("is_outdated", traceparent), async {
        answered(rust_sak::github::is_outdated_release(OWNER, REPO, opai::version()).await)
    })
    .await
}

/// The check's result as the window reads it, logging why a failed one could not answer.
fn answered(result: Result<bool, impl std::fmt::Display>) -> bool {
    result.unwrap_or_else(|error| {
        tracing::warn!("could not check for a newer release of {OWNER}/{REPO}: {error}");

        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Captured;

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
        let log = Captured::default();
        let subscriber = tracing_subscriber::fmt().with_writer(log.clone()).finish();

        let outdated = tracing::subscriber::with_default(subscriber, || {
            answered(Err("GitHub API request failed: 403 rate limit exceeded"))
        });

        assert!(!outdated);

        let log = log.text();
        let line = log
            .lines()
            .find(|line| line.contains("WARN"))
            .unwrap_or_else(|| panic!("a failed check logged no warning: {log}"));

        assert!(line.contains("vegidio/open-photo-ai"), "the warning did not name the repository: {line}");
        assert!(line.contains("403 rate limit exceeded"), "the warning did not say why the check failed: {line}");
    }

    #[test]
    fn being_current_is_not_logged() {
        let log = Captured::default();
        let subscriber = tracing_subscriber::fmt().with_writer(log.clone()).finish();

        tracing::subscriber::with_default(subscriber, || answered(Ok::<_, String>(false)));

        assert_eq!(log.text(), "");
    }
}
