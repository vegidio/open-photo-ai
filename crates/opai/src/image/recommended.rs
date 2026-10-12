//! The encoder settings a front end writes each format with unless its user chooses otherwise.
//!
//! One table for every front end, so the GUI's export and the CLI write a given format the same way. Writing with no
//! settings at all is unaffected: [`save`](super::save) handed `None` still uses the codec's own defaults.

// These numbers are this application's product choice — exactly what the reference's `utils.EncodeImage` hardcodes —
// rather than a general-purpose codec default, which is why they live here and not in `rust-sak`'s `image` module.
// Another project would want its own.

use rust_sak::image::{EncodeOptions, ImageFormat};

/// The lowest quality a lossy format is written at. Not `0`: a zero reaches the AVIF, HEIF and WebP encoders as a real
/// setting and produces an unusable image, as the reference found.
pub const MIN_QUALITY: u8 = 1;

/// The highest quality a lossy format is written at.
pub const MAX_QUALITY: u8 = 100;

/// The quality `format` is written at where the caller names none, or `None` for a lossless format, which takes none.
///
/// **Per format, deliberately**: the scales are not comparable across encoders — 60 in libheif is a very different
/// picture to 60 in libjpeg — and these are what the reference writes, so a user who never chooses a quality gets the
/// output the reference produces.
pub const fn default_quality(format: ImageFormat) -> Option<u8> {
    match format {
        ImageFormat::Avif | ImageFormat::Heif => Some(60),
        ImageFormat::Jpeg => Some(90),
        ImageFormat::WebP => Some(75),
        ImageFormat::Bmp | ImageFormat::Gif | ImageFormat::Png | ImageFormat::Tiff => None,
    }
}

/// The encoder settings for `format` at `quality`, or at the format's [`default_quality`] where `quality` is `None`.
///
/// The encoder's own defaults, with the quality set on the four lossy formats and AVIF at speed 6, as the reference
/// writes it. A lossless format takes no quality, and is handed its encoder's defaults whatever `quality` is.
///
/// A quality outside [`MIN_QUALITY`]`..=`[`MAX_QUALITY`] is brought inside it rather than refused, and a value that is
/// not a number is the lowest quality, never `0`.
pub fn recommended_encode_options(format: ImageFormat, quality: Option<f64>) -> EncodeOptions {
    // `max(1)` after the cast as well as the clamp before it, because a NaN survives a clamp and casts to zero.
    let quality = match quality {
        Some(quality) => (quality.clamp(f64::from(MIN_QUALITY), f64::from(MAX_QUALITY)).round() as u8).max(MIN_QUALITY),
        None => default_quality(format).unwrap_or(MAX_QUALITY),
    };

    match EncodeOptions::default_for(format) {
        EncodeOptions::Jpeg { .. } => EncodeOptions::Jpeg { quality },
        EncodeOptions::Avif { threads, .. } => EncodeOptions::Avif { quality, speed: 6, threads },
        EncodeOptions::Heif { preset, chroma, .. } => EncodeOptions::Heif { quality, preset, chroma },
        EncodeOptions::WebP { quality_alpha, compression, lossless, threads, .. } => {
            EncodeOptions::WebP { quality, quality_alpha, compression, lossless, threads }
        }
        lossless @ (EncodeOptions::Bmp | EncodeOptions::Gif | EncodeOptions::Png { .. } | EncodeOptions::Tiff) => {
            lossless
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::save_blocking;
    use crate::image::test_support::sample_image;

    use tempfile::tempdir;

    /// The quality set on `options`, or `None` for a format that takes none.
    fn quality_of(options: EncodeOptions) -> Option<u8> {
        match options {
            EncodeOptions::Jpeg { quality }
            | EncodeOptions::Avif { quality, .. }
            | EncodeOptions::Heif { quality, .. }
            | EncodeOptions::WebP { quality, .. } => Some(quality),
            EncodeOptions::Bmp | EncodeOptions::Gif | EncodeOptions::Png { .. } | EncodeOptions::Tiff => None,
        }
    }

    #[test]
    fn a_lossy_format_asked_for_no_quality_carries_its_default() {
        for (format, default) in [
            (ImageFormat::Jpeg, 90),
            (ImageFormat::Avif, 60),
            (ImageFormat::Heif, 60),
            (ImageFormat::WebP, 75),
        ] {
            assert_eq!(default_quality(format), Some(default), "{format:?}");
            assert_eq!(quality_of(recommended_encode_options(format, None)), Some(default), "{format:?}");
        }
    }

    #[test]
    fn a_lossless_format_carries_no_quality_and_its_encoders_defaults() {
        for format in [ImageFormat::Bmp, ImageFormat::Gif, ImageFormat::Png, ImageFormat::Tiff] {
            assert_eq!(default_quality(format), None, "{format:?}");
            for quality in [None, Some(30.0), Some(90.0)] {
                assert_eq!(
                    recommended_encode_options(format, quality),
                    EncodeOptions::default_for(format),
                    "{format:?} at {quality:?}"
                );
            }
        }
    }

    #[test]
    fn a_quality_outside_its_range_is_brought_inside_it() {
        for (asked, written) in [(0.0, 1), (-20.0, 1), (150.0, 100), (100.0, 100), (1.0, 1), (72.4, 72), (f64::NAN, 1)]
        {
            assert_eq!(
                quality_of(recommended_encode_options(ImageFormat::Jpeg, Some(asked))),
                Some(written),
                "asked for {asked}"
            );
        }
    }

    #[test]
    fn avif_is_written_at_speed_six_and_the_rest_keep_the_encoders_defaults() {
        let EncodeOptions::Avif { speed, threads, .. } = recommended_encode_options(ImageFormat::Avif, None) else {
            panic!("AVIF settings are not AVIF's");
        };
        assert_eq!((speed, threads), (6, None));

        let EncodeOptions::WebP { lossless, .. } = recommended_encode_options(ImageFormat::WebP, None) else {
            panic!("WebP settings are not WebP's");
        };
        assert!(!lossless, "a lossless WebP would ignore its quality");
    }

    #[test]
    fn every_format_is_written_with_its_own_settings() {
        let dir = tempdir().expect("a temporary directory");

        for format in ImageFormat::ALL {
            let options = recommended_encode_options(format, None);
            assert_eq!(options.format(), format);

            let destination = dir.path().join(format!("recommended.{}", format.extension()));
            save_blocking(&sample_image(), &destination, format, Some(options))
                .unwrap_or_else(|error| panic!("{format:?} refused its own recommended settings: {error}"));
        }
    }
}
