//! The format an export is written in, what each format can do, and the encoder settings the window's quality
//! becomes.
//!
//! One place for all three, so the window's words for the formats, which of them take a quality, what each is named
//! on disk and which source a Preserve export writes back as itself are decided once — and published to the window
//! through [`export_formats`](super::export_formats) rather than restated there. See design.md D5.

use opai::{EncodeOptions, ImageFormat};
use serde::{Deserialize, Serialize};

/// The format the window asks for, spelled as it spells it.
///
/// Which of them take a quality, and the rest of what the window needs to know about each, is [`capabilities`].
/// `heic` is the window's word for what the library calls HEIF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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

/// The lowest quality a lossy format is written at. Not `0`: a zero reaches the AVIF, HEIF and WebP encoders as a real
/// setting and produces an unusable image, as the reference found.
const MIN_QUALITY: u8 = 1;

/// The highest quality a lossy format is written at.
const MAX_QUALITY: u8 = 100;

/// What a lossy format's quality may be, and what it is written at where the window names none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct QualityRange {
    /// The lowest quality accepted.
    pub(crate) min: u8,
    /// The highest quality accepted.
    pub(crate) max: u8,
    /// What the format is written at before a user moves a slider.
    ///
    /// **Per format, deliberately**: the scales are not comparable across encoders — 60 in libheif is a very
    /// different picture to 60 in libjpeg — and these are exactly what the reference's `utils.EncodeImage` hardcodes,
    /// so a user who never moves a slider gets the output the reference produces.
    pub(crate) default: u8,
}

/// What one format can do, as the window is told it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FormatCapability {
    /// The format, as the window names it.
    pub(crate) format: ExportFormat,
    /// The extension a file written in it is given when it is chosen: `jpg` for JPEG, the format's own name for the
    /// rest — what the reference's `jpg` value wrote, and what a user expects to see.
    pub(crate) extension: &'static str,
    /// The source extensions a Preserve export writes back **as this format**, keeping the source's own extension.
    /// The aliases collapse here, as the reference's `IMAGE_FORMAT_BY_EXT` collapses them: `jpg` and `jpeg` are one
    /// format, as are `heic` and `heif`.
    pub(crate) preserves: &'static [&'static str],
    /// The quality it takes, or `None` for a lossless format, whose encoder ignores one.
    pub(crate) quality: Option<QualityRange>,
}

/// Every format an export can be written in, what each can do, and what a Preserve export writes a source none of
/// them can write back as itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportFormats {
    /// Every format, in the order the window's chooser offers them: alphabetical by name.
    pub(crate) formats: Vec<FormatCapability>,
    /// What a source no format writes back as itself — a camera RAW file — is written as under Preserve: TIFF,
    /// because it holds everything a RAW source decodes to, depth included, which is the reference's own fallback.
    pub(crate) fallback: ExportFormat,
}

/// A lossy format's quality range, starting at `default`.
const fn lossy(default: u8) -> Option<QualityRange> {
    Some(QualityRange { min: MIN_QUALITY, max: MAX_QUALITY, default })
}

/// What every format can do: the one table the window's format rules are read from.
pub(crate) fn capabilities() -> ExportFormats {
    let capability = |format, extension, preserves, quality| FormatCapability { format, extension, preserves, quality };

    ExportFormats {
        formats: vec![
            capability(ExportFormat::Avif, "avif", &["avif"], lossy(60)),
            capability(ExportFormat::Bmp, "bmp", &["bmp"], None),
            capability(ExportFormat::Gif, "gif", &["gif"], None),
            capability(ExportFormat::Heic, "heic", &["heic", "heif"], lossy(60)),
            capability(ExportFormat::Jpeg, "jpg", &["jpeg", "jpg"], lossy(90)),
            capability(ExportFormat::Png, "png", &["png"], None),
            capability(ExportFormat::Tiff, "tiff", &["tif", "tiff"], None),
            capability(ExportFormat::Webp, "webp", &["webp"], lossy(75)),
        ],
        fallback: ExportFormat::Tiff,
    }
}

/// The quality `format` is written at where the window names none, or `None` for a format that takes none.
fn default_quality(format: ExportFormat) -> Option<u8> {
    capabilities()
        .formats
        .into_iter()
        .find(|capability| capability.format == format)?
        .quality
        .map(|range| range.default)
}

