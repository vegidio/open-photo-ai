//! The format an export is written in, and the encoder settings the window's quality becomes.
//!
//! One place for both, so the window's words for the formats and its quality range are translated once. See
//! design.md D5.

use opai::{EncodeOptions, ImageFormat};
use serde::Deserialize;

/// The format the window asks for, spelled as it spells it.
///
/// The four lossy names are `QUALITY_FORMATS` in `frontend/stores/settings.ts`. `heic` is the window's word for
/// what the library calls HEIF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ExportFormat {
    /// Windows Bitmap.
    Bmp,
    /// GIF.
    Gif,
    /// JPEG.
    Jpeg,
    /// PNG.
    Png,
    /// TIFF.
    Tiff,
    /// AVIF.
    Avif,
    /// HEIC, which the library writes as HEIF.
    Heic,
    /// WebP.
    Webp,
}

impl From<ExportFormat> for ImageFormat {
    fn from(format: ExportFormat) -> Self {
        match format {
            ExportFormat::Bmp => Self::Bmp,
            ExportFormat::Gif => Self::Gif,
            ExportFormat::Jpeg => Self::Jpeg,
            ExportFormat::Png => Self::Png,
            ExportFormat::Tiff => Self::Tiff,
            ExportFormat::Avif => Self::Avif,
            ExportFormat::Heic => Self::Heif,
            ExportFormat::Webp => Self::WebP,
        }
    }
}

/// The encoder settings for `format` at `quality`.
///
/// The encoder's own defaults, with the quality set on the four lossy formats and AVIF at speed 6, as the reference
/// writes it. The lossless formats ignore the quality.
///
/// A quality outside `1..=100` is brought inside it rather than refused. Not `0..=100`: a zero reaches the AVIF,
/// HEIF and WebP encoders as a real setting and produces an unusable image, as the reference found.
pub(crate) fn encode_options(format: ExportFormat, quality: f64) -> EncodeOptions {
    // `max(1)` after the cast as well as the clamp before it, because a NaN survives a clamp and casts to zero.
    let quality = (quality.clamp(1.0, 100.0).round() as u8).max(1);

    match EncodeOptions::default_for(format.into()) {
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

    const EVERY: [ExportFormat; 8] = [
        ExportFormat::Bmp,
        ExportFormat::Gif,
        ExportFormat::Jpeg,
        ExportFormat::Png,
        ExportFormat::Tiff,
        ExportFormat::Avif,
        ExportFormat::Heic,
        ExportFormat::Webp,
    ];

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
    fn every_spelling_the_window_sends_is_read_as_its_format() {
        let spelled = ["bmp", "gif", "jpeg", "png", "tiff", "avif", "heic", "webp"];

        for (spelling, format) in spelled.into_iter().zip(EVERY) {
            let read: ExportFormat = serde_json::from_value(serde_json::json!(spelling))
                .unwrap_or_else(|error| panic!("`{spelling}` is how the window names a format: {error}"));

            assert_eq!(read, format);
        }

        assert!(
            serde_json::from_value::<ExportFormat>(serde_json::json!("heif")).is_err(),
            "the window says heic"
        );
    }

    #[test]
    fn heic_is_written_as_heif() {
        assert_eq!(ImageFormat::from(ExportFormat::Heic), ImageFormat::Heif);
        assert_eq!(encode_options(ExportFormat::Heic, 80.0).format(), ImageFormat::Heif);
    }

    #[test]
    fn each_format_gets_its_own_settings() {
        for format in EVERY {
            assert_eq!(encode_options(format, 80.0).format(), ImageFormat::from(format), "{format:?}");
        }
    }

    #[test]
    fn the_quality_reaches_the_four_lossy_formats_and_no_other() {
        for format in EVERY {
            let expected = match format {
                ExportFormat::Jpeg | ExportFormat::Avif | ExportFormat::Heic | ExportFormat::Webp => Some(42),
                ExportFormat::Bmp | ExportFormat::Gif | ExportFormat::Png | ExportFormat::Tiff => None,
            };

            assert_eq!(quality_of(encode_options(format, 42.0)), expected, "{format:?}");
        }
    }

    #[test]
    fn a_lossless_format_is_written_the_same_at_any_quality() {
        for format in [ExportFormat::Bmp, ExportFormat::Gif, ExportFormat::Png, ExportFormat::Tiff] {
            assert_eq!(encode_options(format, 90.0), encode_options(format, 30.0), "{format:?}");
            assert_eq!(encode_options(format, 90.0), EncodeOptions::default_for(format.into()), "{format:?}");
        }
    }

    #[test]
    fn a_quality_outside_its_range_is_brought_inside_it() {
        for (asked, written) in [(0.0, 1), (-20.0, 1), (150.0, 100), (100.0, 100), (1.0, 1), (72.4, 72), (f64::NAN, 1)]
        {
            assert_eq!(quality_of(encode_options(ExportFormat::Jpeg, asked)), Some(written), "asked for {asked}");
        }
    }

    #[test]
    fn avif_is_written_at_speed_six_and_the_rest_keep_the_encoders_defaults() {
        let EncodeOptions::Avif { speed, threads, .. } = encode_options(ExportFormat::Avif, 50.0) else {
            panic!("AVIF was not written as AVIF");
        };
        assert_eq!((speed, threads), (6, None));

        let EncodeOptions::Heif { preset, chroma, .. } = encode_options(ExportFormat::Heic, 50.0) else {
            panic!("HEIC was not written as HEIF");
        };
        let EncodeOptions::Heif { preset: default_preset, chroma: default_chroma, .. } =
            EncodeOptions::default_for(ImageFormat::Heif)
        else {
            unreachable!("the default for HEIF is HEIF's");
        };
        assert_eq!((preset, chroma), (default_preset, default_chroma));

        let EncodeOptions::WebP { lossless, .. } = encode_options(ExportFormat::Webp, 50.0) else {
            panic!("WebP was not written as WebP");
        };
        assert!(!lossless, "a WebP export ignored its quality by being lossless");
    }
}
