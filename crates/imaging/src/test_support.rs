//! The picture fixtures every suite that runs pixels shares: the pictures they run, and the stand-in model they run
//! them through.
//!
//! Behind the `test-support` feature as well as `cfg(test)`, because `cfg(test)` does not cross a crate boundary: the
//! driver's own tests use these, and so do `opai`'s pass sequence, chain, run cache and family pipelines, which assert
//! pixel equality against the same gradient and the same stand-in model. Two copies of either is two things that can
//! quietly stop being the same — at which point two tests disagree about what "the picture" or "the model" was, and
//! neither of them is wrong.

use image::{DynamicImage, ImageBuffer, Rgb};

/// A source image whose every pixel is distinct, so a result that reproduces it can only have done so by reading each
/// tile from the right place.
pub fn gradient(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(width, height, |x, y| {
        Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
    }))
}

/// A photograph with something different in every channel and no flat region, so a pixel read from the wrong place
/// is a different number rather than a plausible one.
///
/// Distinct from [`gradient`], which repeats every 256 pixels on each axis and is flat in whole regions of a large
/// image. That is what the tiling tests want — a tile read from the right place reproduces the picture — and it is
/// exactly what a presentation or a blend test must not have, since a margin mirrored from the wrong row would
/// hold the same numbers as the right one. The three moduli here are coprime, so no two positions in any picture
/// this is asked for share a value.
pub fn photograph(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(width, height, |x, y| {
        Rgb([((x * 7 + y * 3) % 256) as u8, ((x * 2 + y * 11) % 251) as u8, ((x + y) % 241) as u8])
    }))
}

/// Enlarges each pixel of `input` into a block, writing the result into `output`: the arithmetic an upscaler's shape
/// does, with none of its detail.
///
/// The scale is read from the two buffers rather than passed, because that is what every caller of this has: a tile
/// function is handed a scratch pair and nothing else, so deriving the factor here is what lets one implementation
/// serve the fixed-scale drivers and the pass sequence alike. Both buffers are square, planar and three-channel,
/// which is the shape `run_tiled` allocates.
pub fn nearest_neighbour(input: &[f32], output: &mut [f32]) {
    let side = ((input.len() / 3) as f64).sqrt() as usize;
    let out_side = ((output.len() / 3) as f64).sqrt() as usize;
    let scale = out_side / side;

    for plane in 0..3 {
        for y in 0..out_side {
            for x in 0..out_side {
                output[plane * out_side * out_side + y * out_side + x] =
                    input[plane * side * side + (y / scale) * side + (x / scale)];
            }
        }
    }
}
