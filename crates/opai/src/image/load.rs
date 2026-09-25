//! Reading an image file into memory.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rust_sak::crypto::xxh3_bytes;
use rust_sak::image::{decode_bytes, decode_raw_bytes};

use super::error::{self, ImageIoError};
use super::picture::Picture;
use super::raw;
use crate::task::spawn_blocking;
use crate::telemetry::unit::{Unit, unit_span};

/// Reads and decodes the image at `path`, without occupying the thread driving the caller.
///
/// See [`load_blocking`] for what it does; the two report the same result and the same errors.
///
/// # Errors
///
/// Everything [`load_blocking`] returns, plus [`ImageIoError::Cancelled`] if the runtime shut down before the read
/// could run.
pub async fn load(path: impl Into<PathBuf>) -> Result<Picture, ImageIoError> {
    let path = path.into();
    spawn_blocking::<_, ImageIoError, _>(move || load_blocking(&path)).await?
}

/// Reads and decodes the image at `path`, on the calling thread.
///
/// The result carries the pixels, the path they came from, and the XXH3-64 of the whole of the file's bytes. See
/// [`Picture`] for what that identity promises.
///
/// The format is read from the content's magic bytes, not from the file's name, so a JPEG saved as `.png` decodes as
/// the JPEG it is. Camera RAW is the exception and routes on the extension; see [`is_raw`](super::is_raw).
///
/// # What a developed RAW is
///
/// 16 bits per channel, sRGB, demosaiced and tone-mapped, then cropped and oriented as the camera recorded — the
/// photograph rather than the sensor readings. Every format behind [`save_blocking`](super::save_blocking) writes it;
/// the four that cannot hold 16 bits narrow it rather than refusing, so which ones keep the depth is what the module
/// docs spell out.
///
/// # Errors
///
/// - [`ImageIoError::UnsupportedRaw`] if the extension names one of the recognised camera RAW formats that has no
///   decoder. Checked before the file is opened, so a RAW on a disconnected drive names its format rather than an
///   I/O failure.
/// - [`ImageIoError::Read`] if the file does not exist or cannot be read.
/// - [`ImageIoError::Decode`] if its content matches no supported format, or the codec refuses it — including a RAW
///   file the decoder cannot develop, and a RAW-named file whose content is not RAW at all.
pub fn load_blocking(path: impl AsRef<Path>) -> Result<Picture, ImageIoError> {
    let path = path.as_ref();

    let span = unit_span!("image_load", path = %path.display());
    error::traced(Unit::ImageLoad, span, || read(path).inspect_err(error::unreadable("load", path)))
}

