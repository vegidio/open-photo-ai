//! Describing an image file without decoding its pixels.

use std::path::{Path, PathBuf};

use rust_sak::image::{ImageError, ImageInfo, RawImageInfo, probe_file, probe_raw_file};

use super::error::{self, ImageIoError};
use super::raw;
use crate::task::spawn_blocking;
use crate::telemetry::unit::{Unit, unit_span};

/// Reads the header of the image at `path`, without occupying the thread driving the caller.
///
/// See [`probe_blocking`] for what it does; the two report the same result and the same errors.
///
/// # Errors
///
/// Everything [`probe_blocking`] returns, plus [`ImageIoError::Cancelled`] if the runtime shut down before the read
/// could run.
pub async fn probe(path: impl Into<PathBuf>) -> Result<ImageInfo, ImageIoError> {
    let path = path.into();
    spawn_blocking::<_, ImageIoError, _>(move || probe_blocking(&path)).await?
}

/// Reads the header of the image at `path`, on the calling thread, reporting its format, size, colour type and bit
/// depth without decoding a pixel.
///
/// This is what a file listing calls: it describes a folder of photographs for the cost of a few kilobytes each,
/// where loading them would decode every one. `bit_depth` is the true per-channel depth, which is the only thing
/// that distinguishes 10- and 12-bit AVIF and HEIF from 16-bit — `color_type` alone cannot.
///
/// It routes on the extension, where [`load_blocking`](super::load_blocking) routes on content, so a mis-named file
/// is refused here while still loading and exporting correctly.
///
/// # It does not cover camera RAW
///
/// The formats here are the ones that can also be written, and no RAW format can. A file whose name names a RAW
/// format is refused as a codec failure — the extension names no [`ImageFormat`](super::ImageFormat) — and is
/// described through [`probe_raw_blocking`] instead. [`is_raw`](super::is_raw) is how a caller chooses between them.
///
/// # Errors
///
/// - [`ImageIoError::Read`] if the file does not exist or cannot be opened — the same case, reported the same way, as
///   [`load_blocking`](super::load_blocking) reports it.
/// - [`ImageIoError::Codec`] if the extension names no supported format, including a camera RAW name, or if the
///   header cannot be parsed.
pub fn probe_blocking(path: impl AsRef<Path>) -> Result<ImageInfo, ImageIoError> {
    // The extension rather than the content, deliberately and for cost. `load_blocking` already holds every byte of
    // the file, because the identity it computes requires them, so routing on content is free there. Probing exists
    // precisely to avoid holding those bytes — `rust-sak` reads the header alone for the native formats and a bounded
    // prefix for AVIF, HEIF and WebP — so it takes the cheaper signal. What that costs is a mis-named file losing a
    // dimensions column in a listing.
    let path = path.as_ref();
    let span = unit_span!("image_probe", path = %path.display());

    error::traced(Unit::ImageProbe, span, || probe_at(path))
}

/// [`probe_blocking`]'s body, run inside its span.
fn probe_at(path: &Path) -> Result<ImageInfo, ImageIoError> {
    let probed = probe_file(path).map_err(|err| match err {
        // `rust-sak` reports "there is no such file" and "this header is malformed" through one error type, but they
        // are not one thing to whoever sees them, and a file listing — which is what probing is for — hits the first
        // constantly. Kept apart here so that a caller matching on `Read` to mean "the file is gone" is right about a
        // probe as well as a load, which is what this function's `# Errors` promises.
        ImageError::Io(source) => ImageIoError::Read { path: path.to_path_buf(), source },
        other => ImageIoError::Codec { path: path.to_path_buf(), source: other },
    });

    probed.inspect_err(error::unreadable("probe", path))
}

/// Reads the metadata of the camera RAW file at `path`, without occupying the thread driving the caller.
///
/// See [`probe_raw_blocking`] for what it does and what it costs; the two report the same result and the same
/// errors.
///
/// # Errors
///
/// Everything [`probe_raw_blocking`] returns, plus [`ImageIoError::Cancelled`] if the runtime shut down before the
/// read could run.
pub async fn probe_raw(path: impl Into<PathBuf>) -> Result<RawImageInfo, ImageIoError> {
    let path = path.into();
    spawn_blocking::<_, ImageIoError, _>(move || probe_raw_blocking(&path)).await?
}

