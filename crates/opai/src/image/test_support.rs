//! Fixtures the image IO tests share, compiled only under `cfg(test)`.

// A file directly under `image/` because every one of load, save, probe and the round trip needs the same encoded
// bytes, and fixtures that differed between them would make their results incomparable — the round trip's claim that
// both forms of an operation agree rests on all of them describing the *same* picture.
//
// The fixtures are **encoded here rather than committed**. Eight formats times a file each is eight binaries in the
// repository that nobody can read in a diff, and their provenance would be a commit message; generated, the picture
// is the twenty lines below and every format is encoded by the same codec the test is about to exercise.

use std::path::{Path, PathBuf};

use ::image::{DynamicImage, RgbImage};
use rust_sak::image::{EncodeOptions, ImageFormat, encode_writer};

// Above 16x16 deliberately. `rust-sak`'s `image` module records that the bundled AVIF encoder **hangs** on sub-16px
// frames rather than returning an error, so a fixture below that turns a failing test into a CI job that never
// finishes. 32x24 is comfortably clear of it and still encodes in milliseconds.
/// The size every fixture is generated at.
pub(crate) const FIXTURE_SIZE: (u32, u32) = (32, 24);

/// The picture every fixture holds.
///
/// Deliberately detailed rather than flat: a solid colour compresses to the same handful of bytes at every quality,
/// which would make "a lower quality produces a smaller file" unprovable.
pub(crate) fn sample_image() -> DynamicImage {
    // A diagonal gradient crossed with a high-frequency checker, which gives a lossy encoder both something smooth to
    // keep and something awkward to throw away.
    let (width, height) = FIXTURE_SIZE;

    DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
        let gradient = ((x * 255 / width.max(1)) + (y * 255 / height.max(1))) / 2;
        let checker = if (x / 2 + y / 2) % 2 == 0 { 60 } else { 0 };

        image::Rgb([
            (gradient as u8).saturating_add(checker),
            ((255 - gradient) as u8).saturating_sub(checker),
            ((x * 7 + y * 13) % 256) as u8,
        ])
    }))
}

/// [`sample_image`] encoded as `format`, with `options` or the format's defaults.
///
/// # Panics
///
/// Panics, naming the format, if the fixture cannot be encoded.
pub(crate) fn encoded(format: ImageFormat, options: Option<EncodeOptions>) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode_writer(&sample_image(), &mut bytes, format, options)
        .unwrap_or_else(|err| panic!("the {format:?} fixture could not be encoded: {err}"));
    bytes
}

/// [`sample_image`] as PNG. The default fixture: lossless, native, and needing no downloaded codec binaries.
pub(crate) fn written_png() -> Vec<u8> {
    encoded(ImageFormat::Png, None)
}

/// [`sample_image`] as JPEG, for the tests about content outranking a file's name.
pub(crate) fn written_jpeg() -> Vec<u8> {
    encoded(ImageFormat::Jpeg, None)
}

/// [`sample_image`] as TIFF, for the tests about RAW: a RAW file is a TIFF as far as its magic bytes go.
pub(crate) fn written_tiff() -> Vec<u8> {
    encoded(ImageFormat::Tiff, None)
}

/// Writes `bytes` into `dir` under `name` and hands back the path.
pub(crate) fn write_fixture(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    // The name is the test's to choose, because half of what these tests assert is that the name does *not* decide
    // the format — a JPEG called `.png` has to be as easy to write as one called `.jpg`.
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap_or_else(|err| panic!("the fixture {name} could not be written: {err}"));
    path
}

/// Runs `work` against an async runtime that has already shut down, which is what a process on its way down leaves
/// behind, and hands back what it reported.
pub(crate) fn on_a_dead_runtime<T>(work: impl Future<Output = T>) -> T {
    // The runtime is shut down *before* the work is handed to it rather than racing a shutdown against a task already
    // in flight, so the outcome does not depend on which won. `spawn_blocking` targets whichever runtime is current,
    // which is why the dead one is entered for the call while a live current-thread runtime drives the future — a dead
    // runtime cannot poll anything itself.
    let shut_down = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let handle = shut_down.handle().clone();
    shut_down.shutdown_timeout(std::time::Duration::ZERO);

    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
        let _entered = handle.enter();
        work.await
    })
}

