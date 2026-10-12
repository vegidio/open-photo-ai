//! The two files a run touches, checked before anything is installed: the input can be read, and the output's
//! extension names a format that can be written.

use std::path::{Path, PathBuf};

use opai::image::{self, recommended_encode_options};
use opai::{EncodeOptions, ImageFormat};

use crate::error::CliError;

/// Where the result goes, and how it is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// The path, exactly as given. A file already there is overwritten.
    pub path: PathBuf,
    /// The format the path's extension names.
    pub format: ImageFormat,
    /// The library's recommended settings for that format: what a GUI export writes it with.
    pub options: EncodeOptions,
}

/// `path` resolved to the format its extension names and the settings that format is written with.
///
/// # Errors
///
/// [`CliError::Usage`] where the extension names no format that can be written, or there is none; the message names
/// the path and lists the extensions that can.
pub fn resolve(path: &Path) -> Result<Output, CliError> {
    let Some(format) = ImageFormat::from_path(path) else {
        let reason = match path.extension() {
            Some(extension) => format!(".{} is not a format opai can write", extension.to_string_lossy()),
            None => "it has no extension to choose a format by".to_string(),
        };

        return Err(CliError::Usage(format!(
            "cannot write {}: {reason}; use one of: {}",
            path.display(),
            writable_extensions()
        )));
    };

    Ok(Output { path: path.to_path_buf(), format, options: recommended_encode_options(format, None) })
}

/// The width and height of the image at `path`, read from its header without decoding it.
///
/// Camera RAW is recognised by extension and described by its own probe, which reports the size a load develops it
/// to.
///
/// # Errors
///
/// [`CliError::Usage`] where the file does not exist, cannot be read, or is not an image this application opens; the
/// message names the path.
pub fn check_input(path: &Path) -> Result<(u32, u32), CliError> {
    // The blocking forms: this runs once, before anything else, and reads a header.
    let dimensions = if image::is_raw(path) {
        image::probe_raw_blocking(path).map(|info| (info.width, info.height))
    } else {
        image::probe_blocking(path).map(|info| (info.width, info.height))
    };

    // Every refusal a probe makes already names the path.
    dimensions.map_err(|error| CliError::Usage(error.to_string()))
}

/// Every extension a format can be written under, comma-separated: `bmp, gif, jpg, jpeg, …`.
fn writable_extensions() -> String {
    ImageFormat::ALL
        .into_iter()
        .flat_map(image::extensions_of)
        .copied()
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The repository's test photograph, a JPEG despite its extension.
    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/test.dat")
    }

    #[test]
    fn the_format_follows_the_extension_whatever_its_case() {
        for (name, format) in
            [("out.webp", ImageFormat::WebP), ("OUT.JPG", ImageFormat::Jpeg), ("out.heic", ImageFormat::Heif)]
        {
            let output = resolve(Path::new(name)).unwrap_or_else(|error| panic!("{name}: {error}"));

            assert_eq!(output.format, format, "{name}");
            assert_eq!(output.options, recommended_encode_options(format, None), "{name}");
            assert_eq!(output.path, PathBuf::from(name));
        }
    }

    #[test]
    fn an_unknown_extension_is_refused_listing_the_writable_ones() {
        let error = resolve(Path::new("out.xyz")).expect_err("xyz is not a format");

        assert_eq!(error.exit_code(), 2);
        let message = error.to_string();
        assert!(message.contains("out.xyz") && message.contains(".xyz is not a format"), "{message}");
        for extension in ["bmp", "gif", "jpg", "png", "tiff", "avif", "heic", "webp"] {
            assert!(message.contains(extension), "{extension} is not listed in: {message}");
        }
    }

    #[test]
    fn a_missing_extension_is_refused() {
        let error = resolve(Path::new("out")).expect_err("no extension");

        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("no extension"), "{error}");
    }

    #[test]
    fn a_missing_input_is_refused_naming_it() {
        let error = check_input(Path::new("missing.jpg")).expect_err("there is no such file");

        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("missing.jpg"), "{error}");
    }

    #[test]
    fn an_input_is_recognised_by_its_extension() {
        // `.dat` names no format, so the fixture is refused as it is, though it is a JPEG.
        let error = check_input(&fixture()).expect_err(".dat names no format");
        assert_eq!(error.exit_code(), 2);
        assert!(error.to_string().contains("test.dat"), "{error}");

        let dir = tempfile::tempdir().expect("a temporary directory");
        let renamed = dir.path().join("test.jpg");
        std::fs::copy(fixture(), &renamed).expect("the fixture copies");

        let (width, height) = check_input(&renamed).expect("a JPEG is readable");
        let loaded = image::load_blocking(&renamed).expect("decodable");
        assert_eq!((width, height), loaded.dimensions());
    }
}
