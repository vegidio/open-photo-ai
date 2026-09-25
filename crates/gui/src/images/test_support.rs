//! Fixtures the tests in more than one of this module's files need.

// Built rather than committed, for the reason the crate's other fixtures are: these tests depend on no
// photograph in the working tree and can name the size they need.

use std::path::{Path, PathBuf};

use opai::ImageFormat;
use tempfile::TempDir;

use super::files::{ImageRecord, Opened};

/// A picture of a known size, written into `dir` under `name`, and the path it was written to.
pub(super) fn write_image(dir: &TempDir, name: &str, width: u32, height: u32, format: ImageFormat) -> PathBuf {
    let path = dir.path().join(name);
    let pixels = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    }));

    opai::image::save_blocking(&pixels, &path, format, None).expect("the fixture should be writable");
    path
}

/// Describes and admits one file, handing back its identity.
pub(super) fn admit(opened: &Opened, path: &Path) -> String {
    let record = ImageRecord::describe(path);
    let identity = record.identity.clone().expect("a readable fixture has an identity");
    opened.admit(&[(record, path.to_path_buf())]);
    identity
}