/// Reads the metadata of the camera RAW file at `path`, on the calling thread, reporting its format, dimensions,
/// sensor bit depth, camera make and model, and whether it is a DNG — without developing a pixel.
///
/// The dimensions are the ones the file decodes to, after the crop and orientation the camera recorded, so a listing
/// built from this agrees with the picture [`load_blocking`](super::load_blocking) later produces. `bit_depth`
/// describes the *sensor* and is `None` where the file does not record it; a developed RAW is always 16-bit
/// whatever it says.
///
/// <div class="warning">
///
/// **This reads the whole file**, where [`probe_blocking`] reads a header. RAW metadata lives in IFD chains whose
/// offsets routinely point deep into a 40 MB file, so the backend probes the complete bytes. A listing over a
/// directory of RAWs therefore costs its whole size in reads, and one that finds that too expensive should show RAW
/// dimensions lazily.
///
/// </div>
///
/// # Errors
///
/// - [`ImageIoError::UnsupportedRaw`] if the extension names one of the recognised RAW formats that has no decoder.
///   Checked before the file is opened, exactly as [`load_blocking`](super::load_blocking) checks it, so the two
///   agree about a path on an unreachable drive.
/// - [`ImageIoError::Read`] if the file does not exist or cannot be read — the same failure loading it reports.
/// - [`ImageIoError::Decode`] if the file's content is not RAW, or its metadata cannot be read.
/// - [`ImageIoError::Codec`] if `path` does not name a RAW format at all, which is a caller that should have asked
///   [`is_raw`](super::is_raw) first, passed through as the codec's own refusal.
pub fn probe_raw_blocking(path: impl AsRef<Path>) -> Result<RawImageInfo, ImageIoError> {
    // Its own operation rather than part of `probe`: the two report different facts — a RAW file has a camera make
    // and model and no colour type — and they differ in cost by four orders of magnitude. One name over both would
    // tell a caller nothing about which they were paying for. The whole file rather than a bounded prefix, because a
    // prefix would miss more often than it hit.
    let path = path.as_ref();

    let span = unit_span!("image_probe_raw", path = %path.display());
    error::traced(Unit::ImageProbeRaw, span, || {
        probe_raw_at(path).inspect_err(error::unreadable("probe_raw", path))
    })
}

