//! How a run can fail, the one line each failure prints, and the exit status it ends with.
//!
//! | Status | When |
//! |---|---|
//! | `0` | the output was written |
//! | `2` | a usage error: bad arguments, an unreadable input, or an output format nothing writes (clap's own too) |
//! | `130` | stopped by Ctrl-C |
//! | `1` | anything else: initialization, inference, or writing the file |

use opai::{ImageIoError, InferenceError, InitError};

/// Why a run ended without writing its output.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// The command line, the input or the output's format was refused. The message names the path or the argument.
    #[error("{0}")]
    Usage(String),

    /// The library could not be initialized, including another instance holding the claim.
    #[error(transparent)]
    Init(#[from] InitError),

    /// The enhancement failed.
    #[error(transparent)]
    Inference(InferenceError),

    /// The user pressed Ctrl-C.
    #[error("stopped: nothing was written")]
    Cancelled,

    /// The result could not be written. The library's message already names the output path, so it is shown as it
    /// is. Boxed, because a codec's error would otherwise make every result in the run as large as itself.
    #[error(transparent)]
    Write(Box<ImageIoError>),
}

impl From<InferenceError> for CliError {
    /// A stop is the user's own and ends as one; every other failure of a run is the run's.
    fn from(error: InferenceError) -> Self {
        match error {
            InferenceError::Cancelled => Self::Cancelled,
            other => Self::Inference(other),
        }
    }
}

impl CliError {
    /// The status the process exits with.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Usage(_) => 2,
            Self::Cancelled => 130,
            Self::Init(_) | Self::Inference(_) | Self::Write(_) => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opai::Holder;

    #[test]
    fn a_usage_error_exits_two() {
        assert_eq!(CliError::Usage("bad".into()).exit_code(), 2);
    }

    #[test]
    fn another_instance_exits_one_with_the_librarys_message() {
        let refusal = InitError::AlreadyRunning { holder: Some(Holder { app: opai::GUI.into(), pid: 4821 }) };
        let expected = refusal.to_string();

        let error = CliError::from(refusal);

        assert_eq!(error.exit_code(), 1);
        assert_eq!(error.to_string(), expected);
        assert!(expected.contains("gui, pid 4821"), "{expected}");
    }

    #[test]
    fn a_stopped_run_exits_one_hundred_thirty() {
        let error = CliError::from(InferenceError::Cancelled);

        assert!(matches!(error, CliError::Cancelled));
        assert_eq!(error.exit_code(), 130);
    }

    #[test]
    fn any_other_inference_failure_exits_one() {
        let error = CliError::from(InferenceError::Shutdown);

        assert!(matches!(error, CliError::Inference(InferenceError::Shutdown)));
        assert_eq!(error.exit_code(), 1);
    }

    #[test]
    fn a_failed_write_exits_one_naming_the_output() {
        let source = ImageIoError::Write {
            path: "missing/out.png".into(),
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
        };

        let error = CliError::Write(Box::new(source));

        assert_eq!(error.exit_code(), 1);
        let message = error.to_string();
        assert_eq!(message.matches("missing/out.png").count(), 1, "the path is named exactly once: {message}");
        assert!(!message.contains('\n'), "{message}");
    }
}
