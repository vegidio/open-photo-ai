//! The samples a colour balance fit is taken over: the un-padded photograph, read back out of a planar CHW tensor
//! as flat RGB triples.

// Family tier rather than Rio's, because the reader is what both of this family's contracts hand their solver.
// Rio builds two views — the square it showed the graph and the square the graph returned — and fits one mapping
// between them; São Paulo builds one source view and one per rendering over a nine-plane output, which is what the
// plane offset below is for. Neither owns it, so neither holds it.
//
// The crop is the point of this, not an optimisation. A fixed-shape graph is shown a square, and a photograph that is
// not square fills the rest of it with a mirror of its own edge — see `imaging::present`. That mirror must not reach
// the fit, and the reason is that this family's correction is **global**: one mapping is fitted across the whole
// photograph and then applied to every pixel of it, so extra border content does not affect only the border. It
// re-weights the mirrored edge against the rest of the picture and moves every pixel of the result.
//
// The reference measured it over 72 photographs — three sources, six aspect ratios, four illuminant casts, scored on
// the final full-resolution image against what the dynamic-shape graph rendered — and cropping first takes the median
// from **47.9 dB to 52.7 dB** and the worst case from 38.8 dB to 41.0 dB. It costs nothing: the extension is only
// ever on the right and the bottom, so the wanted region is already contiguous from the origin.

use imaging::tensor::{Normalisation, TensorShape};

/// How many planes one view reads. Three, because a view is one RGB rendering however many the tensor carries.
const PLANES: usize = 3;

// A view rather than a materialised vector. The reference builds `[][3]float32` for the source and one per
// destination. At Rio's 656 square that is 430,336 triples — about 5 MB — and São Paulo takes three views, one over
// the input tensor and two over the output, which would be 15 MB of copies of data that is already in memory and
// already laid out for a strided read. Nothing here outlives the tensors: the input scratch and the output buffer are
// both owned by the run, and the fit borrows them for its single pass.
//
// An `Iterator` is the alternative, rejected: the fit walks a source and several destinations in lockstep and wants
// them by index, and an indexed view monomorphises to the same loop with no zip.
/// A borrowing view over the un-padded top-left crop of one RGB plane triple inside a planar CHW tensor.
#[derive(Debug)]
pub(super) struct Samples<'a> {
    /// The three planes this view reads, already offset to the first of them.
    data: &'a [f32],
    /// The square the tensor is, which is the stride between one row of a plane and the next.
    canvas: u32,
    /// The photograph's own extent inside that square — the plan's `scaled_width` by `scaled_height`.
    crop: (u32, u32),
    // A parameter rather than assumed, for the reason `presented` takes one: this family declares `Normalisation::Unit`
    // and face recovery's restorers declare the signed range, and a reader that hard-coded either would be a silently
    // wrong fit for the first family with the other. For `Unit` the decode is the identity, so it is free here.
    /// The range the tensor's values are written in, which is what [`triple`](Self::triple) decodes through.
    norm: Normalisation,
}

impl<'a> Samples<'a> {
    /// A view over the `crop` region of the three planes of `tensor` beginning at plane `offset`, of a `canvas`
    /// square, whose values are written in `norm`.
    ///
    /// # Errors
    ///
    /// Returns [`TensorShape`] where `tensor` does not hold three whole planes of the `canvas` square from `offset`
    /// onwards, or where `crop` reaches outside that square.
    pub(super) fn new(
        tensor: &'a [f32],
        offset: u32,
        canvas: u32,
        crop: (u32, u32),
        norm: Normalisation,
    ) -> Result<Self, TensorShape> {
        let plane = (canvas as usize) * (canvas as usize);
        let base = (offset as usize) * plane;
        let expected = base + PLANES * plane;

        if tensor.len() < expected {
            return Err(TensorShape { expected, actual: tensor.len(), width: canvas, height: canvas });
        }

        // Reported as what the crop would have needed rather than as what the tensor holds, exactly as
        // `decode_chw` reports a region reaching past its tensor: the two numbers together are the whole of the
        // mistake, and an error naming the tensor's own extent would describe a buffer that is the right length.
        if crop.0 > canvas || crop.1 > canvas {
            return Err(TensorShape {
                expected: PLANES * (crop.0 as usize) * (crop.1 as usize),
                actual: tensor.len(),
                width: crop.0,
                height: crop.1,
            });
        }

        Ok(Self { data: &tensor[base..expected], canvas, crop, norm })
    }

