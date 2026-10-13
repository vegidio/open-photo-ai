//! The format an export is written in, what each format can do, and the encoder settings a quality becomes.
//!
//! One place for all three, so the formats' names, which of them take a quality, what each is named on disk and which
//! source a Preserve export writes back as itself are decided once, and every front end reads them from here.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::image::{EncodeOptions, ImageFormat};

/// A format an export is written in, spelled as the front ends spell it.
///
/// Which of them take a quality, and the rest of what a front end needs to know about each, is [`capabilities`].
/// `heic` is the front ends' word for what the library calls HEIF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
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

impl ExportFormat {
    /// Every format, in the order [`capabilities`] publishes them: alphabetical by name.
    pub const ALL: [Self; 8] =
        [Self::Avif, Self::Bmp, Self::Gif, Self::Heic, Self::Jpeg, Self::Png, Self::Tiff, Self::Webp];

    /// The format's name as the front ends spell it, which is also how it is serialized and parsed.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bmp => "bmp",
            Self::Gif => "gif",
            Self::Jpeg => "jpeg",
            Self::Png => "png",
            Self::Tiff => "tiff",
            Self::Avif => "avif",
            Self::Heic => "heic",
            Self::Webp => "webp",
        }
    }

    /// The extension a file is given when this format is chosen: `jpg` for JPEG, the format's own name for the rest —
    /// what the reference's `jpg` value wrote, and what a user expects to see.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            other => other.name(),
        }
    }

    /// The format a Preserve export writes a source with `extension` back as, keeping that extension, or `None` where
    /// no format does. `extension` is compared as given, so the caller lowercases it.
    pub(crate) fn preserving(extension: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|format| crate::image::extensions_of((*format).into()).contains(&extension))
    }
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

impl fmt::Display for ExportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for ExportFormat {
    type Err = UnknownFormat;

    /// Reads exactly the spelling [`name`](Self::name) gives: `jpg` and `heif` are extensions, not names.
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|format| format.name() == name)
            .ok_or_else(|| UnknownFormat(name.to_owned()))
    }
}

/// A word that names neither a format nor Preserve.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a format an export can be written in")]
pub struct UnknownFormat(pub String);

/// The format a user picks for an export: one format for every file, or each source's own where it can be written.
///
/// Spelled as one lowercase word — `preserve`, or an [`ExportFormat`]'s [`name`](ExportFormat::name) — by
/// [`Display`](fmt::Display), [`FromStr`] and serde alike, so what a front end stores is what crosses its wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormatChoice {
    /// The source's own format where one writes it back, and TIFF for the rest — camera RAW, and a file with no
    /// extension — because TIFF holds everything a RAW source decodes to, depth included. The reference's own
    /// fallback.
    Preserve,
    /// This format, whatever the source.
    Format(ExportFormat),
}

impl FormatChoice {
    /// What a Preserve export writes a source no format writes back as itself.
    pub const FALLBACK: ExportFormat = ExportFormat::Tiff;
}

impl From<ExportFormat> for FormatChoice {
    fn from(format: ExportFormat) -> Self {
        Self::Format(format)
    }
}

impl fmt::Display for FormatChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preserve => f.write_str("preserve"),
            Self::Format(format) => format.fmt(f),
        }
    }
}

impl FromStr for FormatChoice {
    type Err = UnknownFormat;

    fn from_str(word: &str) -> Result<Self, Self::Err> {
        match word {
            "preserve" => Ok(Self::Preserve),
            name => name.parse().map(Self::Format),
        }
    }
}

impl Serialize for FormatChoice {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for FormatChoice {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?.parse().map_err(serde::de::Error::custom)
    }
}

/// What a lossy format's quality may be, and what it is written at where none is named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct QualityRange {
    /// The lowest quality accepted.
    pub min: u8,
    /// The highest quality accepted.
    pub max: u8,
    /// What the format is written at before a user moves a slider.
    ///
    /// **Per format, deliberately**: the scales are not comparable across encoders — 60 in libheif is a very
    /// different picture to 60 in libjpeg — and these are exactly what the reference's `utils.EncodeImage` hardcodes,
    /// so a user who never moves a slider gets the output the reference produces.
    pub default: u8,
}

/// What one format can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatCapability {
    /// The format.
    pub format: ExportFormat,
    /// The extension a file written in it is given when it is chosen: [`ExportFormat::extension`].
    pub extension: &'static str,
    /// The source extensions a Preserve export writes back **as this format**, keeping the source's own extension.
    /// The aliases collapse here, as the reference's `IMAGE_FORMAT_BY_EXT` collapses them: `jpg` and `jpeg` are one
    /// format, as are `heic` and `heif`.
    pub preserves: &'static [&'static str],
    /// The quality it takes, or `None` for a lossless format, whose encoder ignores one.
    pub quality: Option<QualityRange>,
}

