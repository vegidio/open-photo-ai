//! The photograph moved towards a finished result by the control a run carries.

// Cross-family tier. **The bias never reaches the graph**: it is applied after everything the model contributed,
// which is what lets one session serve every position of a control — dragging a slider re-runs this loop and opens
// nothing.
//
// Deliberately not part of `blend`. That module combines **overlapping model outputs** into one image, and the whole
// of its argument is about seams. What is here has no ramp, no overlap and no seam — it is one photograph and one
// finished result at the same extent, mixed per pixel by a scalar. Filing the two together on the strength of the
// word would make that module mean *things with "blend" in the name*.
//
// Nor does it sit beside the fixed-canvas geometry in `present`. Denoise and sharpen call it too, through `filter`:
// both carry a strength and end exactly this way, and both run on a **tile grid** — they have no square at all. Parked
// under the canvas it would have been promoted a second time by the first family that wanted it without one.

use image::{ImageBuffer, Rgb};

use crate::tensor::{Channel, Sampler};

/// The photograph moved towards — or away from — `corrected` by `bias`, in place.
///
/// `source + bias * (corrected - source)`, per channel in unit space and bounded. At **0** the photograph comes back
/// byte for byte. At **1** the whole correction lands. At a **negative** value the photograph moves along the same
/// line in the opposite direction, which is an extrapolation past itself rather than a second model or a run in
/// reverse.
pub fn blended<T: Channel>(
    source: &Sampler<'_>,
    mut corrected: ImageBuffer<Rgb<T>, Vec<T>>,
    bias: f32,
) -> ImageBuffer<Rgb<T>, Vec<T>>
where
    Rgb<T>: image::Pixel<Subpixel = T>,
{
    // The reference's `BlendWithIntensity`, with the alpha it carries dropped for the reason every pipeline in this
    // crate drops it. The three positions fall out of the arithmetic rather than being special-cased: at 0 the term
    // vanishes, and `to_unit` and `from_unit` are exactly inverse at both depths.
    //
    // Taken by value and handed back rather than written into a third buffer: nothing else holds the correction, and
    // on a 24-megapixel photograph a third full-resolution buffer is 144 MB at `u8` to hold a result that is about to
    // replace it anyway.
    for (x, y, pixel) in corrected.enumerate_pixels_mut() {
        let full = source.rgb(x, y);

        *pixel = Rgb([0, 1, 2].map(|channel| {
            let original = full[channel].to_unit();

            T::from_unit(bias.mul_add(pixel.0[channel].to_unit() - original, original))
        }));
    }

    corrected
}

#[cfg(test)]
mod tests {
    use image::DynamicImage;

    use super::*;

    use crate::test_support::photograph;

    /// A flat correction at `value` in every channel, over an extent, for the blend tests.
    fn flat(width: u32, height: u32, value: u8) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
        ImageBuffer::from_pixel(width, height, Rgb([value; 3]))
    }

    #[test]
    fn a_bias_of_zero_returns_the_photograph_byte_for_byte() {
        // The one place in this file where "unchanged" means exactly that rather than within a level: nothing here
        // goes through a resample, so `to_unit` and `from_unit` are inverse and the blend term is multiplied by zero.
        let source = photograph(23, 17);
        let sampler = Sampler::new(&source);

        let produced = blended(&sampler, flat(23, 17, 200), 0.0);

        for (x, y, pixel) in produced.enumerate_pixels() {
            let [r, g, b] = sampler.rgb(x, y);

            assert_eq!(
                pixel.0,
                [r, g, b].map(|value| u8::from_unit(value.to_unit())),
                "a bias of zero changed ({x}, {y})"
            );
        }
    }

    #[test]
    fn a_bias_of_one_returns_the_correction_unchanged() {
        let source = photograph(23, 17);
        let correction = flat(23, 17, 200);

        let produced = blended(&Sampler::new(&source), correction.clone(), 1.0);

        assert_eq!(produced.as_raw(), correction.as_raw(), "a bias of one did not return the correction");
    }

    #[test]
    fn a_negative_bias_moves_the_photograph_the_other_way_along_the_same_line() {
        // Not a second model and not a run in reverse: the same adjustment, extrapolated past the photograph. So a
        // correction that brightens must darken at -1, and by the same distance it brightened by at 1.
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(9, 9, Rgb([100_u8; 3])));
        let sampler = Sampler::new(&source);

        let brighter = blended(&sampler, flat(9, 9, 160), 1.0);
        let darker = blended(&sampler, flat(9, 9, 160), -1.0);

        assert_eq!(brighter.get_pixel(4, 4).0, [160; 3], "a bias of one did not reach the correction");
        assert_eq!(darker.get_pixel(4, 4).0, [40; 3], "a bias of minus one did not mirror it about the photograph");

        // Stated as the property rather than as the two numbers: the photograph sits exactly between them.
        for channel in 0..3 {
            let (up, down) =
                (i32::from(brighter.get_pixel(4, 4).0[channel]), i32::from(darker.get_pixel(4, 4).0[channel]));

            assert_eq!(up - 100, 100 - down, "the two directions moved by different distances");
        }
    }

    #[test]
    fn an_extrapolation_past_the_channel_is_bounded_rather_than_wrapped() {
        // The `from_unit` bound, on the input that reaches it: a dark photograph and a bright correction at -1 take
        // the result below zero, which without the bound would wrap to the top of the channel — a black shadow
        // rendered white, and the worst-looking failure this arithmetic has.
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(4, 4, Rgb([10_u8; 3])));

        let under = blended(&Sampler::new(&source), flat(4, 4, 250), -1.0);
        let over = blended(&Sampler::new(&source), flat(4, 4, 250), 1.0);

        assert_eq!(under.get_pixel(0, 0).0, [0; 3], "an underflowing blend did not bound to black");
        assert_eq!(over.get_pixel(0, 0).0, [250; 3]);
    }

    #[test]
    fn a_bias_between_the_endpoints_moves_the_photograph_proportionally() {
        // The middle of the range, which neither endpoint test covers and which is what a slider spends its time
        // in: half the bias is half the distance.
        let source = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(4, 4, Rgb([60_u8; 3])));

        for (bias, expected) in [(0.25_f32, 75_u8), (0.5, 90), (0.75, 105)] {
            let produced = blended(&Sampler::new(&source), flat(4, 4, 120), bias);

            assert_eq!(produced.get_pixel(0, 0).0, [expected; 3], "a bias of {bias} did not land proportionally");
        }
    }
}
