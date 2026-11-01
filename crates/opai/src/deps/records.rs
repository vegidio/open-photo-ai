//! The records an install writes: that it began, that it finished, and how one that did not finish ended.
//!
//! The installs themselves are `rust-sak`'s. What is written about them, and at which level, is this application's.

use std::time::Duration;

use crate::error::InitError;

/// Records that an install has begun moving or hashing files.
pub(crate) fn started(dependency: &str, version: &str, path: &std::path::Path) {
    tracing::info!(dependency, version, path = %path.display(), "installing a dependency");
}

/// Records that an install finished with its files in place, and how long it took.
pub(crate) fn ready(dependency: &str, version: &str, duration: Duration) {
    tracing::info!(dependency, version, ?duration, "the dependency is ready");
}

/// Records how an install that did not finish ended: stopped, or failed.
pub(crate) fn record_ended(dependency: &str, version: &str, duration: Duration, error: &InitError) {
    // A stop is not a failure and is recorded as what it is: nobody is left waiting, or the process is on its way
    // down, and a `warn` here would put an install failure in the file of every user who changed their mind. `info`,
    // because the install opened at `info` and a bracket closes at the level it opened.
    if let Some(reason) = error.stop_reason() {
        tracing::info!(dependency, version, reason, ?duration, "an install stopped");

        return;
    }

    // `warn` naming the dependency, which is the fact a bug report about an install that never finishes is missing.
    tracing::warn!(dependency, version, ?duration, %error, "installing a dependency failed");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging;

    #[test]
    fn a_stop_is_recorded_as_a_stop_and_a_failure_as_a_warning() {
        // Through the recorder rather than a real shutdown: `InitError::Cancelled` is what `spawn_blocking` answers
        // when the runtime is dropped under it, and a test cannot drop the runtime it is running on and still read
        // its own records.
        let (log, ()) = logging::records_of_blocking("info", || {
            record_ended("up_kyoto_4x_fp32", "abc", Duration::from_millis(250), &InitError::Stopped);
            record_ended("up_kyoto_4x_fp32", "abc", Duration::from_millis(250), &InitError::Cancelled);
            record_ended(
                "up_kyoto_4x_fp32",
                "abc",
                Duration::from_millis(250),
                &InitError::UnpublishedModel { artifact: "up_kyoto_4x_fp32".to_string() },
            );
        });

        let stopped: Vec<&str> = log.lines().filter(|line| line.contains("an install stopped")).collect();
        assert_eq!(stopped.len(), 2, "{log}");
        assert!(stopped.iter().all(|line| line.contains("level=INFO")), "{log}");
        assert!(stopped[0].contains("cancelled") && stopped[1].contains("shutdown"), "{log}");
        assert!(stopped[0].contains("250ms"), "{log}");

        let failed = log.lines().find(|line| line.contains("installing a dependency failed")).unwrap();
        assert!(failed.contains("level=WARN") && failed.contains("dependency=up_kyoto_4x_fp32"), "{failed}");
    }
}
