//! Reading, writing and describing image files.
//!
//! Six operations, each offered twice. [`fn@load`] reads a file into a [`Picture`], camera RAW included; [`fn@save`]
//! writes an image to a destination the caller names, in a format the caller names, and [`fn@encode`] produces the same
//! bytes with no file at the end of them; [`fn@identity`] names a file from its bytes without decoding it; [`fn@probe`]
//! reports a file's format, size, colour type and bit depth without decoding it, and [`probe_raw`] does the same for
//! a RAW file, which has different facts to report and a different cost.
//!
//! [`input_extensions`] is the one question rather than an operation: which file extensions [`fn@load`] accepts, for a
//! file picker to filter on and a drag-and-drop to check against. It costs nothing and touches no file, so it has no
//! blocking form to distinguish.
//!
//! # Both forms of every operation
//!
//! The `_blocking` form runs on the calling thread. The plain name is an `async` wrapper that runs it on a blocking
//! thread. Both produce the same result and report the same errors for the same input, except that only the
//! asynchronous form can report [`ImageIoError::Cancelled`].
//!
//! # Camera RAW
//!
//! [`fn@load`] develops camera RAW into a display-ready photograph. **RAW is recognised by extension and never by
//! content**, because most RAW formats carry plain TIFF magic; [`is_raw`] is that check.
//!
//! **47 extensions are recognised and 29 of them decode.** The other 18 — `bay`, `bmq`, `cap`, `cine`, `cs1`,
//! `dc2`, `eip`, `gpr`, `ia`, `k25`, `kc2`, `mdc`, `ptx`, `pxn`, `rdc`, `sr2`, `srf`, `sti` — are refused with
//! [`ImageIoError::UnsupportedRaw`], **before the file is read**.
//!
//! # A developed RAW is 16-bit, and every format writes it
//!
//! [`fn@load`] produces 16 bits per channel for every RAW file, whatever the sensor recorded, and **all eight formats
//! behind [`fn@save`] write it**. A format that cannot hold 16 bits narrows the picture rather than refusing it, so no
//! export fails on the colour type of what was loaded.
//!
//! What differs is what survives the write. **PNG, TIFF, AVIF and HEIF keep 16 bits per channel; BMP, GIF, JPEG and
//! WebP are 8-bit**, so a RAW exported to one of those four comes back at 8.
//!
//! # What is not here
//!
//! No caching: the identity is designed to key one, and the cache itself lives in `crate::cache`. No colour
//! management, EXIF orientation or ICC profiles.

// Both forms exist because the GUI's runtime is shared with the window and the IPC plumbing: a decode is tens of
// milliseconds for a phone photograph and seconds for a 100-megapixel scan, and inline it would stop the event loop
// rather than merely slow this crate down. The CLI and the tests have no runtime to protect, and an `async`-only API
// would force one on them.
//
// Scaling a picture down is deliberately not an operation here. Its one caller is the GUI's rendition handler, turning
// an opened photograph into a thumbnail or a canvas-sized preview, which is a question about how an interface draws
// rather than about reading a file, so it lives beside that handler, in `crates/gui/src/images/serve.rs`. The
// reference draws the same line: its core library exports no resize either, and `cmd/gui`'s image service calls
// `imaging.Resize` itself. The model pipelines reach for `image::imageops::resize` directly, for their own tensor
// preparation and tiling — a different operation, with a different filter and no bound.
//
// The 18 undecodable RAW formats are the remaining gap against the reference, which decodes through LibRaw and covers
// close to everything. The backend here is pure Rust, with no C toolchain and no global state to serialise decodes
// through, and the 18 formats are what that costs.
//
// The 16-bit narrowing is a property of those codecs rather than a choice made here — libwebp is 8-bit, and so is the
// JPEG that predates the sensor — and lives in `rust-sak`'s encode dispatch, where every consumer gets it; nothing in
// this module converts pixels before handing them over.
//
// Colour management, EXIF orientation and ICC profiles are absent because neither the reference nor `rust-sak`
// handles them today, so nothing here differs either way.

mod encode;
mod error;
mod extensions;
mod identity;
mod load;
mod picture;
mod probe;
mod raw;
mod save;
#[cfg(test)]
mod test_log;
#[cfg(test)]
mod test_support;

pub use encode::{encode, encode_blocking};
pub use error::ImageIoError;
pub use extensions::{extensions_of, input_extensions};
pub use identity::{identity, identity_blocking};
pub use load::{load, load_blocking};
pub use picture::Picture;
pub use probe::{identify_raw_blocking, probe, probe_blocking, probe_raw, probe_raw_blocking};
pub use raw::is_raw;
pub use save::{save, save_blocking};

// `rust-sak`'s own vocabulary, re-exported rather than restated. A caller has to name an `ImageFormat` to save and
// reads an `ImageInfo` back from a probe, and making it depend on `rust-sak` directly to spell the arguments to this
// crate's own functions would put a private dependency in its manifest — one it cannot simply add, since `rust-sak`
// is pinned by git tag rather than published to a registry.
//
// `ImageError` is here for the same reason in the opposite direction: it is a public field of [`ImageIoError`], so
// without it a front end could display a decode failure but never match on what the codec actually said.
//
// `RawFormat` and `RawImageInfo` join them for the same reason: [`probe_raw`] hands back a `RawImageInfo`, and
// `RawFormat` is one of its fields. `rust_sak::image::is_raw_bytes` is deliberately **not** among them — it answers
// `true` for a plain TIFF, so it cannot route between the RAW and the TIFF decoder, and [`is_raw`] is the function
// that can. Keeping it off this surface stops it being reached for.
pub use rust_sak::image::{
    Chroma, EncodeOptions, ImageError, ImageFormat, ImageInfo, PngCompression, PngFilter, Preset, RawFormat,
    RawImageInfo,
};

