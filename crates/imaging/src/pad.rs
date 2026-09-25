//! Extending a region to the shape a model accepts by mirroring its own edge pixels outwards.
//!
//! The rule is applied as a coordinate map rather than by materialising a padded copy — see
//! [`source_offset`] — so the conversion reads the padded shape straight out of the source with no second buffer.

// Only the right and bottom edges are ever extended: the grid places every tile at an offset inside the image and
// moves a tile that would overhang back rather than shrinking it, so the only region short of the tile shape is the
// single tile covering an image that is smaller than one — and that region starts at the origin.
//
// The alternative, a constant-coloured border, is not a neutral choice: the model treats it as picture content and
// enhances it, and whatever it makes of a flat rectangle then bleeds back across the edge into pixels the image
// actually has.

/// The offset **within the region** that supplies the value at `offset` along an axis the region is `length` pixels
/// long, mirroring the region's own pixels back outwards once past its end.
///
/// Nothing outside the region is ever read.
///
/// Where the extension is wider than the region has pixels to mirror — a region three pixels across padded to a full
/// 256-pixel tile — one reflection is not enough to get back inside, and the mirrored index is then clamped into the
/// region so an outermost pixel is repeated.
///
/// # Panics
///
/// Panics if `length` is zero.
pub fn source_offset(offset: u32, length: u32) -> u32 {
    assert!(length > 0, "an axis of no length has no pixel to mirror");

    if offset < length {
        return offset;
    }

    // `2 * length - offset - 1` is the mirror of `offset` about the region's last pixel, and it goes below zero once
    // the extension is more than one region wide. Signed arithmetic so that underflow is a number to clamp rather
    // than a wrap to the top of `u32`.
    let mirrored = 2 * i64::from(length) - i64::from(offset) - 1;

    // Clamped into `0..length` above, so the value always fits the `u32` the axis is measured in. The clamp case is
    // reachable rather than theoretical, and it is the same fallback every other reflect-pad implementation takes.
    // Never reading outside the region is the property that matters: out-of-range indices read through an accessor
    // that returns transparent black write a hard black border into the very edge the padding exists to make
    // continuous.
    u32::try_from(mirrored.clamp(0, i64::from(length) - 1)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The offsets a region of `length` pixels supplies when it is extended to `shape`.
    fn padded(length: u32, shape: u32) -> Vec<u32> {
        (0..shape).map(|offset| source_offset(offset, length)).collect()
    }

    #[test]
    fn the_region_itself_is_untouched_by_the_padding() {
        // The centre — here the whole of the original, since only the far edges are extended — reads back as itself
        // whatever it is padded out to.
        for length in [1, 2, 7, 64, 255] {
            let offsets = padded(length, 256);

            for offset in 0..length {
                assert_eq!(offsets[offset as usize], offset, "a region of {length} was altered at {offset}");
            }
        }
    }

    #[test]
    fn the_extension_is_the_mirror_of_the_pixels_beside_it() {
        // A 6-pixel region padded to 10: the four added offsets mirror back across the last pixel, so they are the
        // four pixels before it in reverse — 5, 4, 3, 2.
        assert_eq!(padded(6, 10), vec![0, 1, 2, 3, 4, 5, 5, 4, 3, 2]);
    }

    #[test]
    fn an_extension_wider_than_the_region_repeats_an_edge_pixel_and_reads_nothing_outside() {
        // Three pixels padded to a full tile: 2, 1, 0 mirror back, and there the reflection runs out. Everything
        // beyond repeats the pixel it stopped at rather than reading past the region.
        let offsets = padded(3, 256);

        assert_eq!(&offsets[..8], &[0, 1, 2, 2, 1, 0, 0, 0]);
        assert!(offsets.iter().all(|offset| *offset < 3), "the padding read outside a 3-pixel region");

        // The pathological end of the same case: one pixel has nothing to mirror at all.
        assert!(padded(1, 256).iter().all(|offset| *offset == 0));
    }

    #[test]
    fn a_region_that_is_already_the_requested_shape_is_returned_unchanged() {
        for length in [1, 16, 256] {
            assert_eq!(padded(length, length), (0..length).collect::<Vec<_>>());
        }
    }
}
