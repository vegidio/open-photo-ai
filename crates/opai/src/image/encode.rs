//! Encoding an image into memory rather than onto a disk.

use std::sync::Arc;

use ::image::DynamicImage;
use rust_sak::image::{EncodeOptions, ImageError, ImageFormat, encode_writer};

use super::error::{self, ImageIoError};
use crate::task::spawn_blocking;
use crate::telemetry::unit::{Unit, unit_span};

/// Encodes `pixels` as `format` into a buffer, without occupying the thread driving the caller.
///
/// See [`encode_blocking`] for what it does; the two produce the same bytes and report the same errors.
///
/// # Errors
///
/// Everything [`encode_blocking`] returns, plus [`ImageIoError::Cancelled`] if the runtime shut down before the work
/// could run.
pub async fn encode(
    pixels: Arc<DynamicImage>,
    format: ImageFormat,
    options: Option<EncodeOptions>,
) -> Result<Vec<u8>, ImageIoError> {
    // An `Arc` for the reason `save` takes one.
    spawn_blocking::<_, ImageIoError, _>(move || encode_blocking(&pixels, format, options)).await?
}

/// Encodes `pixels` as `format` into a buffer, on the calling thread.
///
/// [`save`](fn@super::save) with no file at the end of it, for the callers that are not writing one: a preview streamed
/// to a window, an entry put into a cache, an image handed to something that wants bytes. It is the same encode
/// through the same dispatch, so what a caller gets here is byte for byte what saving the same picture in the same
/// format would have written.
///
/// **16 bits per channel are narrowed for the formats that cannot hold them** — a developed camera RAW encoded as
/// JPEG comes back at 8 bits rather than being refused. Which formats keep the depth is in the module's own
/// documentation.
///
/// `options` may be omitted for the format's defaults, and settings targeting another format are refused.
///
/// # Errors
///
/// - [`ImageIoError::Encode`] if `options` target a format other than `format`, or if the encoder refuses the
///   picture.
pub fn encode_blocking(
    pixels: &DynamicImage,
    format: ImageFormat,
    options: Option<EncodeOptions>,
) -> Result<Vec<u8>, ImageIoError> {
    let span = unit_span!("image_encode", format = ?format);
    error::traced(Unit::ImageEncode, span, || {
        encode_into(pixels, format, options).inspect_err(error::unencodable(format))
    })
}