#[cfg(test)]
mod tests {
    use super::*;
    use test_support::{DNG_FIXTURE_SIZE, FIXTURE_SIZE, sample_image, write_fixture, written_dng};

    use std::sync::Arc;
    use tempfile::tempdir;

    /// Every format the module supports, which is every format `rust-sak` supports.
    const EVERY_FORMAT: [ImageFormat; 8] = [
        ImageFormat::Bmp,
        ImageFormat::Gif,
        ImageFormat::Jpeg,
        ImageFormat::Png,
        ImageFormat::Tiff,
        ImageFormat::Avif,
        ImageFormat::Heif,
        ImageFormat::WebP,
    ];

    #[test]
    fn every_supported_format_survives_being_written_and_read_back() {
        // The cross-cutting check: eight formats through three operations, rather than one format proving the seam
        // and the other seven being assumed. Lossy formats are not compared pixel for pixel — what is asserted is
        // that the picture comes back at its own size, in its own format, which is what a failed codec would break.
        let dir = tempdir().expect("a temporary directory");

        for format in EVERY_FORMAT {
            let destination = dir.path().join(format!("round-trip.{}", format.extension()));

            let written = save_blocking(&sample_image(), &destination, format, None)
                .unwrap_or_else(|err| panic!("{format:?} could not be written: {err}"));
            assert!(written > 0, "{format:?} reported success having written nothing");

            let loaded = load_blocking(&destination).unwrap_or_else(|err| panic!("{format:?} did not decode: {err}"));
            assert_eq!(loaded.dimensions(), FIXTURE_SIZE, "{format:?} came back the wrong size");
            assert_eq!(loaded.path(), destination);

            let probed = probe_blocking(&destination).unwrap_or_else(|err| panic!("{format:?} header: {err}"));
            assert_eq!(probed.format, format, "{format:?} was written as something else");
            assert_eq!((probed.width, probed.height), loaded.dimensions(), "{format:?} probed and loaded differently");
        }
    }

    #[tokio::test]
    async fn both_forms_of_every_operation_agree_on_every_format() {
        // The contract the two forms are offered under: a front end that picks the asynchronous one and a CLI that
        // picks the blocking one must not be able to get different answers out of the same file.
        let dir = tempdir().expect("a temporary directory");
        let pixels = Arc::new(sample_image());

        for format in EVERY_FORMAT {
            let extension = format.extension();
            let asynchronous = dir.path().join(format!("async.{extension}"));
            let blocking = dir.path().join(format!("blocking.{extension}"));

            let written_asynchronously = save(Arc::clone(&pixels), &asynchronous, format, None)
                .await
                .unwrap_or_else(|err| panic!("{format:?} could not be written asynchronously: {err}"));
            let written_blocking = save_blocking(&pixels, &blocking, format, None)
                .unwrap_or_else(|err| panic!("{format:?} could not be written: {err}"));

            assert_eq!(written_asynchronously, written_blocking, "{format:?} wrote a different number of bytes");
            assert_eq!(
                std::fs::read(&asynchronous).expect("readable"),
                std::fs::read(&blocking).expect("readable"),
                "{format:?} encoded different bytes through the two forms"
            );

            let loaded_asynchronously = load(&asynchronous).await.expect("decodable");
            let loaded_blocking = load_blocking(&blocking).expect("decodable");

            assert_eq!(loaded_asynchronously.pixels(), loaded_blocking.pixels(), "{format:?} decoded differently");
            assert_eq!(
                loaded_asynchronously.identity(),
                loaded_blocking.identity(),
                "{format:?} produced two identities for one set of bytes"
            );

            let probed_asynchronously = probe(&asynchronous).await.expect("the header is readable");
            let probed_blocking = probe_blocking(&blocking).expect("the header is readable");

            assert_eq!(probed_asynchronously.format, probed_blocking.format);
            assert_eq!(
                (probed_asynchronously.width, probed_asynchronously.height),
                (probed_blocking.width, probed_blocking.height),
                "{format:?} probed differently through the two forms"
            );
            assert_eq!(probed_asynchronously.bit_depth, probed_blocking.bit_depth);
        }
    }

    #[test]
    fn a_developed_raw_is_sixteen_bit_and_every_encoder_writes_it() {
        // Which formats keep the depth is `rust-sak`'s contract and is tested there; what this asserts is what this
        // crate promises — a picture it loaded can be saved in every format it offers.
        let dir = tempdir().expect("a temporary directory");
        let source = load_blocking(write_fixture(dir.path(), "DSC_0001.dng", &written_dng())).expect("decodable");

        assert!(
            source.pixels().as_rgb16().is_some(),
            "a developed RAW is 16-bit RGB, got {:?}",
            source.pixels().color()
        );
        assert_eq!(source.dimensions(), DNG_FIXTURE_SIZE);

        for format in EVERY_FORMAT {
            let destination = dir.path().join(format!("developed.{}", format.extension()));
            let written = save_blocking(source.pixels(), &destination, format, None)
                .unwrap_or_else(|err| panic!("{format:?} refused a 16-bit picture: {err}"));
            assert!(written > 0, "{format:?} reported success having written nothing");

            let reloaded = load_blocking(&destination).unwrap_or_else(|err| panic!("{format:?}: {err}"));
            assert_eq!(reloaded.dimensions(), DNG_FIXTURE_SIZE, "{format:?} came back the wrong size");
        }
    }

    #[test]
    fn no_fixture_is_small_enough_to_hang_the_avif_encoder() {
        // See `FIXTURE_SIZE` for why.
        let (width, height) = FIXTURE_SIZE;

        assert!(width >= 16 && height >= 16, "a {width}x{height} fixture can hang the AVIF encoder");
    }
}