/// [`probe_raw_blocking`]'s body, which leaves recording its failure to the public boundary.
fn probe_raw_at(path: &Path) -> Result<RawImageInfo, ImageIoError> {
    // The same routing decision loading makes, through the same function and so necessarily in step with it. The
    // decodable extension it hands back is of no use here, because `probe_raw_file` reads the format from the path's
    // extension itself.
    raw::route(path)?;

    probe_raw_file(path).map_err(|err| match err {
        // Kept apart the way `probe_blocking` keeps them apart, and for the same reason: a missing file and a corrupt
        // one are two different things to tell a user, and a caller matching `Read` to mean "the file is gone" has to
        // be right about both probes and the load.
        ImageError::Io(source) => ImageIoError::Read { path: path.to_path_buf(), source },
        // The file was read and could not be understood — the same thing a corrupt JPEG is. `zenraw` reports an
        // unknown camera body and a corrupt file as one error and cannot separate them, so a variant claiming the
        // format is unsupported would be wrong about the next file in the same folder.
        ImageError::Raw(_) | ImageError::NotRaw => ImageIoError::Decode { path: path.to_path_buf(), source: err },
        // Only reachable by probing a path that is not RAW at all. A caller's mistake, passed through as the codec's
        // refusal rather than dressed up as an unsupported RAW format.
        other => ImageIoError::Codec { path: path.to_path_buf(), source: other },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::load::load_blocking;
    use crate::image::test_support::{
        DNG_FIXTURE_SIZE, DNG_MAKE, DNG_MODEL, FIXTURE_SIZE, on_a_dead_runtime, write_fixture, written_dng,
        written_jpeg, written_png,
    };

    use rust_sak::image::{ImageFormat, RawFormat};
    use tempfile::tempdir;

    #[test]
    fn a_file_is_described_without_being_decoded() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "holiday.png", &written_png());

        let info = probe_blocking(&path).expect("a PNG this crate wrote has a readable header");

        assert_eq!(info.format, ImageFormat::Png);
        assert_eq!((info.width, info.height), FIXTURE_SIZE);
        // The fixture is RGB with no alpha and the PNG encoder is lossless, so the channel layout the header reports
        // is the one it was handed — the header is being read, not guessed at.
        assert_eq!(info.color_type, image::ColorType::Rgb8);
        // Eight bits a channel, matching that colour type. Both are asserted because they are not the same fact; see
        // `probe_blocking`'s docs.
        assert_eq!(info.bit_depth, 8);
    }

    #[test]
    fn probing_and_loading_agree_about_the_size() {
        // The property a file listing depends on: the dimensions shown before a photograph is opened must be the ones
        // it turns out to have.
        let dir = tempdir().expect("a temporary directory");

        for (name, bytes) in [("holiday.png", written_png()), ("holiday.jpg", written_jpeg())] {
            let path = write_fixture(dir.path(), name, &bytes);

            let probed = probe_blocking(&path).expect("the header is readable");
            let loaded = load_blocking(&path).expect("decodable");

            assert_eq!((probed.width, probed.height), loaded.dimensions(), "{name} probed and loaded differently");
        }
    }

    #[test]
    fn a_missing_file_is_refused_as_unreadable_the_way_loading_refuses_it() {
        // A file listing probes constantly and meets missing files constantly, so this is the common case rather than
        // an edge one. Reporting it as a codec failure would tell a user their photograph is corrupt, and would make
        // a front end matching on `Read` correct about a load and wrong about a probe of the very same path.
        let dir = tempdir().expect("a temporary directory");
        let path = dir.path().join("never-written.png");

        let error = probe_blocking(&path).expect_err("a file that was never written has no header");

        assert!(matches!(error, ImageIoError::Read { .. }), "expected a read failure, got {error:?}");
        assert!(error.to_string().contains("never-written.png"), "the message did not name the file: {error}");

        // Named as the same thing by both operations, which is the promise the shared error type makes.
        let loading = load_blocking(&path).expect_err("the same file cannot be loaded either");
        assert!(matches!(loading, ImageIoError::Read { .. }), "loading disagreed: {loading:?}");
    }

    #[test]
    fn a_name_that_matches_no_supported_format_is_refused_as_a_codec_failure() {
        // Probing routes on the extension, so a file it cannot name a format for is refused rather than guessed at.
        // Loading the same file would still decode it, which is the asymmetry this module documents.
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "holiday.unknown", &written_png());

        let error = probe_blocking(&path).expect_err("an unrecognised extension names no format");

        // The other half of the split: the file is perfectly readable, so this is not a `Read` failure — it is the
        // codec having nothing to parse the header with.
        assert!(matches!(error, ImageIoError::Codec { .. }), "expected a codec refusal, got {error:?}");
        assert_eq!(load_blocking(&path).expect("the content is still a PNG").dimensions(), FIXTURE_SIZE);
    }

    #[tokio::test]
    async fn the_asynchronous_form_reports_what_the_blocking_one_does() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "holiday.png", &written_png());

        let asynchronous = probe(&path).await.expect("the header is readable");
        let blocking = probe_blocking(&path).expect("the header is readable");

        assert_eq!(asynchronous.format, blocking.format);
        assert_eq!((asynchronous.width, asynchronous.height), (blocking.width, blocking.height));
        assert_eq!(asynchronous.color_type, blocking.color_type);
        assert_eq!(asynchronous.bit_depth, blocking.bit_depth);
    }

    #[tokio::test]
    async fn describing_a_photograph_is_a_span_naming_the_operation() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "holiday.png", &written_png());

        let (_, observed, probed) = crate::logging::traced_of("info", || async { probe(&path).await }).await;
        probed.expect("the header is readable");

        let span = observed.only("image_probe");
        assert_eq!(span.field("path"), Some(path.display().to_string().as_str()));
        assert_eq!(span.field("outcome"), Some("finished"));
    }

    #[tokio::test]
    async fn the_asynchronous_form_refuses_a_missing_file_the_way_the_blocking_one_does() {
        let dir = tempdir().expect("a temporary directory");
        let path = dir.path().join("never-written.png");

        let error = probe(&path).await.expect_err("a file that was never written has no header");

        assert!(matches!(error, ImageIoError::Read { .. }), "expected a read failure, got {error:?}");
        assert!(error.to_string().contains("never-written.png"), "the message did not name the file: {error}");
    }

    #[test]
    fn a_raw_file_is_described_without_being_decoded() {
        // Every field the operation exists to report, asserted against a fixture whose contents are known — the make
        // and the model especially, since they are the two facts no `ImageInfo` holds and the reason this is its own
        // operation rather than a wider `probe`.
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "DSC_0001.dng", &written_dng());

        let info = probe_raw_blocking(&path).expect("a DNG this crate wrote can be described");

        // Filled in from the extension, which is the only thing that separates the TIFF-based RAW formats — and the
        // reason the path form is taken rather than the bytes one, which would leave this `None`.
        assert_eq!(info.format, Some(RawFormat::Dng));
        assert_eq!((info.width, info.height), DNG_FIXTURE_SIZE);
        assert_eq!(info.make, DNG_MAKE);
        assert_eq!(info.model, DNG_MODEL);
        assert!(info.is_dng, "the fixture carries a DNGVersion tag");
        // The sensor's depth rather than the developed picture's, which is 16-bit whatever this says. The fixture
        // records 16-bit photosites at a white level of 65535, so the backend has something to derive it from.
        assert_eq!(info.bit_depth, Some(16));
    }

    #[test]
    fn describing_and_loading_a_raw_file_agree_about_the_size() {
        // The property a file listing depends on: the dimensions shown before a photograph is opened must be the ones
        // it turns out to have. It is not free here — the backend reports the size after the crop and orientation the
        // camera recorded, which is not the sensor's own.
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "DSC_0001.dng", &written_dng());

        let probed = probe_raw_blocking(&path).expect("describable");
        let loaded = load_blocking(&path).expect("decodable");

        assert_eq!((probed.width, probed.height), loaded.dimensions());
    }

    #[test]
    fn a_missing_raw_file_is_refused_as_unreadable_the_way_loading_refuses_it() {
        let dir = tempdir().expect("a temporary directory");
        let path = dir.path().join("never-written.dng");

        let error = probe_raw_blocking(&path).expect_err("a file that was never written has no metadata");

        assert!(matches!(error, ImageIoError::Read { .. }), "expected a read failure, got {error:?}");
        assert!(error.to_string().contains("never-written.dng"), "the message did not name the file: {error}");

        // Named as the same thing by both operations, which is what lets a front end match on `Read` once.
        let loading = load_blocking(&path).expect_err("the same file cannot be loaded either");
        assert!(matches!(loading, ImageIoError::Read { .. }), "loading disagreed: {loading:?}");
    }

    #[test]
    fn a_raw_format_with_no_decoder_is_refused_before_the_file_is_read() {
        // The same refusal loading makes, in the same place: on the name, before anything is opened. The directory
        // does not exist, so a check made after the read would report `Read` and never name the format.
        let error = probe_raw_blocking("/nowhere-at-all/HERO0001.gpr").expect_err("GoPro RAW has no decoder");

        match error {
            ImageIoError::UnsupportedRaw { extension, .. } => assert_eq!(extension, "gpr"),
            other => panic!("expected an unsupported-RAW refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_raw_name_holding_something_that_is_not_raw_is_refused_as_a_decode_failure() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "DSC_0001.nef", &written_png());

        let error = probe_raw_blocking(&path).expect_err("PNG bytes carry no RAW metadata");

        assert!(matches!(error, ImageIoError::Decode { .. }), "expected a decode failure, got {error:?}");
        assert!(error.to_string().contains("DSC_0001.nef"), "the message did not name the file: {error}");
    }

    #[test]
    fn describing_a_file_that_is_not_raw_at_all_is_the_codecs_refusal() {
        // A caller that should have asked `is_raw` first. Passed through as the codec having no RAW format to read
        // rather than dressed up as an unsupported one, which would say something false about the file.
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "holiday.png", &written_png());

        let error = probe_raw_blocking(&path).expect_err("a PNG is not a RAW file");

        assert!(matches!(error, ImageIoError::Codec { .. }), "expected a codec refusal, got {error:?}");
        assert!(!crate::image::is_raw(&path), "`is_raw` is the check that would have avoided this");
    }

    #[test]
    fn probing_a_raw_name_with_the_non_raw_operation_stays_a_codec_failure() {
        // Pinned rather than inherited. `probe` routes on the extension and no `ImageFormat` names a RAW format, so
        // this is what it returns; the alternative — a "use the RAW form" variant — would widen a user-facing error
        // enum to describe a programming mistake that `is_raw` already prevents.
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "DSC_0001.dng", &written_dng());

        let error = probe_blocking(&path).expect_err("probing describes the eight writable formats");

        assert!(matches!(error, ImageIoError::Codec { .. }), "expected a codec refusal, got {error:?}");
        assert!(error.to_string().contains("DSC_0001.dng"), "the message did not name the file: {error}");
        // And the file is perfectly describable — through the operation `is_raw` points at.
        assert!(crate::image::is_raw(&path));
        assert!(probe_raw_blocking(&path).is_ok(), "the RAW operation describes what this one refused");
    }

    #[tokio::test]
    async fn the_asynchronous_raw_form_reports_what_the_blocking_one_does() {
        let dir = tempdir().expect("a temporary directory");
        let path = write_fixture(dir.path(), "DSC_0001.dng", &written_dng());

        let asynchronous = probe_raw(&path).await.expect("describable");
        let blocking = probe_raw_blocking(&path).expect("describable");

        assert_eq!(asynchronous, blocking);
    }

    #[tokio::test]
    async fn the_asynchronous_raw_form_refuses_a_missing_file_the_way_the_blocking_one_does() {
        let dir = tempdir().expect("a temporary directory");
        let path = dir.path().join("never-written.dng");

        let error = probe_raw(&path).await.expect_err("a file that was never written has no metadata");

        assert!(matches!(error, ImageIoError::Read { .. }), "expected a read failure, got {error:?}");
    }

    /// Loads and describes every RAW file in the directory named by `OPAI_RAW_SAMPLES_DIR`, and does nothing at all
    /// when that variable is unset.
    ///
    /// **Never in CI**: the files are tens of megabytes each and belong to whoever shot them.
    ///
    /// ```text
    /// OPAI_RAW_SAMPLES_DIR=~/Pictures/raw-samples cargo test -p opai -- --nocapture samples_directory
    /// ```
    #[test]
    fn every_raw_file_in_the_samples_directory_loads_and_describes() {
        // Ported from `rust-sak`'s equivalent, and here for the same reason: the synthesised DNG proves the seam, the
        // routing and the error mapping, but a hand-built DNG only exercises the path a hand-built DNG takes. What it
        // cannot show is that a real Nikon file off a real card loads through *this* crate. This is where that gets
        // checked — by a developer with a folder of camera files, on demand.
        let Ok(dir) = std::env::var("OPAI_RAW_SAMPLES_DIR") else {
            // Unset is the normal case, and it is not a failure — this test simply has nothing to look at.
            return;
        };

        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("OPAI_RAW_SAMPLES_DIR is set to {dir}, which cannot be read: {e}"));

        let mut checked = 0_usize;
        let mut refused = 0_usize;
        for entry in entries {
            let path = entry.expect("directory entry").path();
            // Skip whatever else lives in the folder — sidecars, JPEGs, subdirectories — rather than failing on it.
            if !crate::image::is_raw(&path) {
                continue;
            }

            // A recognised format with no decoder is the expected outcome for 18 of the 47, not a failure. It is
            // reported so a folder of them does not look like a passing run over nothing.
            if let Err(ImageIoError::UnsupportedRaw { extension, .. }) = load_blocking(&path) {
                eprintln!("{}: no decoder for {extension}", path.display());
                refused += 1;
                continue;
            }

            let info =
                probe_raw_blocking(&path).unwrap_or_else(|e| panic!("describing {} failed: {e}", path.display()));
            let loaded = load_blocking(&path).unwrap_or_else(|e| panic!("loading {} failed: {e}", path.display()));

            assert_eq!(
                (info.width, info.height),
                loaded.dimensions(),
                "{}: describing and loading disagree about the dimensions",
                path.display()
            );
            assert!(
                loaded.pixels().as_rgb16().is_some(),
                "{}: a developed RAW is 16-bit RGB, got {:?}",
                path.display(),
                loaded.pixels().color()
            );
            assert_eq!(loaded.identity(), rust_sak::crypto::xxh3_file(&path).expect("readable"), "{}", path.display());

            eprintln!(
                "{}: {}x{} {} {} ({}-bit sensor)",
                path.display(),
                info.width,
                info.height,
                info.make,
                info.model,
                info.bit_depth.map_or_else(|| "?".to_string(), |d| d.to_string()),
            );
            checked += 1;
        }

        // A directory with no RAW files in it is a mistake worth reporting, since the run looks green otherwise.
        assert!(
            checked + refused > 0,
            "OPAI_RAW_SAMPLES_DIR is set to {dir}, which holds no recognised RAW files"
        );
    }

    #[test]
    fn a_runtime_that_shuts_down_before_the_probe_runs_reports_cancellation() {
        let outcome = on_a_dead_runtime(probe("/pictures/holiday.png"));

        match outcome {
            Err(ImageIoError::Cancelled) => {}
            Err(other) => panic!("expected cancellation, got {other:?}"),
            Ok(info) => panic!("the probe ran against a runtime that was already gone, reporting {info:?}"),
        }
    }

    #[test]
    fn a_failed_probe_is_recorded_once_by_either_form_and_a_success_not_at_all() {
        use crate::image::test_log::{as_written, recorded_async, the_one_failure};
        use crate::logging::records_of_blocking;

        let dir = tempdir().expect("a temporary directory");
        let missing = dir.path().join("gone.png");

        let (log, outcome) = records_of_blocking("info", || probe_blocking(&missing));
        assert!(outcome.is_err());
        let record = the_one_failure(&log, "an image could not be read", "probe");
        assert!(record.contains(&format!("path={}", as_written(&missing))), "{record}");

        let (log, outcome) = recorded_async(super::probe(missing.clone()));
        assert!(outcome.is_err());
        the_one_failure(&log, "an image could not be read", "probe");

        let readable = write_fixture(dir.path(), "holiday.png", &written_png());
        let (log, outcome) = records_of_blocking("info", || probe_blocking(&readable));
        assert!(outcome.is_ok());
        assert!(!log.contains("msg="), "a success was recorded:\n{log}");
    }

    #[test]
    fn a_failed_raw_probe_is_recorded_once_by_either_form_and_a_success_not_at_all() {
        use crate::image::test_log::{as_written, recorded_async, the_one_failure};
        use crate::logging::records_of_blocking;

        let dir = tempdir().expect("a temporary directory");
        let missing = dir.path().join("DSC_0001.dng");

        let (log, outcome) = records_of_blocking("info", || probe_raw_blocking(&missing));
        assert!(outcome.is_err());
        let record = the_one_failure(&log, "an image could not be read", "probe_raw");
        assert!(record.contains(&format!("path={}", as_written(&missing))), "{record}");

        let (log, outcome) = recorded_async(super::probe_raw(missing.clone()));
        assert!(outcome.is_err());
        the_one_failure(&log, "an image could not be read", "probe_raw");

        let readable = write_fixture(dir.path(), "DSC_0002.dng", &written_dng());
        let (log, outcome) = records_of_blocking("info", || probe_raw_blocking(&readable));
        assert!(outcome.is_ok());
        assert!(!log.contains("msg="), "a success was recorded:\n{log}");
    }
}