    /// How many samples the crop holds — the photograph's own pixels inside the square, and none of the extension.
    pub(super) fn len(&self) -> usize {
        (self.crop.0 as usize) * (self.crop.1 as usize)
    }

    /// The sample at `index`, in row-major order over the crop, decoded to the `[0, 1]` unit space the fit works in.
    ///
    /// # Panics
    ///
    /// Panics where `index` is not below [`len`](Self::len).
    pub(super) fn triple(&self, index: usize) -> [f32; 3] {
        assert!(index < self.len(), "sample {index} is outside a view of {} samples", self.len());

        let plane = (self.canvas as usize) * (self.canvas as usize);
        // The stride is the **square's** width rather than the crop's, which is the whole of what the crop is: the
        // wanted region is contiguous from the origin, and each row of it is followed by the extension.
        let element = (index / (self.crop.0 as usize)) * (self.canvas as usize) + index % (self.crop.0 as usize);

        [
            self.norm.decode(self.data[element]),
            self.norm.decode(self.data[plane + element]),
            self.norm.decode(self.data[2 * plane + element]),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `canvas` square of `planes` planes whose every element is distinct and says where it came from:
    /// `plane * 1000 + row * 10 + column`, scaled into the unit range so a signed decode has somewhere to move it.
    fn tensor(planes: usize, canvas: u32) -> Vec<f32> {
        let side = canvas as usize;

        (0..planes * side * side)
            .map(|index| {
                let (plane, position) = (index / (side * side), index % (side * side));
                let (row, column) = (position / side, position % side);

                (plane * 1000 + row * 10 + column) as f32 / 10_000.0
            })
            .collect()
    }

    /// What [`tensor`] holds at one position of one plane, as the raw value before any decode.
    fn element(plane: usize, row: usize, column: usize) -> f32 {
        (plane * 1000 + row * 10 + column) as f32 / 10_000.0
    }

    #[test]
    fn the_count_is_the_crops_rather_than_the_squares() {
        // The first half of the property the 4.8 dB rests on. A view over a landscape photograph on a square holds
        // the photograph's own pixels and none of the mirror beside them.
        let canvas = 8;
        let data = tensor(PLANES, canvas);

        let samples = Samples::new(&data, 0, canvas, (8, 5), Normalisation::Unit).expect("a three-plane square");

        assert_eq!(samples.len(), 40, "the view held the square's pixels rather than the photograph's");

        // And a square photograph needs no crop at all, which is the case where the two counts coincide.
        let whole = Samples::new(&data, 0, canvas, (8, 8), Normalisation::Unit).expect("a three-plane square");
        assert_eq!(whole.len(), 64);
    }

    #[test]
    fn the_last_sample_is_the_photographs_own_corner_rather_than_a_mirrored_one() {
        // The second half, and the one that actually separates a reader that crops from one that merely reads the
        // right *number* of pixels. A flat walk of `len()` elements over a 8x5 crop of an 8-square runs off the end
        // of row 4 and into rows 5 and 6 — which are the mirror — and lands on (7, 4) of the tensor rather than on
        // the photograph's own (7, 4). A count-only assertion passes on exactly that reader.
        let canvas = 8;
        let data = tensor(PLANES, canvas);
        let (crop_width, crop_height) = (6_u32, 5_u32);

        let samples = Samples::new(&data, 0, canvas, (crop_width, crop_height), Normalisation::Unit)
            .expect("a three-plane square");

        let last = samples.triple(samples.len() - 1);
        let corner = [0, 1, 2].map(|plane| element(plane, (crop_height - 1) as usize, (crop_width - 1) as usize));

        assert_eq!(last, corner, "the last sample is not the photograph's own bottom-right pixel");

        // Stated the other way as well, because the flat walk's answer is a real value from a real position and a
        // test that only checked the wanted one would pass if both happened to agree.
        let flat = [0, 1, 2].map(|plane| {
            let position = (crop_width as usize) * (crop_height as usize) - 1;
            element(plane, position / (canvas as usize), position % (canvas as usize))
        });

        assert_ne!(last, flat, "the fixture stopped separating a cropped read from a flat one");
    }

    #[test]
    fn every_sample_inside_the_crop_is_the_pixel_at_that_position() {
        // The whole region rather than its corner: a reader that transposed the axes, or that used the crop's width
        // as the stride, reproduces the first row correctly and nothing after it.
        let canvas = 9;
        let data = tensor(PLANES, canvas);
        let (crop_width, crop_height) = (7_u32, 4_u32);

        let samples = Samples::new(&data, 0, canvas, (crop_width, crop_height), Normalisation::Unit).expect("a square");

        for row in 0..crop_height as usize {
            for column in 0..crop_width as usize {
                let index = row * (crop_width as usize) + column;

                assert_eq!(
                    samples.triple(index),
                    [0, 1, 2].map(|plane| element(plane, row, column)),
                    "sample {index} is not the pixel at ({column}, {row})"
                );
            }
        }
    }

    #[test]
    fn each_channel_comes_from_its_own_plane() {
        // Planar CHW, so the three channels of one pixel are a plane apart rather than adjacent. A reader that had
        // read them as interleaved triples would produce three neighbouring reds and render a grey photograph.
        let canvas = 4;
        let data = tensor(PLANES, canvas);

        let samples = Samples::new(&data, 0, canvas, (4, 4), Normalisation::Unit).expect("a square");
        let [r, g, b] = samples.triple(5);

        assert_eq!([r, g, b], [element(0, 1, 1), element(1, 1, 1), element(2, 1, 1)]);
        assert!(r < g && g < b, "the three channels did not come from three planes: {r}, {g}, {b}");
    }

    #[test]
    fn a_view_reads_the_plane_triple_the_offset_names() {
        // The parameter São Paulo needs: it reads its second rendering out of planes 6, 7 and 8 of one nine-plane
        // output, and a view that ignored the offset would fit that rendering to the weight maps.
        let canvas = 5;
        let data = tensor(9, canvas);

        for offset in [0_u32, 3, 6] {
            let samples =
                Samples::new(&data, offset, canvas, (5, 5), Normalisation::Unit).expect("a nine-plane square");

            assert_eq!(
                samples.triple(7),
                [0, 1, 2].map(|plane| element(offset as usize + plane, 1, 2)),
                "the view at plane {offset} did not read that rendering"
            );
        }
    }

    #[test]
    fn a_signed_range_decodes_differently_from_the_unit_one() {
        // The parameter that costs nothing today and is the difference between a fit and nonsense for the first
        // family whose graphs are trained on the signed range. Checked as the relation between the two rather than
        // against either alone, because `decode` is what both this and an expectation written here would call.
        let canvas = 6;
        let data = tensor(PLANES, canvas);

        let unit = Samples::new(&data, 0, canvas, (6, 6), Normalisation::Unit).expect("a square");
        let signed = Samples::new(&data, 0, canvas, (6, 6), Normalisation::Signed).expect("a square");

        for index in 0..unit.len() {
            let (plain, moved) = (unit.triple(index), signed.triple(index));

            assert_ne!(plain, moved, "sample {index} decoded alike under both ranges");

            for channel in 0..3 {
                assert!(
                    (moved[channel] - plain[channel].mul_add(0.5, 0.5)).abs() < 1e-6,
                    "sample {index} channel {channel}: {} is not {} read in the signed range",
                    moved[channel],
                    plain[channel]
                );
            }
        }
    }

    #[test]
    fn a_tensor_that_does_not_hold_the_planes_the_offset_names_is_refused() {
        // The caller's own allocation disagreeing with the geometry it planned against, which for this family is a
        // plane offset past the end of an output — the one way São Paulo can get its layout wrong and be handed
        // numbers anyway.
        let canvas = 4;
        let data = tensor(PLANES, canvas);

        let error = Samples::new(&data, 1, canvas, (4, 4), Normalisation::Unit).expect_err("a fourth plane");
        assert_eq!(error.expected, 4 * 16);
        assert_eq!(error.actual, 3 * 16);

        // And a crop reaching outside the square, reported as what the crop would have needed.
        let error = Samples::new(&data, 0, canvas, (5, 4), Normalisation::Unit).expect_err("a crop past the square");
        assert_eq!((error.width, error.height), (5, 4));
    }

    #[test]
    #[should_panic(expected = "is outside a view of")]
    fn a_sample_past_the_crop_is_a_contradiction_rather_than_a_neighbouring_pixel() {
        // Without the bound the index runs into the extension and answers with a mirrored pixel, which is the one
        // value this whole module exists to keep out of a fit.
        let canvas = 8;
        let data = tensor(PLANES, canvas);

        let samples = Samples::new(&data, 0, canvas, (8, 5), Normalisation::Unit).expect("a square");
        let _ = samples.triple(samples.len());
    }
}
