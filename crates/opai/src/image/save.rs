//! Writing an image out to a file.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ::image::DynamicImage;
use rust_sak::image::{EncodeOptions, ImageError, ImageFormat, encode_writer};

use super::error::{self, ImageIoError};
use crate::task::spawn_blocking;
use crate::telemetry::unit::{Unit, unit_span};

/// Writes `pixels` to `destination` as `format`, without occupying the thread driving the caller.
///
/// [`Picture::shared_pixels`](super::Picture::shared_pixels) is where the [`Arc`] comes from, a refcount bump rather
/// than a copy.
///
/// See [`save_blocking`] for what it does; the two report the same result and the same errors.
///
/// # Errors
///
/// Everything [`save_blocking`] returns, plus [`ImageIoError::Cancelled`] if the runtime shut down before the write
/// could run.
pub async fn save(
    pixels: Arc<DynamicImage>,
    destination: impl Into<PathBuf>,
    format: ImageFormat,
    options: Option<EncodeOptions>,
) -> Result<u64, ImageIoError> {
    let destination = destination.into();
    // An `Arc` because `spawn_blocking` requires `'static`, and nothing borrowed can cross onto a blocking thread.
    spawn_blocking::<_, ImageIoError, _>(move || save_blocking(&pixels, &destination, format, options)).await?
}

/// Writes `pixels` to `destination` as `format`, on the calling thread, and reports the bytes written.
///
/// # The destination and the format are both the caller's
///
/// `destination` is a parameter rather than a property of the image, so writing a result never overwrites its source
/// unless the caller names the source.
///
/// `format` governs the bytes, and the destination's **name is never consulted**. Writing a JPEG to a file called
/// `.png` produces JPEG bytes.
///
/// `options` may be omitted for the format's defaults. Settings that target another format are refused, and the
/// refusal happens before the destination is touched — see below.
///
/// # Errors
///
/// - [`ImageIoError::Codec`] if `options` target a format other than `format`. Nothing is written, and a file
///   already at the destination is left exactly as it was.
/// - [`ImageIoError::Write`] if the destination cannot be created: its directory does not exist, or it is not
///   writable.
/// - [`ImageIoError::Codec`] if the encoder refuses, or a write fails partway through. A destination truncated by a
///   failure of this kind is **not** restored; only the mismatch above is checked early enough to promise that.
pub fn save_blocking(
    pixels: &DynamicImage,
    destination: impl AsRef<Path>,
    format: ImageFormat,
    options: Option<EncodeOptions>,
) -> Result<u64, ImageIoError> {
    // The reference writes to the image's own path field instead of taking a destination, which forces every call
    // site to build a throwaway image carrying the *output* path and an empty identity.
    let destination = destination.as_ref();

    let span = unit_span!("image_save", path = %destination.display(), format = ?format);
    error::traced(Unit::ImageSave, span, || {
        write(pixels, destination, format, options).inspect_err(error::unwritable(destination, format))
    })
}

/// [`save_blocking`]'s body, which leaves recording its failure to the public boundary.
fn write(
    pixels: &DynamicImage,
    destination: &Path,
    format: ImageFormat,
    options: Option<EncodeOptions>,
) -> Result<u64, ImageIoError> {
    // Before the destination is opened, and deliberately duplicating a check `encode_writer` also makes. It makes the
    // same one too late to matter: it is handed a writer, so by the time it refuses, `File::create` has already
    // truncated whatever was at the destination — and a user re-exporting over a previous export with a mis-paired
    // setting would lose that export and get an error. Checking here is what makes "nothing is written" true.
    if let Some(options) = options
        && options.format() != format
    {
        let source = ImageError::FormatMismatch { expected: format, options: options.format() };
        return Err(ImageIoError::Codec { path: destination.to_path_buf(), source });
    }

    let file = std::fs::File::create(destination).map_err(ImageIoError::write(destination))?;
    // Counted on the way through rather than read back with a `metadata` call on the finished file, because it is
    // what was actually encoded, not what the filesystem happens to report afterwards — and it costs an addition per
    // write.
    let mut counted = Counted { inner: std::io::BufWriter::new(file), written: 0 };

    // To a writer rather than through `rust_sak::image::encode_file`, which routes on the extension and would silently
    // ignore the format it was given whenever the two disagreed — and the disagreement is precisely the case worth
    // being honest about.
    encode_writer(pixels, &mut counted, format, options).map_err(ImageIoError::codec(destination))?;

    // The encoder is finished, but a `BufWriter` is not: up to its buffer of encoded image is still in memory, and
    // dropping it would swallow whatever the flush reported. A disk that filled on the last write is a failed save.
    counted.inner.flush().map_err(ImageIoError::write(destination))?;

    Ok(counted.written)
}