/// The encoder settings for `format` at `quality`.
///
/// The encoder's own defaults, with the quality set on the four lossy formats and AVIF at speed 6, as the reference
/// writes it. The lossless formats take no quality, and the window sends none for them.
///
/// A lossy format sent no quality is written at its published default. A quality outside `1..=100` is brought inside
/// it rather than refused.
pub(crate) fn encode_options(format: ExportFormat, quality: Option<f64>) -> EncodeOptions {
    // `max(1)` after the cast as well as the clamp before it, because a NaN survives a clamp and casts to zero.
    let quality = match quality {
        Some(quality) => (quality.clamp(f64::from(MIN_QUALITY), f64::from(MAX_QUALITY)).round() as u8).max(MIN_QUALITY),
        None => default_quality(format).unwrap_or(MAX_QUALITY),
    };

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
        assert_eq!(encode_options(ExportFormat::Heic, Some(80.0)).format(), ImageFormat::Heif);
    }

    #[test]
    fn each_format_gets_its_own_settings() {
        for format in EVERY {
            assert_eq!(encode_options(format, Some(80.0)).format(), ImageFormat::from(format), "{format:?}");
        }
    }

    #[test]
    fn the_quality_reaches_the_four_lossy_formats_and_no_other() {
        for format in EVERY {
            let expected = match format {
                ExportFormat::Jpeg | ExportFormat::Avif | ExportFormat::Heic | ExportFormat::Webp => Some(42),
                ExportFormat::Bmp | ExportFormat::Gif | ExportFormat::Png | ExportFormat::Tiff => None,
            };

            assert_eq!(quality_of(encode_options(format, Some(42.0))), expected, "{format:?}");
        }
    }

    #[test]
    fn a_lossless_format_is_written_the_same_at_any_quality() {
        for format in [ExportFormat::Bmp, ExportFormat::Gif, ExportFormat::Png, ExportFormat::Tiff] {
            assert_eq!(encode_options(format, Some(90.0)), encode_options(format, None), "{format:?}");
            assert_eq!(encode_options(format, Some(30.0)), EncodeOptions::default_for(format.into()), "{format:?}");
        }
    }

    #[test]
    fn a_quality_outside_its_range_is_brought_inside_it() {
        for (asked, written) in [(0.0, 1), (-20.0, 1), (150.0, 100), (100.0, 100), (1.0, 1), (72.4, 72), (f64::NAN, 1)]
        {
            assert_eq!(quality_of(encode_options(ExportFormat::Jpeg, Some(asked))), Some(written), "asked for {asked}");
        }
    }

    #[test]
    fn avif_is_written_at_speed_six_and_the_rest_keep_the_encoders_defaults() {
        let EncodeOptions::Avif { speed, threads, .. } = encode_options(ExportFormat::Avif, Some(50.0)) else {
            panic!("AVIF was not written as AVIF");
        };
        assert_eq!((speed, threads), (6, None));

        let EncodeOptions::Heif { preset, chroma, .. } = encode_options(ExportFormat::Heic, Some(50.0)) else {
            panic!("HEIC was not written as HEIF");
        };
        let EncodeOptions::Heif { preset: default_preset, chroma: default_chroma, .. } =
            EncodeOptions::default_for(ImageFormat::Heif)
        else {
            unreachable!("the default for HEIF is HEIF's");
        };
        assert_eq!((preset, chroma), (default_preset, default_chroma));

        let EncodeOptions::WebP { lossless, .. } = encode_options(ExportFormat::Webp, Some(50.0)) else {
            panic!("WebP was not written as WebP");
        };
        assert!(!lossless, "a WebP export ignored its quality by being lossless");
    }

    #[test]
    fn a_lossy_format_sent_no_quality_is_written_at_its_published_default() {
        for (format, default) in [
            (ExportFormat::Avif, 60),
            (ExportFormat::Heic, 60),
            (ExportFormat::Jpeg, 90),
            (ExportFormat::Webp, 75),
        ] {
            assert_eq!(quality_of(encode_options(format, None)), Some(default), "{format:?}");
        }
    }

    #[test]
    fn the_published_capabilities_agree_with_what_the_encoder_is_handed() {
        // A format published as taking a quality is one whose encoder is handed it, and the reverse: the window hides
        // the slider on exactly the formats that would ignore it.
        let published = capabilities();

        assert_eq!(
            published.formats.iter().map(|capability| capability.format).collect::<Vec<_>>().len(),
            EVERY.len()
        );
        for format in EVERY {
            let capability = published
                .formats
                .iter()
                .find(|capability| capability.format == format)
                .unwrap_or_else(|| panic!("{format:?} is not published"));

            assert_eq!(
                capability.quality.is_some(),
                quality_of(encode_options(format, Some(42.0))).is_some(),
                "{format:?}"
            );
        }
    }

    /// The whole wire shape, pinned: the window's format rules are read from exactly this.
    #[test]
    fn the_capabilities_cross_the_boundary_as_the_window_reads_them() {
        let json = serde_json::to_value(capabilities()).expect("the capabilities serialize");

        assert_eq!(json["fallback"], "tiff");
        assert_eq!(
            json["formats"][4],
            serde_json::json!({
                "format": "jpeg",
                "extension": "jpg",
                "preserves": ["jpeg", "jpg"],
                "quality": { "min": 1, "max": 100, "default": 90 },
            })
        );
        assert_eq!(
            json["formats"][6],
            serde_json::json!({ "format": "tiff", "extension": "tiff", "preserves": ["tif", "tiff"], "quality": null })
        );
    }
}