// ── The DNG fixture ───────────────────────────────────────────────────────────────────────────────────────────────
//
// Ported from `rust-sak`'s own `src/image/tests.rs`, where the same builder synthesises the fixture its RAW tests
// use. It is duplicated rather than shared because there is no way to share it: it lives in a `#[cfg(test)]`
// module, so it is unreachable from here, and publishing it would mean a `test-support` feature and a `rust-sak`
// release.
//
// What makes the duplication tolerable is that this encodes a *file format* rather than an API. DNG 1.x tag layout
// does not move when `rust-sak` releases, so the two copies cannot drift apart the way two copies of a call site
// would; if `rawler` ever tightens what it accepts, both fail the same way and the pin is a git tag that moves only
// deliberately.
//
// Built rather than committed, for the reason the eight encoded fixtures are: a 10-30 MB camera file in the
// repository is a binary nobody can read in a diff. Building it also makes the tests *say* something — the sensor
// values are known, so an assertion can be "a red CFA develops to a red picture" rather than "it did not error".
//
// `rawler`'s DNG decoder reads each of these tags from the IFD and falls back to a default where one is absent, so
// a minimal file is a supported file rather than a lucky one.

/// TIFF field types, by their on-disk codes.
const BYTE: u16 = 1;
const ASCII: u16 = 2;
const SHORT: u16 = 3;
const LONG: u16 = 4;
const RATIONAL: u16 = 5;
const SRATIONAL: u16 = 10;

/// CFA colour codes, as `CFAPattern` spells them.
pub(crate) const RED: u8 = 0;
pub(crate) const GREEN: u8 = 1;
pub(crate) const BLUE: u8 = 2;

/// The sensor value written where the CFA says "this photosite saw the bright channel".
const BRIGHT: u16 = 60_000;
/// The sensor value written everywhere else.
const DIM: u16 = 2_000;

// Its own constant rather than `FIXTURE_SIZE` because a demosaic has edge handling that a 32x24 frame is small enough
// to let dominate the channel means the tests assert on.
/// The size the DNG fixture is generated at, even in both directions so the 2x2 CFA of a Bayer sensor tiles.
pub(crate) const DNG_FIXTURE_SIZE: (u32, u32) = (64, 64);

/// The make and model the fixture records, which probing reads back out of it.
pub(crate) const DNG_MAKE: &str = "rust-sak";
pub(crate) const DNG_MODEL: &str = "Synthetic DNG";

/// One IFD entry, with its payload not yet placed.
struct Field {
    tag: u16,
    kind: u16,
    count: u32,
    payload: Vec<u8>,
}

fn field(tag: u16, kind: u16, count: u32, payload: Vec<u8>) -> Field {
    Field { tag, kind, count, payload }
}

fn shorts(tag: u16, values: &[u16]) -> Field {
    let payload = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    field(tag, SHORT, values.len() as u32, payload)
}

fn longs(tag: u16, values: &[u32]) -> Field {
    let payload = values.iter().flat_map(|v| v.to_le_bytes()).collect();
    field(tag, LONG, values.len() as u32, payload)
}

fn bytes_field(tag: u16, values: &[u8]) -> Field {
    field(tag, BYTE, values.len() as u32, values.to_vec())
}

fn ascii(tag: u16, text: &str) -> Field {
    let mut payload = text.as_bytes().to_vec();
    payload.push(0);
    let count = payload.len() as u32;
    field(tag, ASCII, count, payload)
}

/// Unsigned rationals, written as numerator/denominator pairs.
fn rationals(tag: u16, values: &[(u32, u32)]) -> Field {
    let payload = values.iter().flat_map(|(n, d)| [n.to_le_bytes(), d.to_le_bytes()]).flatten().collect();
    field(tag, RATIONAL, values.len() as u32, payload)
}

/// Signed rationals, as `ColorMatrix1` needs — a camera matrix has negative terms.
fn srationals(tag: u16, values: &[(i32, i32)]) -> Field {
    let payload = values.iter().flat_map(|(n, d)| [n.to_le_bytes(), d.to_le_bytes()]).flatten().collect();
    field(tag, SRATIONAL, values.len() as u32, payload)
}