/// A sink that counts what passes through it on its way to `inner`.
struct Counted<W: Write> {
    inner: W,
    written: u64,
}

impl<W: Write> Write for Counted<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buf)?;
        // The accepted count, not the offered one: a sink is free to take less than it was given.
        self.written += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::load::load_blocking;
    use crate::image::test_support::{FIXTURE_SIZE, on_a_dead_runtime, sample_image, write_fixture, written_png};

    use rust_sak::crypto::xxh3_file;
    use tempfile::tempdir;

    #[test]
    fn an_image_survives_being_written_and_read_back() {
        let dir = tempdir().expect("a temporary directory");
        let destination = dir.path().join("out.png");

        let written = save_blocking(&sample_image(), &destination, ImageFormat::Png, None).expect("PNG is writable");

        assert!(written > 0, "nothing was written but the save reported success");
        assert_eq!(written, std::fs::metadata(&destination).expect("the file exists").len());

        let read_back = load_blocking(&destination).expect("what was just written is decodable");
        assert_eq!(read_back.dimensions(), FIXTURE_SIZE);
        // Lossless both ways, so the pixels must come back identical rather than merely similar.
        assert_eq!(read_back.pixels(), &sample_image());
    }

    #[test]
    fn writing_somewhere_else_leaves_the_source_file_untouched() {
        // The reason the destination is a parameter: the export queue writes to a directory the user chose, and a
        // save that overwrote the original would destroy the photograph it was enhancing.
        let dir = tempdir().expect("a temporary directory");
        let source = write_fixture(dir.path(), "original.png", &written_png());
        let before = xxh3_file(&source).expect("the source is readable");

        let loaded = load_blocking(&source).expect("decodable");
        save_blocking(loaded.pixels(), dir.path().join("enhanced.png"), ImageFormat::Png, None).expect("writable");

        assert_eq!(xxh3_file(&source).expect("the source is still readable"), before);
        assert_eq!(loaded.path(), source, "the loaded image's path followed the output");
    }

    #[test]
    fn the_requested_format_decides_the_bytes_rather_than_the_destinations_name() {
        // Asked for JPEG, named `.png`.
        let dir = tempdir().expect("a temporary directory");
        let destination = dir.path().join("mislabelled.png");

        save_blocking(&sample_image(), &destination, ImageFormat::Jpeg, None).expect("writable");

        let bytes = std::fs::read(&destination).expect("readable");
        assert_eq!(
            rust_sak::image::format_from_bytes(&bytes).expect("the bytes are a recognised image"),
            ImageFormat::Jpeg,
            "the destination's name changed the format that was written"
        );
    }

    #[test]
    fn settings_targeting_another_format_are_refused_and_nothing_is_written() {
        // A silently ignored setting is worse than a refusal: a user who asked for maximum quality and got the
        // default has no way to tell.
        let dir = tempdir().expect("a temporary directory");
        let destination = dir.path().join("out.jpg");

        let error = save_blocking(
            &sample_image(),
            &destination,
            ImageFormat::Jpeg,
            Some(EncodeOptions::default_for(ImageFormat::Png)),
        )
        .expect_err("PNG settings do not tune a JPEG");

        assert!(matches!(error, ImageIoError::Codec { .. }), "expected a codec refusal, got {error:?}");
        assert!(error.to_string().contains("out.jpg"), "the message did not name the destination: {error}");
        // Nothing written means no file either: a refused save must not leave an empty one behind for a user to find.
        assert!(!destination.exists(), "a refused save created the destination anyway");
    }

    #[test]
    fn a_refused_save_leaves_an_existing_destination_exactly_as_it_was() {
        // The case `save_blocking`'s early mismatch check exists for, and the one that costs a user something real:
        // re-exporting over a previous export with a mis-paired setting.
        let dir = tempdir().expect("a temporary directory");
        let destination = dir.path().join("out.jpg");

        save_blocking(&sample_image(), &destination, ImageFormat::Jpeg, None).expect("writable");
        let before = std::fs::read(&destination).expect("readable");
        assert!(!before.is_empty(), "the fixture export wrote nothing");

        let error = save_blocking(
            &sample_image(),
            &destination,
            ImageFormat::Jpeg,
            Some(EncodeOptions::default_for(ImageFormat::Png)),
        )
        .expect_err("PNG settings do not tune a JPEG");

        assert!(matches!(error, ImageIoError::Codec { .. }), "expected a codec refusal, got {error:?}");
        assert_eq!(
            std::fs::read(&destination).expect("the earlier export is still readable"),
            before,
            "the refused save destroyed the export already at the destination"
        );
    }

    #[test]
    fn omitting_the_settings_encodes_with_the_formats_own_defaults() {
        // `None` is not "no settings" but "the format's defaults", which `rust-sak` resolves through
        // `EncodeOptions::default_for`. Every other test here passes `None` and checks only that what came back
        // decodes — which a silently ignored or substituted default would survive. Asserted byte for byte instead,
        // against the defaults spelled out, so a change to either side has to be deliberate.
        let dir = tempdir().expect("a temporary directory");
        let image = sample_image();

        // The two formats with defaults worth drifting: JPEG's quality and PNG's compression and filter. The others
        // have no tunable parameters, so equality there would prove nothing.
        for format in [ImageFormat::Jpeg, ImageFormat::Png] {
            let omitted = dir.path().join(format!("omitted.{}", format.extension()));
            let spelled_out = dir.path().join(format!("spelled-out.{}", format.extension()));

            save_blocking(&image, &omitted, format, None).expect("writable");
            save_blocking(&image, &spelled_out, format, Some(EncodeOptions::default_for(format))).expect("writable");

            assert_eq!(
                std::fs::read(&omitted).expect("readable"),
                std::fs::read(&spelled_out).expect("readable"),
                "{format:?} encoded differently with its defaults omitted than with them spelled out"
            );
        }
    }

    #[test]
    fn supplied_settings_reach_the_encoder() {
        // The check that `options` is applied rather than accepted and dropped: the same picture at two qualities has
        // to produce two different files, and the lower one the smaller.
        let dir = tempdir().expect("a temporary directory");
        let image = sample_image();

        let coarse = save_blocking(
            &image,
            dir.path().join("coarse.jpg"),
            ImageFormat::Jpeg,
            Some(EncodeOptions::Jpeg { quality: 20 }),
        )
        .expect("writable");
        let fine = save_blocking(
            &image,
            dir.path().join("fine.jpg"),
            ImageFormat::Jpeg,
            Some(EncodeOptions::Jpeg { quality: 95 }),
        )
        .expect("writable");

        assert!(coarse < fine, "quality 20 produced {coarse} bytes and quality 95 produced {fine}");
        // And both are still the picture, not merely two differently sized files.
        for name in ["coarse.jpg", "fine.jpg"] {
            let read_back = load_blocking(dir.path().join(name)).expect("decodable");
            assert_eq!(read_back.dimensions(), FIXTURE_SIZE, "{name} did not decode to the picture");
        }
    }

    #[test]
    fn a_destination_that_cannot_be_created_is_refused_and_names_itself() {
        let dir = tempdir().expect("a temporary directory");
        let destination = dir.path().join("no-such-directory").join("out.png");

        let error =
            save_blocking(&sample_image(), &destination, ImageFormat::Png, None).expect_err("the directory is absent");

        assert!(matches!(error, ImageIoError::Write { .. }), "expected a write failure, got {error:?}");
        assert!(error.to_string().contains("out.png"), "the message did not name the destination: {error}");
    }

    #[tokio::test]
    async fn the_asynchronous_form_reports_what_the_blocking_one_does() {
        let dir = tempdir().expect("a temporary directory");
        let pixels = Arc::new(sample_image());

        let asynchronous = save(Arc::clone(&pixels), dir.path().join("async.png"), ImageFormat::Png, None)
            .await
            .expect("writable");
        let blocking =
            save_blocking(&pixels, dir.path().join("blocking.png"), ImageFormat::Png, None).expect("writable");

        assert_eq!(asynchronous, blocking);
        assert_eq!(
            std::fs::read(dir.path().join("async.png")).expect("readable"),
            std::fs::read(dir.path().join("blocking.png")).expect("readable"),
            "the two forms encoded different bytes"
        );
    }

    #[tokio::test]
    async fn the_asynchronous_form_refuses_an_unwritable_destination_the_way_the_blocking_one_does() {
        let dir = tempdir().expect("a temporary directory");
        let destination = dir.path().join("no-such-directory").join("out.png");

        let error = save(Arc::new(sample_image()), &destination, ImageFormat::Png, None)
            .await
            .expect_err("the directory is absent");

        assert!(matches!(error, ImageIoError::Write { .. }), "expected a write failure, got {error:?}");
    }

    #[test]
    fn a_runtime_that_shuts_down_before_the_write_runs_reports_cancellation() {
        let dir = tempdir().expect("a temporary directory");
        let outcome =
            on_a_dead_runtime(save(Arc::new(sample_image()), dir.path().join("out.png"), ImageFormat::Png, None));

        match outcome {
            Err(ImageIoError::Cancelled) => {}
            Err(other) => panic!("expected cancellation, got {other:?}"),
            Ok(written) => panic!("the write ran against a runtime that was already gone, writing {written} bytes"),
        }
    }

    #[test]
    fn a_failure_is_recorded_once_by_either_form_and_a_success_not_at_all() {
        use crate::image::test_log::{as_written, recorded_async, the_one_failure};
        use crate::image::test_support::sample_image;
        use crate::logging::{field, records_of_blocking};

        let dir = tempfile::tempdir().expect("a temporary directory");
        let unwritable = dir.path().join("no-such-folder").join("out.png");

        let (log, outcome) =
            records_of_blocking("info", || save_blocking(&sample_image(), &unwritable, ImageFormat::Png, None));
        assert!(outcome.is_err());
        let record = the_one_failure(&log, "an image could not be written", "save");
        assert!(record.contains(&format!("path={}", as_written(&unwritable))), "{record}");
        assert!(field(record, "format").is_some(), "{record}");

        let (log, outcome) =
            recorded_async(super::save(Arc::new(sample_image()), unwritable.clone(), ImageFormat::Png, None));
        assert!(outcome.is_err());
        the_one_failure(&log, "an image could not be written", "save");

        let writable = dir.path().join("out.png");
        let (log, outcome) =
            records_of_blocking("info", || save_blocking(&sample_image(), &writable, ImageFormat::Png, None));
        assert!(outcome.is_ok());
        assert!(!log.contains("msg="), "a success was recorded:\n{log}");
    }
}
