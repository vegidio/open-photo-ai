//! The image the application is working on: its pixels, where they came from, and what identifies them.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ::image::DynamicImage;

/// A decoded image together with where its pixels came from and the identity that describes them.
///
/// This is the one description of "an image the application is working on" that inference, the per-operation cache
/// and the export queue all work from.
///
/// # Identity
///
/// [`identity`](Self::identity) is a lowercase hexadecimal string, and what holds of every picture is that it
/// describes **these** pixels: it is never carried onto different ones and is never stale. The fields are private
/// and the pixels cannot be altered in place. It is what the per-operation cache is keyed on.
///
/// Where the value comes from depends on who produced the picture:
///
/// - **Loaded** through [`load`](fn@super::load), it is the XXH3-64 of the **whole** of the file's bytes — the same
///   value [`rust_sak::crypto::xxh3_file`] produces for that file. That equality is the loader's guarantee and the
///   point of it: a file listing can hash a path without decoding it, and later recognise the loaded image as the
///   same one.
/// - **Computed** from other pixels, it is derived from the identity of what they were computed from together with
///   what was done to them. No file on disk hashes to it, and none needs to.
///
/// [`Picture::new`] is the one place the invariant can be broken, because it is the one place pixels and an identity
/// meet without having been read from the same file; see its documentation.
///
/// # Cost of cloning
///
/// Cloning shares the pixels rather than copying them, which after an 8x upscale is the difference between a
/// refcount bump and several hundred megabytes.
#[derive(Debug, Clone)]
pub struct Picture {
    /// Where the pixels came from, never where they are going.
    path: PathBuf,
    // Behind an `Arc` so the pixels cannot be altered in place while `identity` goes on describing what they used to
    // be — the failure the reference's type warns about and cannot prevent. Sharing is also what lets the asynchronous
    // operations exist at all: `spawn_blocking` requires `'static`, so a borrow of the caller's image cannot cross onto
    // a blocking thread, and a copy would defeat the purpose of going there.
    /// The decoded pixels, shared rather than owned.
    pixels: Arc<DynamicImage>,
    /// The identity of these pixels, lowercase hexadecimal. See the type's documentation for where it comes from.
    identity: String,
}

impl Picture {
    /// Builds an image from pixels and the identity that describes them.
    ///
    /// **The caller is responsible for the pairing.** Nothing here can check that `identity` describes `pixels` —
    /// a hash is opaque, and when the pixels were decoded the bytes behind them are gone. Loading a file goes
    /// through [`load`](fn@super::load), which derives both from the same bytes; this constructor exists for the cases
    /// that cannot.
    pub fn new(path: impl Into<PathBuf>, pixels: impl Into<Arc<DynamicImage>>, identity: impl Into<String>) -> Self {
        Self { path: path.into(), pixels: pixels.into(), identity: identity.into() }
    }

    /// Where the pixels were read from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The decoded pixels.
    pub fn pixels(&self) -> &DynamicImage {
        &self.pixels
    }

    /// The pixels as a shared handle, for a caller that needs to keep them beyond this image's lifetime.
    ///
    /// Cheap: a refcount bump, not a copy. This is how an operation hands the same pixels to a blocking thread
    /// without the caller giving up its own image.
    pub fn shared_pixels(&self) -> Arc<DynamicImage> {
        Arc::clone(&self.pixels)
    }

    /// What identifies these pixels, lowercase hexadecimal. See the type's documentation for where it comes from.
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// The image's width and height, in pixels.
    pub fn dimensions(&self) -> (u32, u32) {
        // Derived from the pixels on every call rather than stored. A stored pair would be a second answer to a
        // question the pixels already answer, and a second answer is a thing that can disagree — the same class of
        // defect as a stale identity, which this type otherwise spends its shape preventing.
        (self.pixels.width(), self.pixels.height())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::image::RgbaImage;

    /// An image of a known size, built rather than decoded so the test does not depend on a fixture.
    fn image_of(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::new(width, height))
    }

    #[test]
    fn the_reported_dimensions_are_the_pixels_own() {
        let source = Picture::new("/pictures/wide.png", image_of(640, 480), "0123456789abcdef");

        assert_eq!(source.dimensions(), (640, 480));
        // Derived rather than stored, so the two cannot disagree: the same numbers the pixels themselves report.
        assert_eq!(source.dimensions(), (source.pixels().width(), source.pixels().height()));
    }

    #[test]
    fn the_path_and_the_identity_are_reported_back_as_given() {
        let source = Picture::new("/pictures/holiday.jpg", image_of(2, 2), "cafebabecafebabe");

        assert_eq!(source.path(), Path::new("/pictures/holiday.jpg"));
        assert_eq!(source.identity(), "cafebabecafebabe");
    }

    #[test]
    fn cloning_shares_the_pixel_buffer_rather_than_copying_it() {
        let source = Picture::new("/pictures/large.tif", image_of(64, 64), "deadbeefdeadbeef");
        let copy = source.clone();

        assert!(
            std::ptr::eq(source.pixels(), copy.pixels()),
            "the clone decoded a second buffer instead of sharing the first"
        );
        assert_eq!(Arc::strong_count(&source.shared_pixels()), 3, "the handle is not the one the clone shares");
    }

    #[test]
    fn the_shared_handle_outlives_the_image_it_came_from() {
        // What an operation does: take the pixels onto a blocking thread, where nothing borrowed can go, while the
        // caller keeps — or in this case drops — its own image.
        let pixels = {
            let source = Picture::new("/pictures/small.png", image_of(8, 8), "f00df00df00df00d");
            source.shared_pixels()
        };

        assert_eq!((pixels.width(), pixels.height()), (8, 8));
    }
}