/// Every format an export can be written in, what each can do, and what a Preserve export writes a source none of
/// them can write back as itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportFormats {
    /// Every format, in the order a chooser offers them: alphabetical by name.
    pub formats: Vec<FormatCapability>,
    /// What a source no format writes back as itself is written as under Preserve: [`FormatChoice::FALLBACK`].
    pub fallback: ExportFormat,
}

/// The quality `format` is written at where none is named, or `None` for a format that takes none: the library's
/// [`crate::image::default_quality`], which every front end writes with.
fn default_quality(format: ExportFormat) -> Option<u8> {
    crate::image::default_quality(format.into())
}

/// The quality range `format` takes, starting at its [`default_quality`], or `None` for a lossless format.
fn quality_range(format: ExportFormat) -> Option<QualityRange> {
    default_quality(format).map(|default| QualityRange {
        min: crate::image::MIN_QUALITY,
        max: crate::image::MAX_QUALITY,
        default,
    })
}

/// What every format can do: the one table a front end's format rules are read from.
///
/// What a Preserve export writes back as each format is the library's own list of the extensions it is written under
/// — see [`crate::image::extensions_of`] — so the aliases this reads are the ones a picker offered.
pub fn capabilities() -> ExportFormats {
    ExportFormats {
        formats: ExportFormat::ALL
            .into_iter()
            .map(|format| FormatCapability {
                format,
                extension: format.extension(),
                preserves: crate::image::extensions_of(format.into()),
                quality: quality_range(format),
            })
            .collect(),
        fallback: FormatChoice::FALLBACK,
    }
}

/// The encoder settings for `format` at `quality`: the library's
/// [`recommended_encode_options`](crate::image::recommended_encode_options), so an export writes each format exactly
/// as every other front end does.
///
/// A lossy format sent no quality is written at its published default. A quality outside `1..=100` is brought inside
/// it rather than refused.
pub fn encode_options(format: ExportFormat, quality: Option<f64>) -> EncodeOptions {
    crate::image::recommended_encode_options(format.into(), quality)
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
                "preserves": ["jpg", "jpeg"],
                "quality": { "min": 1, "max": 100, "default": 90 },
            })
        );
        assert_eq!(
            json["formats"][6],
            serde_json::json!({ "format": "tiff", "extension": "tiff", "preserves": ["tiff", "tif"], "quality": null })
        );
    }

    #[test]
    fn every_format_choice_round_trips_through_its_one_spelling() {
        let choices = std::iter::once(FormatChoice::Preserve).chain(ExportFormat::ALL.map(FormatChoice::Format));
        let spelled = ["preserve", "avif", "bmp", "gif", "heic", "jpeg", "png", "tiff", "webp"];

        for (choice, spelling) in choices.zip(spelled) {
            assert_eq!(choice.to_string(), spelling);
            assert_eq!(spelling.parse::<FormatChoice>(), Ok(choice));
            assert_eq!(serde_json::to_value(choice).expect("serializes"), serde_json::json!(spelling));
            assert_eq!(serde_json::from_value::<FormatChoice>(serde_json::json!(spelling)).expect("reads"), choice);
        }
    }

    #[test]
    fn an_extension_or_an_unknown_word_is_not_a_format_choice() {
        for word in ["heif", "jpg", "tif", "raw", "", "JPEG", "Preserve"] {
            assert_eq!(word.parse::<FormatChoice>(), Err(UnknownFormat(word.to_owned())), "`{word}`");
            assert!(serde_json::from_value::<FormatChoice>(serde_json::json!(word)).is_err(), "`{word}`");
        }
    }

    #[test]
    fn preserve_writes_back_every_alias_of_a_format_and_nothing_else() {
        for (extension, format) in [
            ("jpg", Some(ExportFormat::Jpeg)),
            ("jpeg", Some(ExportFormat::Jpeg)),
            ("heif", Some(ExportFormat::Heic)),
            ("heic", Some(ExportFormat::Heic)),
            ("tif", Some(ExportFormat::Tiff)),
            ("webp", Some(ExportFormat::Webp)),
            ("nef", None),
            ("", None),
        ] {
            assert_eq!(ExportFormat::preserving(extension), format, "`{extension}`");
        }
    }
}
