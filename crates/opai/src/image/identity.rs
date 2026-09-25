//! What identifies an image file, read from its bytes without decoding them.

use std::path::{Path, PathBuf};

use rust_sak::crypto::xxh3_file;

use super::error::{self, ImageIoError};
use crate::task::spawn_blocking;
use crate::telemetry::unit::{Unit, unit_span};

/// The identity of the image file at `path`, without occupying the thread driving the caller.
///
/// See [`identity_blocking`] for what it does; the two report the same value and the same errors.
///
/// # Errors
///
/// Everything [`identity_blocking`] returns, plus [`ImageIoError::Cancelled`] if the runtime shut down before the
/// read could run.
pub async fn identity(path: impl Into<PathBuf>) -> Result<String, ImageIoError> {
    let path = path.into();
    spawn_blocking::<_, ImageIoError, _>(move || identity_blocking(&path)).await?
}

/// The identity of the image file at `path`, on the calling thread: the XXH3-64 of the whole of its bytes, lowercase
/// hexadecimal.
///
/// **The same value [`load_blocking`](super::load_blocking) gives for the same file**, and that equality is the whole
/// reason this exists; see [`Picture`](super::Picture) for what it lets a caller do.
///
/// # Reading every byte, decoding none of them
///
/// It still reads the file in full: the identity covers every byte, including trailing bytes a decoder stops before,
/// which is what makes it honest about the file rather than about the picture inside it. What it skips is the decode,
/// and that is where the cost is — a folder of 500 camera RAW files hashes at the price of 500 reads, where loading
/// them would also demosaic, white-balance and tone-map every one.
///
/// It is read in chunks rather than held, so a 60 MB RAW costs a chunk of memory rather than 60 MB of it.
///
/// # It says nothing about the file being an image
///
/// No format is consulted and none is checked. A file this refuses is one that could not be read at all; a file it
/// answers for may still turn out not to be a photograph, which is what [`load`](fn@super::load) is for.
///
/// # Errors
///
/// [`ImageIoError::Read`] if the file does not exist or cannot be read — the same case, reported the same way, as
/// [`load_blocking`](super::load_blocking) reports it.
pub fn identity_blocking(path: impl AsRef<Path>) -> Result<String, ImageIoError> {
    let path = path.as_ref();

    let span = unit_span!("image_identity", path = %path.display());
    error::traced(Unit::ImageIdentity, span, || {
        xxh3_file(path)
            .map_err(ImageIoError::read(path))
            .inspect_err(error::unreadable("identity", path))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::load_blocking;
    use crate::image::test_support::{on_a_dead_runtime, write_fixture, written_dng, written_png};

    use tempfile::tempdir;

    #[test]
    fn it_is_the_identity_the_loaded_image_carries() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "holiday.png", &written_png());

        assert_eq!(identity_blocking(&path).expect("readable"), load_blocking(&path).expect("decodable").identity());
    }

    #[test]
    fn it_is_the_loaded_identity_for_a_camera_raw_too() {
        // The case the cost argument is about: a RAW listing must not have to develop every file to name it.
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "DSC_0001.dng", &written_dng());

        assert_eq!(identity_blocking(&path).expect("readable"), load_blocking(&path).expect("decodable").identity());
    }

    #[test]
    fn it_covers_bytes_a_decoder_never_reads() {
        // The same property `load` promises, asserted from this side: trailing bytes after the image data are hashed,
        // so a listing and a loaded image cannot disagree about a file neither of them changed.
        let dir = tempdir().expect("a temporary directory");
        let mut padded = written_png();
        padded.extend_from_slice(b"trailing bytes the PNG decoder stops before");

        let plain = identity_blocking(write_fixture(dir.path(), "plain.png", &written_png())).expect("readable");
        let with_padding = identity_blocking(write_fixture(dir.path(), "padded.png", &padded)).expect("readable");

        assert_ne!(plain, with_padding, "the trailing bytes were not hashed");
    }

    #[test]
    fn it_is_sixteen_lowercase_hexadecimal_characters() {
        // What a rendition URL is checked against on the other side of the boundary, so the shape is worth pinning.
        let dir = tempdir().expect("a temporary directory");
        let answered = identity_blocking(write_fixture(dir.path(), "holiday.png", &written_png())).expect("readable");

        assert_eq!(answered.len(), 16, "an XXH3-64 is 16 hexadecimal characters: {answered}");
        assert!(
            answered.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "{answered}"
        );
    }

    #[test]
    fn a_file_that_is_not_an_image_still_has_one() {
        // No format is consulted, and that is the point: a listing describes what the user picked, and a file that
        // turns out not to be a photograph is still a file they picked.
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "notes.png", b"this is not an image, whatever it is called");

        assert!(identity_blocking(&path).is_ok());
        assert!(
            load_blocking(&path).is_err(),
            "the same file does not load, which is the asymmetry being asserted"
        );
    }

    #[test]
    fn a_missing_file_is_refused_the_way_loading_refuses_it() {
        let dir = tempdir().expect("a temporary directory");
        let path = dir.path().join("never-written.png");

        let error = identity_blocking(&path).expect_err("a file that was never written cannot be read");

        assert!(matches!(error, ImageIoError::Read { .. }), "expected a read failure, got {error:?}");
        assert!(error.to_string().contains("never-written.png"), "the message did not name the file: {error}");
    }

    #[tokio::test]
    async fn the_asynchronous_form_reports_what_the_blocking_one_does() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "holiday.png", &written_png());

        assert_eq!(identity(&path).await.expect("readable"), identity_blocking(&path).expect("readable"));
    }

    #[test]
    fn a_runtime_that_shuts_down_before_the_read_runs_reports_cancellation() {
        let outcome = on_a_dead_runtime(identity("/pictures/holiday.png"));

        match outcome {
            Err(ImageIoError::Cancelled) => {}
            Err(other) => panic!("expected cancellation, got {other:?}"),
            Ok(_) => panic!("the read ran against a runtime that was already gone"),
        }
    }

    #[test]
    fn a_failure_is_recorded_once_by_either_form_and_a_success_not_at_all() {
        use crate::image::test_log::{as_written, recorded_async, the_one_failure};
        use crate::logging::records_of_blocking;

        let dir = tempdir().expect("a temporary directory");
        let missing = dir.path().join("gone.png");

        let (log, outcome) = records_of_blocking("info", || identity_blocking(&missing));
        assert!(outcome.is_err());
        let record = the_one_failure(&log, "an image could not be read", "identity");
        assert!(record.contains(&format!("path={}", as_written(&missing))), "{record}");

        let (log, outcome) = recorded_async(super::identity(missing.clone()));
        assert!(outcome.is_err());
        the_one_failure(&log, "an image could not be read", "identity");

        let readable = write_fixture(dir.path(), "holiday.png", &written_png());
        let (log, outcome) = records_of_blocking("info", || identity_blocking(&readable));
        assert!(outcome.is_ok());
        assert!(!log.contains("msg="), "a success was recorded:\n{log}");
    }
}