/// [`load_blocking`]'s body, which leaves recording its failure to the public boundary.
fn read(path: &Path) -> Result<Picture, ImageIoError> {
    // Which decoder this file goes to, decided from its name before the file is even opened; see `raw::route`.
    let raw_extension = raw::route(path)?;

    let bytes = std::fs::read(path).map_err(ImageIoError::read(path))?;

    // Identity and pixels both from the same buffer, which is the whole reason it is held: the hash covers every byte,
    // including any the decoder stops before, and so matches `xxh3_file` of this same file without a second pass. The
    // reference reaches the same property through a `TeeReader` into a pipe feeding the hasher plus a drain loop,
    // purely to avoid holding a 60 MB RAW in memory; the buffer here is small beside the decoded image that is about
    // to exist anyway.
    let identity = xxh3_bytes(&bytes);

    // RAW decodes from the bytes rather than from the path for the same reason: `rust-sak` offers both, and the
    // path-taking form would read a 60 MB NEF a second time or cost the identity the property above.
    //
    // Everything `decode_raw_bytes` can refuse — a camera the backend does not know, a corrupt file, or a RAW-named
    // file that is not RAW — is "read it, could not decode it", which is what `Decode` already means for a corrupt
    // JPEG. It never reports an I/O failure, having been handed bytes rather than a path.
    let decoded = if raw_extension.is_some() { decode_raw_bytes(&bytes) } else { decode_bytes(&bytes) };
    let pixels = decoded.map_err(ImageIoError::decode(path))?;

    Ok(Picture::new(path, Arc::new(pixels), identity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::test_support::{
        BLUE, DNG_FIXTURE_SIZE, FIXTURE_SIZE, GREEN, RED, channel_means, minimal_dng, on_a_dead_runtime, write_fixture,
        written_dng, written_jpeg, written_png,
    };

    use rust_sak::crypto::xxh3_file;
    use rust_sak::image::ImageError;
    use tempfile::tempdir;

    #[test]
    fn a_supported_image_is_decoded_with_its_path_and_its_identity() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "holiday.png", &written_png());

        let source = load_blocking(&path).expect("a PNG this crate wrote is decodable");

        assert_eq!(source.path(), path);
        assert_eq!(source.dimensions(), FIXTURE_SIZE);
        assert!(!source.identity().is_empty());
    }

    #[test]
    fn the_identity_is_the_same_value_as_hashing_the_file_without_decoding_it() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "holiday.png", &written_png());

        let source = load_blocking(&path).expect("decodable");

        assert_eq!(source.identity(), xxh3_file(&path).expect("the file is readable"));
    }

    #[test]
    fn the_identity_covers_bytes_the_decoder_never_reads() {
        // A decoder stops at the end of the image data; trailing metadata or padding after it is never read. The hash
        // still has to cover it, or a file listing and a loaded image disagree about a file neither of them changed.
        let dir = tempdir().expect("a temporary directory");
        let mut padded = written_png();
        padded.extend_from_slice(b"trailing bytes the PNG decoder stops before");
        let path = write_fixture(dir.path(), "padded.png", &padded);

        let source = load_blocking(&path).expect("the trailing bytes do not stop the decode");

        assert_eq!(source.identity(), xxh3_file(&path).expect("readable"));
        assert_ne!(source.identity(), xxh3_bytes(&written_png()), "the trailing bytes were not hashed");
    }

    #[test]
    fn two_files_with_the_same_bytes_and_different_names_share_one_identity() {
        let dir = tempdir().expect("a temporary directory");
        let bytes = written_png();
        let first = load_blocking(write_fixture(dir.path(), "first.png", &bytes)).expect("decodable");
        let second = load_blocking(write_fixture(dir.path(), "second.png", &bytes)).expect("decodable");

        assert_eq!(first.identity(), second.identity());
        assert_ne!(first.path(), second.path());
    }

    #[test]
    fn the_content_decides_the_format_rather_than_the_extension() {
        // A JPEG under a `.png` name. The Go application routes on content for the same reason, and a user who
        // renamed a file should still see their photograph.
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "mislabelled.png", &written_jpeg());

        let source = load_blocking(&path).expect("the JPEG content decodes despite the PNG name");

        assert_eq!(source.dimensions(), FIXTURE_SIZE);
    }

    #[test]
    fn a_file_that_is_not_an_image_is_refused_and_names_itself() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "notes.png", b"this is not an image, whatever it is called");

        let error = load_blocking(&path).expect_err("plain text is not an image");

        assert!(matches!(error, ImageIoError::Decode { .. }), "expected a decode failure, got {error:?}");
        assert!(error.to_string().contains("notes.png"), "the message did not name the file: {error}");
    }

    #[test]
    fn a_missing_file_is_refused_as_unreadable_rather_than_undecodable() {
        // Two different things to tell a user — "this file is not there" and "this file is not a photograph" — so the
        // missing file must not arrive as a decode failure.
        let dir = tempdir().expect("a temporary directory");
        let path = dir.path().join("never-written.png");

        let error = load_blocking(&path).expect_err("a file that was never written cannot be read");

        assert!(matches!(error, ImageIoError::Read { .. }), "expected a read failure, got {error:?}");
        assert!(error.to_string().contains("never-written.png"), "the message did not name the file: {error}");
    }

    #[test]
    fn a_raw_file_is_decoded_as_raw_rather_than_as_the_tiff_it_resembles() {
        // A DNG is a TIFF container, so a content sniffer would route it to the TIFF decoder, which would succeed
        // and hand back the CFA mosaic or an embedded preview. What comes back instead is the developed photograph:
        // 16-bit RGB at the sensor's size, which the TIFF path could not produce from this file.
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "DSC_0001.dng", &written_dng());

        let source = load_blocking(&path).expect("a DNG this crate wrote is decodable");

        assert_eq!(source.path(), path);
        assert_eq!(source.dimensions(), DNG_FIXTURE_SIZE);
        assert!(
            source.pixels().as_rgb16().is_some(),
            "a developed RAW is 16-bit RGB, got {:?}",
            source.pixels().color()
        );
        assert_eq!(source.identity(), xxh3_file(&path).expect("the file is readable"));
    }

    #[test]
    fn the_raw_fixture_develops_to_the_picture_it_encodes() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "red.dng", &written_dng());

        let source = load_blocking(&path).expect("decodable");

        let [red, green, blue] = channel_means(source.pixels());
        assert!(
            red > green && red > blue,
            "an RGGB sensor with its red photosites bright must develop to a red-dominant picture, \
             got r={red:.0} g={green:.0} b={blue:.0}"
        );
    }

    #[test]
    fn transposing_the_channel_order_moves_the_dominant_channel() {
        // What proves the assertion above has teeth. The same builder and the same bright photosites, differing only
        // in which colour the CFA says they are — so a decode that ignored the CFA, or a route that never reached the
        // RAW decoder at all, would produce two identical pictures and fail here.
        let dir = tempdir().expect("a temporary directory");
        let rggb = write_fixture(dir.path(), "rggb.dng", &minimal_dng(64, 64, [RED, GREEN, GREEN, BLUE], RED));
        let bggr = write_fixture(dir.path(), "bggr.dng", &minimal_dng(64, 64, [BLUE, GREEN, GREEN, RED], BLUE));

        let [red_r, _, red_b] = channel_means(load_blocking(&rggb).expect("decodable").pixels());
        let [blue_r, _, blue_b] = channel_means(load_blocking(&bggr).expect("decodable").pixels());

        assert!(red_r > red_b, "RGGB with bright red should be red-dominant");
        assert!(blue_b > blue_r, "BGGR with bright blue should be blue-dominant");
        // And stated the other way round: the red fixture's assertion applied to the transposed file fails.
        assert!(blue_r <= blue_b, "the transposed fixture must not also be red-dominant");
    }

    #[test]
    fn a_raw_format_with_no_decoder_is_refused_by_its_extension_before_its_content_is_looked_at() {
        // The file holds a genuine, decodable TIFF, which is exactly what a RAW file looks like to a content sniffer.
        // A `.gpr` must still be refused rather than decoded as that TIFF: what came back would be the mosaic or an
        // embedded preview, a wrong image reported as a success.
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "HERO0001.gpr", &crate::image::test_support::written_tiff());

        let error = load_blocking(&path).expect_err("GoPro RAW has no decoder in this backend");

        match error {
            ImageIoError::UnsupportedRaw { extension, .. } => assert_eq!(extension, "gpr"),
            other => panic!("expected an unsupported-RAW refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_decodable_raw_name_holding_something_that_is_not_raw_is_a_decode_failure() {
        // The distinction the two errors carry: `UnsupportedRaw` is about the *format*, so a NEF — a format that is
        // decoded — can only fail on its content, and does so the way a corrupt JPEG does.
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "DSC_0001.nef", &written_png());

        let error = load_blocking(&path).expect_err("PNG bytes are not a RAW file");

        match error {
            ImageIoError::Decode { source, .. } => {
                assert!(matches!(source, ImageError::NotRaw), "expected NotRaw, got {source:?}");
            }
            other => panic!("expected a decode failure, got {other:?}"),
        }
        let message = load_blocking(&path).expect_err("still not RAW").to_string();
        assert!(message.contains("DSC_0001.nef"), "the message did not name the file: {message}");
    }

    #[test]
    fn a_raw_file_the_decoder_refuses_is_a_decode_failure_rather_than_an_unsupported_format() {
        // A corrupt file and a camera the backend does not know arrive as one error from `zenraw`, which cannot tell
        // them apart. Either way it must not be reported as the format being unsupported: the next DNG in the same
        // folder opens fine, and telling a photographer DNG is unsupported would be wrong about both files.
        let dir = tempdir().expect("a temporary directory");
        let mut corrupt = written_dng();
        corrupt.truncate(corrupt.len() / 2);
        let path = write_fixture(dir.path(), "truncated.dng", &corrupt);

        let error = load_blocking(&path).expect_err("half a DNG cannot be developed");

        match error {
            ImageIoError::Decode { source, .. } => {
                assert!(matches!(source, ImageError::Raw(_)), "expected the RAW decoder's own words, got {source:?}");
            }
            other => panic!("expected a decode failure, got {other:?}"),
        }
    }

    #[test]
    fn a_genuine_tiff_still_loads() {
        // The other half of the RAW check: it must refuse NEFs without refusing the format they imitate.
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "scan.tiff", &crate::image::test_support::written_tiff());

        let source = load_blocking(&path).expect("a real TIFF is decoded normally");

        assert_eq!(source.dimensions(), FIXTURE_SIZE);
    }

    #[test]
    fn an_undecodable_raw_is_refused_before_it_is_read_at_all() {
        // Refused on the name alone, so a `.gpr` on a disconnected drive reports the format it cannot open rather
        // than an I/O failure that sends the user looking at their hardware. The directory does not exist, so a
        // check that ran after the read would report `Read` here and the user would never learn which it was.
        let error =
            load_blocking("/nowhere-at-all/HERO0001.gpr").expect_err("a RAW under a missing directory is still RAW");

        assert!(matches!(error, ImageIoError::UnsupportedRaw { .. }), "expected a RAW refusal, got {error:?}");

        // The other half of the ordering: a *decodable* RAW under the same missing directory has to be read before
        // anything can be said about it, so it is the read that fails.
        let readable =
            load_blocking("/nowhere-at-all/DSC_0001.dng").expect_err("a missing file cannot be decoded either");
        assert!(matches!(readable, ImageIoError::Read { .. }), "expected a read failure, got {readable:?}");
    }

    #[tokio::test]
    async fn a_load_through_the_async_form_is_one_span_in_its_callers_trace() {
        use tracing::Instrument as _;

        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "holiday.png", &written_png());

        let (log, observed, loaded) = crate::logging::traced_of("debug", || async {
            load(&path).instrument(tracing::info_span!("caller")).await
        })
        .await;
        loaded.expect("decodable");

        // Across the blocking hop, under the caller.
        let caller = observed.only("caller");
        let span = observed.only("image_load");
        assert_eq!(span.parent, Some(caller.index));
        assert_eq!(span.field("path"), Some(path.display().to_string().as_str()));
        assert_eq!(span.field("outcome"), Some("finished"));
        assert!(!log.contains(" outcome="), "the unit span reached the file: {log}");
    }

    #[tokio::test]
    async fn a_load_that_fails_marks_its_span_failed() {
        let dir = tempdir().expect("a temporary directory");
        let path = dir.path().join("never-written.png");

        let (_, observed, loaded) = crate::logging::traced_of("info", || async { load(&path).await }).await;
        assert!(matches!(loaded, Err(ImageIoError::Read { .. })), "{loaded:?}");

        let span = observed.only("image_load");
        assert_eq!(span.field("outcome"), Some("failed"));
        assert!(span.field("error").is_some_and(|error| error.contains("never-written.png")), "{span:?}");
        assert_eq!(observed.event("an image could not be read").and_then(|event| event.span), Some(span.index));
    }

    #[tokio::test]
    async fn the_asynchronous_form_reports_what_the_blocking_one_does() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "holiday.png", &written_png());

        let asynchronous = load(&path).await.expect("decodable");
        let blocking = load_blocking(&path).expect("decodable");

        assert_eq!(asynchronous.path(), blocking.path());
        assert_eq!(asynchronous.identity(), blocking.identity());
        assert_eq!(asynchronous.pixels(), blocking.pixels());
    }

    #[tokio::test]
    async fn the_asynchronous_form_reports_a_missing_file_the_way_the_blocking_one_does() {
        let dir = tempdir().expect("a temporary directory");
        let path = dir.path().join("never-written.png");

        let error = load(&path).await.expect_err("a file that was never written cannot be read");

        assert!(matches!(error, ImageIoError::Read { .. }), "expected a read failure, got {error:?}");
    }

    #[test]
    fn a_runtime_that_shuts_down_before_the_read_runs_reports_cancellation() {
        let outcome = on_a_dead_runtime(load("/pictures/holiday.png"));

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
        let not_an_image = write_fixture(dir.path(), "notes.png", b"this is not an image");

        let (log, outcome) = records_of_blocking("info", || load_blocking(&not_an_image));
        assert!(outcome.is_err());
        let record = the_one_failure(&log, "an image could not be read", "load");
        assert!(record.contains(&format!("path={}", as_written(&not_an_image))), "{record}");

        let (log, outcome) = recorded_async(super::load(not_an_image.clone()));
        assert!(outcome.is_err());
        the_one_failure(&log, "an image could not be read", "load");

        let readable = write_fixture(dir.path(), "holiday.png", &written_png());
        let (log, outcome) = records_of_blocking("info", || load_blocking(&readable));
        assert!(outcome.is_ok());
        assert!(!log.contains("msg="), "a success was recorded:\n{log}");
    }
}
