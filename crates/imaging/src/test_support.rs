//! The picture fixtures every suite that runs pixels shares: the pictures they run, and the stand-in model they run
//! them through.
//!
//! Behind the `test-support` feature as well as `cfg(test)`, because `cfg(test)` does not cross a crate boundary: the
//! driver's own tests use these, and so do `opai`'s pass sequence, chain, run cache and family pipelines, which assert
//! pixel equality against the same gradient and the same stand-in model. Two copies of either is two things that can
//! quietly stop being the same — at which point two tests disagree about what "the picture" or "the model" was, and
//! neither of them is wrong.

use image::{DynamicImage, ImageBuffer, Rgb, Rgba};

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

/// Every variant a source can reach a sampler as, at `width` by `height`, with every channel — alpha included — spread
/// over its whole range, so partial transparency is everywhere rather than at a hand-picked pixel.
pub fn every_variant(width: u32, height: u32) -> Vec<(&'static str, DynamicImage)> {
    let mut state = 0x2545_f491_u32;
    let wide = DynamicImage::ImageRgba16(ImageBuffer::from_fn(width, height, |_, _| {
        Rgba([(); 4].map(|()| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 16) as u16
        }))
    }));

    variants_of(&wide).collect()
}

/// `picture` as every variant a source can reach a sampler as, each named for its layout: the ones a sampler borrows
/// and the ones it has to convert.
///
/// Converted lazily, in this order, so a caller that takes only the first few — a measurement at 24 megapixels — pays
/// for only those.
pub fn variants_of(picture: &DynamicImage) -> impl DoubleEndedIterator<Item = (&'static str, DynamicImage)> + '_ {
    type Convert = fn(&DynamicImage) -> DynamicImage;
    let variants: [(&'static str, Convert); 10] = [
        ("Rgb8", |picture| DynamicImage::ImageRgb8(picture.to_rgb8())),
        ("Rgba8", |picture| DynamicImage::ImageRgba8(picture.to_rgba8())),
        ("Rgb16", |picture| DynamicImage::ImageRgb16(picture.to_rgb16())),
        ("Rgba16", |picture| DynamicImage::ImageRgba16(picture.to_rgba16())),
        ("Luma8", |picture| DynamicImage::ImageLuma8(picture.to_luma8())),
        ("LumaA8", |picture| DynamicImage::ImageLumaA8(picture.to_luma_alpha8())),
        ("Luma16", |picture| DynamicImage::ImageLuma16(picture.to_luma16())),
        ("LumaA16", |picture| DynamicImage::ImageLumaA16(picture.to_luma_alpha16())),
        ("Rgb32F", |picture| DynamicImage::ImageRgb32F(picture.to_rgb32f())),
        ("Rgba32F", |picture| DynamicImage::ImageRgba32F(picture.to_rgba32f())),
    ];

    variants.into_iter().map(move |(name, convert)| (name, convert(picture)))
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