/// Builds a complete little-endian DNG: an uncompressed 16-bit Bayer CFA image whose bright photosites are the ones
/// `cfa` labels with `bright_channel`.
pub(crate) fn minimal_dng(width: u32, height: u32, cfa: [u8; 4], bright_channel: u8) -> Vec<u8> {
    // The sensor readings themselves: a photosite of the bright channel is bright, everything else is dim, plus a
    // gentle gradient so the picture is not one flat colour and a transposed row would show.
    let mut strip = Vec::with_capacity((width * height * 2) as usize);
    for y in 0..height {
        for x in 0..width {
            let colour = cfa[((y % 2) * 2 + (x % 2)) as usize];
            let base = if colour == bright_channel { BRIGHT } else { DIM };
            let gradient = ((x + y) * 8) as u16;
            strip.extend(base.saturating_add(gradient).to_le_bytes());
        }
    }

    // The layout is the plain one the TIFF spec describes — 8-byte header, then IFD0, then the payloads too large to
    // sit inside an entry, then the strip. Entries must be written in ascending tag order, which is why this list is
    // sorted rather than grouped by meaning.
    let fields = vec![
        longs(254, &[0]),                  // NewSubfileType: the full-resolution image
        longs(256, &[width]),              // ImageWidth
        longs(257, &[height]),             // ImageLength
        shorts(258, &[16]),                // BitsPerSample
        shorts(259, &[1]),                 // Compression: none
        shorts(262, &[32803]),             // PhotometricInterpretation: CFA
        ascii(271, DNG_MAKE),              // Make
        ascii(272, DNG_MODEL),             // Model
        longs(273, &[0]),                  // StripOffsets — patched once the layout is known
        shorts(277, &[1]),                 // SamplesPerPixel
        longs(278, &[height]),             // RowsPerStrip: the whole image in one strip
        longs(279, &[strip.len() as u32]), // StripByteCounts
        shorts(284, &[1]),                 // PlanarConfiguration: chunky
        shorts(33421, &[2, 2]),            // CFARepeatPatternDim
        bytes_field(33422, &cfa),          // CFAPattern
        bytes_field(50706, &[1, 4, 0, 0]), // DNGVersion 1.4.0.0
        bytes_field(50707, &[1, 1, 0, 0]), // DNGBackwardVersion
        ascii(50708, "opai synthetic"),    // UniqueCameraModel
        shorts(50714, &[0]),               // BlackLevel
        longs(50717, &[65535]),            // WhiteLevel
        // ColorMatrix1 maps XYZ to camera. Identity keeps the fixture's arithmetic legible: whatever the demosaic
        // produces per channel is what the picture shows, so a red CFA cannot come out red by way of a matrix that
        // happened to boost red.
        srationals(50721, &[(1, 1), (0, 1), (0, 1), (0, 1), (1, 1), (0, 1), (0, 1), (0, 1), (1, 1)]),
        // AsShotNeutral of 1:1:1 means no white-balance scaling, for the same reason.
        rationals(50728, &[(1, 1), (1, 1), (1, 1)]),
        shorts(50778, &[21]), // CalibrationIlluminant1: D65
    ];

    // A payload of four bytes or fewer lives inside the entry; anything larger is placed after the IFD and
    // referenced by offset.
    const HEADER: usize = 8;
    let ifd_len = 2 + fields.len() * 12 + 4;
    let heap_start = HEADER + ifd_len;

    let mut heap = Vec::new();
    let mut entries = Vec::with_capacity(fields.len() * 12);
    let mut strip_offset_position = None;

    for f in &fields {
        entries.extend(f.tag.to_le_bytes());
        entries.extend(f.kind.to_le_bytes());
        entries.extend(f.count.to_le_bytes());

        if f.tag == 273 {
            // StripOffsets points at the strip, which sits after the heap — so remember where to patch it.
            strip_offset_position = Some(entries.len());
            entries.extend(0_u32.to_le_bytes());
        } else if f.payload.len() <= 4 {
            let mut inline = f.payload.clone();
            inline.resize(4, 0);
            entries.extend(inline);
        } else {
            entries.extend(((heap_start + heap.len()) as u32).to_le_bytes());
            heap.extend(&f.payload);
            // Offsets must be even, which a trailing odd-length ASCII payload can break.
            if heap.len() % 2 != 0 {
                heap.push(0);
            }
        }
    }

    let strip_start = heap_start + heap.len();
    let position = strip_offset_position.expect("StripOffsets is in the field list");
    entries[position..position + 4].copy_from_slice(&(strip_start as u32).to_le_bytes());

    let mut dng = Vec::with_capacity(strip_start + strip.len());
    dng.extend(b"II"); // little-endian
    dng.extend(42_u16.to_le_bytes()); // the TIFF magic number
    dng.extend((HEADER as u32).to_le_bytes()); // IFD0 starts immediately
    dng.extend((fields.len() as u16).to_le_bytes());
    dng.extend(&entries);
    dng.extend(0_u32.to_le_bytes()); // no IFD1
    dng.extend(&heap);
    dng.extend(&strip);
    dng
}

/// A red-dominant RGGB DNG — the fixture every RAW test wants, and the counterpart of [`written_png`].
pub(crate) fn written_dng() -> Vec<u8> {
    let (width, height) = DNG_FIXTURE_SIZE;
    minimal_dng(width, height, [RED, GREEN, GREEN, BLUE], RED)
}

/// The mean of each channel across the whole picture.
pub(crate) fn channel_means(image: &DynamicImage) -> [f64; 3] {
    let rgb = image.as_rgb16().expect("a developed raw is 16-bit rgb");
    let mut totals = [0.0_f64; 3];
    for pixel in rgb.pixels() {
        for (total, sample) in totals.iter_mut().zip(pixel.0) {
            *total += f64::from(sample);
        }
    }
    let count = rgb.pixels().len() as f64;
    totals.map(|total| total / count)
}
