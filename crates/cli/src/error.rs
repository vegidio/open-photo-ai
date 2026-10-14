//! How a run can fail, the one line each failure prints, and the exit status it ends with.
//!
//! | Status | When |
//! |---|---|
//! | `0` | every image was written |
//! | `2` | a usage error: bad arguments, an unreadable named input, an output that cannot be used as asked, or no image to
//! enhance (clap's own too) |
//! | `130` | stopped by Ctrl-C |
//! | `1` | anything else: initialization, or any image that failed to decode, enhance or be written |
//!
//! A failed image is a [`CliError::Image`], whose status is its cause's. A run of one image ends with that image's own
//! line; a run of more ends with [`CliError::Batch`], each image's line having been printed already.

use std::path::{Path, PathBuf};

use opai::{ImageIoError, InferenceError, InitError};

/// Why a run, or one image of it, ended without writing its output.
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
    #[error("stopped: nothing more was written")]
    Cancelled,

    /// An image whose header read cleanly could not be decoded. The library's message names the path. Boxed, as
    /// [`Write`](Self::Write) is.
    #[error(transparent)]
    Load(Box<ImageIoError>),

    /// The result could not be written. The library's message already names the output path, so it is shown as it
    /// is. Boxed, because a codec's error would otherwise make every result in the run as large as itself.
    #[error(transparent)]
    Write(Box<ImageIoError>),

    /// The destination's directory could not be created, or its name held or kept. The filesystem's error names no
    /// path, so the destination asked for is named here.
    #[error("cannot write {}: {source}", path.display())]
    Claim {
        /// The destination asked for.
        path: PathBuf,
        /// What the filesystem refused.
        source: std::io::Error,
    },

    /// One image of the run failed. The message names the image once: the cause's alone where it already does, as a
    /// failed decode does, and otherwise prefixed with it, since an inference failure names nothing and a write names
    /// the destination.
    #[error("{}", image_reason(path, source))]
    Image {
        /// The image, as it was given or found.
        path: PathBuf,
        /// Why it failed.
        source: Box<CliError>,
    },

    /// One or more images of the batch failed, each already reported on its own line.
    #[error("{failed} of {total} images failed")]
    Batch {
        /// How many images failed.
        failed: usize,
        /// How many images the run had.
        total: usize,
    },
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

/// [`CliError::Image`]'s message.
fn image_reason(path: &Path, source: &CliError) -> String {
    let message = source.to_string();
    let image = path.display().to_string();

    if message.contains(&image) { message } else { format!("could not enhance {image}: {message}") }
}

impl CliError {
    /// The status the process exits with.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Image { source, .. } => source.exit_code(),
            Self::Usage(_) => 2,
            Self::Cancelled => 130,
            Self::Init(_)
            | Self::Inference(_)
            | Self::Load(_)
            | Self::Write(_)
            | Self::Claim { .. }
            | Self::Batch { .. } => 1,
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

    #[test]
    fn a_batch_with_failures_exits_one_saying_how_many() {
        let error = CliError::Batch { failed: 2, total: 5 };

        assert_eq!(error.exit_code(), 1);
        assert_eq!(error.to_string(), "2 of 5 images failed");
    }

    #[test]
    fn a_failed_claim_exits_one_naming_the_destination() {
        let error = CliError::Claim {
            path: "out/a.png".into(),
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        };

        assert_eq!(error.exit_code(), 1);
        assert!(error.to_string().starts_with("cannot write out/a.png: "), "{error}");
    }

    #[test]
    fn a_failed_image_names_the_image_once() {
        let path = Path::new("photos/a.jpg");
        let image = |source: CliError| CliError::Image { path: path.into(), source: Box::new(source) }.to_string();

        let decode = CliError::Load(Box::new(ImageIoError::Read {
            path: path.into(),
            source: std::io::Error::other("corrupt"),
        }));
        let reason = image(decode);
        assert_eq!(reason.matches("photos/a.jpg").count(), 1, "{reason}");
        assert!(!reason.starts_with("could not enhance"), "{reason}");

        let write = CliError::Claim {
            path: "out/a.jpg".into(),
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        };
        let reason = image(write);
        assert!(reason.starts_with("could not enhance photos/a.jpg: cannot write out/a.jpg: "), "{reason}");

        let reason = image(CliError::Inference(InferenceError::Shutdown));
        assert!(reason.starts_with("could not enhance photos/a.jpg: "), "{reason}");
    }

    #[test]
    fn a_failed_image_exits_with_its_causes_status() {
        let image = |source: CliError| CliError::Image { path: "a.jpg".into(), source: Box::new(source) };

        assert_eq!(image(CliError::Inference(InferenceError::Shutdown)).exit_code(), 1);
        assert_eq!(image(CliError::Usage("bad".into())).exit_code(), 2);
        assert_eq!(image(CliError::Cancelled).exit_code(), 130);
    }

    #[test]
    fn an_image_that_does_not_decode_exits_one() {
        let source = ImageIoError::Write { path: "a.jpg".into(), source: std::io::Error::other("corrupt") };

        assert_eq!(CliError::Load(Box::new(source)).exit_code(), 1);
    }
}