/// [`encode_blocking`]'s body, which leaves recording its failure to the public boundary.
fn encode_into(
    pixels: &DynamicImage,
    format: ImageFormat,
    options: Option<EncodeOptions>,
) -> Result<Vec<u8>, ImageIoError> {
    // The same early check `save_blocking` makes, and here it is the whole of the check rather than a guard over a
    // destination: with no file to truncate there is nothing to protect, so this exists only to report the mismatch
    // in this crate's own words instead of partway through a buffer.
    if let Some(options) = options
        && options.format() != format
    {
        return Err(ImageIoError::Encode {
            source: ImageError::FormatMismatch { expected: format, options: options.format() },
        });
    }

    // Not a second implementation of saving: both call `rust-sak`'s one encode dispatch, and that is what makes this
    // worth having rather than leaving each caller to reach for a codec. The dispatch is where 16 bits are narrowed,
    // and a caller that encoded through the `image` crate directly would have a refusal instead, for exactly the files
    // this crate exists to open.
    let mut bytes = Vec::new();
    encode_writer(pixels, &mut bytes, format, options).map_err(|source| ImageIoError::Encode { source })?;

    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::test_support::{FIXTURE_SIZE, on_a_dead_runtime, sample_image};
    use crate::image::{load_blocking, save_blocking};

    use tempfile::tempdir;

    #[test]
    fn what_it_encodes_is_what_saving_the_same_picture_would_have_written() {
        // The property that makes this one operation rather than two: a preview and an export of the same picture in
        // the same format cannot come out differently, because the bytes come from the same dispatch.
        let dir = tempdir().expect("a temporary directory");
        let destination = dir.path().join("round-trip.png");

        save_blocking(&sample_image(), &destination, ImageFormat::Png, None).expect("writable");
        let encoded = encode_blocking(&sample_image(), ImageFormat::Png, None).expect("encodable");

        assert_eq!(encoded, std::fs::read(&destination).expect("readable"));
    }

    #[test]
    fn every_format_encodes_and_decodes_back_to_the_same_picture() {
        for format in [
            ImageFormat::Bmp,
            ImageFormat::Gif,
            ImageFormat::Jpeg,
            ImageFormat::Png,
            ImageFormat::Tiff,
            ImageFormat::Avif,
            ImageFormat::Heif,
            ImageFormat::WebP,
        ] {
            let bytes = encode_blocking(&sample_image(), format, None)
                .unwrap_or_else(|err| panic!("{format:?} could not be encoded: {err}"));
            assert!(!bytes.is_empty(), "{format:?} reported success having encoded nothing");

            // Written out and read back rather than decoded from the buffer, because what is being checked is that
            // these bytes are a file of that format and not merely a buffer that came back non-empty.
            let dir = tempdir().expect("a temporary directory");
            let path = dir.path().join(format!("encoded.{}", format.extension()));
            std::fs::write(&path, &bytes).expect("writable");

            let loaded = load_blocking(&path).unwrap_or_else(|err| panic!("{format:?} did not decode: {err}"));
            assert_eq!(loaded.dimensions(), FIXTURE_SIZE, "{format:?} came back the wrong size");
        }
    }

    #[test]
    fn the_options_are_honoured() {
        // A lower quality produces fewer bytes, which is what proves the settings reach the encoder rather than being
        // dropped on the way.
        let high = encode_blocking(&sample_image(), ImageFormat::Jpeg, Some(EncodeOptions::Jpeg { quality: 95 }))
            .expect("encodable");
        let low = encode_blocking(&sample_image(), ImageFormat::Jpeg, Some(EncodeOptions::Jpeg { quality: 20 }))
            .expect("encodable");

        assert!(
            low.len() < high.len(),
            "quality 20 produced {} bytes and quality 95 produced {}",
            low.len(),
            high.len()
        );
    }

    #[test]
    fn settings_that_target_another_format_are_refused() {
        let error = encode_blocking(&sample_image(), ImageFormat::Png, Some(EncodeOptions::Jpeg { quality: 90 }))
            .expect_err("JPEG settings do not describe a PNG");

        match error {
            ImageIoError::Encode { source: ImageError::FormatMismatch { expected, options } } => {
                assert_eq!((expected, options), (ImageFormat::Png, ImageFormat::Jpeg));
            }
            other => panic!("expected a format mismatch, got {other:?}"),
        }
    }

    #[test]
    fn the_failure_names_no_file_because_there_is_none() {
        let error = encode_blocking(&sample_image(), ImageFormat::Png, Some(EncodeOptions::Tiff)).expect_err("refused");

        assert!(!error.to_string().contains('/'), "the message named a path it cannot know: {error}");
    }

    #[test]
    fn a_sixteen_bit_picture_is_narrowed_rather_than_refused() {
        let deep =
            DynamicImage::ImageRgb16(image::ImageBuffer::from_pixel(32, 24, image::Rgb([30_000u16, 20_000, 10_000])));

        let bytes = encode_blocking(&deep, ImageFormat::Jpeg, Some(EncodeOptions::Jpeg { quality: 90 }))
            .expect("a 16-bit picture is narrowed for JPEG rather than refused");

        assert!(!bytes.is_empty());
    }

    #[tokio::test]
    async fn both_forms_produce_the_same_bytes() {
        let pixels = Arc::new(sample_image());

        let asynchronous = encode(Arc::clone(&pixels), ImageFormat::Png, None).await.expect("encodable");
        let blocking = encode_blocking(&pixels, ImageFormat::Png, None).expect("encodable");

        assert_eq!(asynchronous, blocking);
    }

    #[test]
    fn a_runtime_that_shuts_down_before_the_encode_runs_reports_cancellation() {
        let outcome = on_a_dead_runtime(encode(Arc::new(sample_image()), ImageFormat::Png, None));

        match outcome {
            Err(ImageIoError::Cancelled) => {}
            Err(other) => panic!("expected cancellation, got {other:?}"),
            Ok(_) => panic!("the encode ran against a runtime that was already gone"),
        }
    }

    #[test]
    fn a_failure_is_recorded_once_by_either_form_and_a_success_not_at_all() {
        use crate::image::test_log::{recorded_async, the_one_failure};
        use crate::logging::{field, records_of_blocking};

        let mismatched = Some(EncodeOptions::Jpeg { quality: 90 });

        let (log, outcome) =
            records_of_blocking("info", || encode_blocking(&sample_image(), ImageFormat::Png, mismatched));
        assert!(outcome.is_err());
        let record = the_one_failure(&log, "an image could not be encoded", "encode");
        assert!(field(record, "format").is_some(), "{record}");
        assert!(field(record, "path").is_none(), "an encode has no file to name: {record}");

        let (log, outcome) = recorded_async(super::encode(Arc::new(sample_image()), ImageFormat::Png, mismatched));
        assert!(outcome.is_err());
        the_one_failure(&log, "an image could not be encoded", "encode");

        let (log, outcome) = records_of_blocking("info", || encode_blocking(&sample_image(), ImageFormat::Png, None));
        assert!(outcome.is_ok());
        assert!(!log.contains("msg="), "a success was recorded:\n{log}");
    }
}
