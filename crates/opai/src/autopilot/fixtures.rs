//! Synthetic photographs for the pixel signals' tests, and the errors they are put through: exposure, white balance,
//! grain and blur.

use image::{DynamicImage, ImageBuffer, Rgb, Rgba};
use imaging::tensor::Sampler;

use crate::models::colorization::srgb_to_linear;

/// Applies the sRGB gamma to a linear-light channel, clamped to `[0, 1]`, at 16 bits.
pub(super) fn encode(linear: f64) -> u16 {
    let linear = linear.clamp(0.0, 1.0);
    let encoded = if linear <= 0.003_130_8 { linear * 12.92 } else { 1.055 * linear.powf(1.0 / 2.4) - 0.055 };

    (encoded * 65535.0).round() as u16
}

/// `source` shifted by `stops` and multiplied by per-channel `gains`, both in linear light, at 16 bits so that nothing
/// is re-quantised to 8.
///
/// This is how a camera gets exposure and white balance wrong: both act on the light before the gamma is applied.
pub(super) fn rendered(source: &DynamicImage, stops: f64, gains: [f64; 3]) -> DynamicImage {
    let sampler = Sampler::new(source);
    let scale = 2.0_f64.powf(stops);

    DynamicImage::ImageRgb16(ImageBuffer::from_fn(source.width(), source.height(), |x, y| {
        let rgb = sampler.rgb(x, y);
        Rgb(std::array::from_fn(|channel| {
            encode(srgb_to_linear(f64::from(rgb[channel]) / 65535.0) * scale * gains[channel])
        }))
    }))
}

/// A tungsten cast's gains at `strength`: red up, blue down. 1 is R x1.4, B x0.6.
pub(super) fn warm(strength: f64) -> [f64; 3] {
    [1.0 + 0.4 * strength, 1.0, 1.0 - 0.4 * strength]
}

/// A cool cast's gains at `strength`: red down, blue up. 1 is R x0.7, B x1.4.
pub(super) fn cool(strength: f64) -> [f64; 3] {
    [1.0 - 0.3 * strength, 1.0, 1.0 + 0.4 * strength]
}

/// The tint of one band of [`scene`]: grey, then a red, a green and a blue of equal strength, so that across the
/// bands no channel is favoured and the light the scene was lit by is neutral.
const TINTS: [[f64; 3]; 4] = [[1.0, 1.0, 1.0], [1.0, 0.72, 0.72], [0.72, 1.0, 0.72], [0.72, 0.72, 1.0]];

// The ramp's dark end is where a typical photograph's darkest tail is, not black: the Kodak set's darkest percentile
// has a median of about 0.1. A ramp from black would be a photograph with unusually deep shadows, which two stops of
// overexposure barely lift, and so no test of what overexposure does to a typical one.
/// A correctly exposed and correctly balanced photograph: brightness ramps across the frame from a dark shadow to near
/// white, and every tint in [`TINTS`] runs through the whole ramp.
pub(super) fn scene(width: u32, height: u32) -> DynamicImage {
    ramp(width, height, 0.08)
}

/// [`scene`], with the ramp starting at the encoded level `darkest`.
fn ramp(width: u32, height: u32, darkest: f64) -> DynamicImage {
    DynamicImage::ImageRgb16(ImageBuffer::from_fn(width, height, |x, y| {
        let level = darkest + (0.98 - darkest) * f64::from(x) / f64::from(width - 1);
        let tint = TINTS[(y * TINTS.len() as u32 / height) as usize];

        Rgb(tint.map(|t| encode(srgb_to_linear(level) * t)))
    }))
}

/// A photograph whose every pixel is equal in all three channels, with the same ramp as [`scene`].
pub(super) fn greyscale(width: u32, height: u32) -> DynamicImage {
    scene(width, height).grayscale()
}

/// `source` printed in black and white: its Rec. 709 luma is kept, in every channel. At 16 bits.
pub(super) fn printed(source: &DynamicImage) -> DynamicImage {
    let sampler = Sampler::new(source);

    DynamicImage::ImageRgb16(ImageBuffer::from_fn(source.width(), source.height(), |x, y| {
        let [r, g, b] = sampler.rgb(x, y).map(|sample| f64::from(sample) / 65535.0);
        let linear = srgb_to_linear(0.2126 * r + 0.7152 * g + 0.0722 * b);

        Rgb([encode(linear); 3])
    }))
}

/// `source` with its colour kept at `keep` of its strength: each encoded pixel moved towards the grey of its own Rec.
/// 709 luma, so its brightness is unchanged. 1 is the photograph and 0 is grey.
pub(super) fn desaturated(source: &DynamicImage, keep: f64) -> DynamicImage {
    let sampler = Sampler::new(source);

    DynamicImage::ImageRgb16(ImageBuffer::from_fn(source.width(), source.height(), |x, y| {
        let rgb = sampler.rgb(x, y).map(|sample| f64::from(sample) / 65535.0);
        let luma = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];

        Rgb(rgb.map(|channel| quantise(luma + keep * (channel - luma))))
    }))
}

/// A muted colour photograph, as an overcast street or a foggy field is: [`scene`] at a sixth of its colour, with one
/// small subject in a strong red covering a thirtieth of the frame.
pub(super) fn muted(width: u32, height: u32) -> DynamicImage {
    let ground = desaturated(&scene(width, height), 1.0 / 6.0).into_rgb16();
    let (side_x, side_y) = (width / 5, height / 6);

    DynamicImage::ImageRgb16(ImageBuffer::from_fn(width, height, |x, y| {
        let inside = (width / 2..width / 2 + side_x).contains(&x) && (height / 3..height / 3 + side_y).contains(&y);
        if inside {
            let level = srgb_to_linear(0.3 + 0.4 * f64::from(x - width / 2) / f64::from(side_x.max(1)));
            Rgb([1.0, 0.25, 0.2].map(|tint| encode(level * tint)))
        } else {
            *ground.get_pixel(x, y)
        }
    }))
}

/// A night scene: almost all of it near black, with small light sources at full brightness.
pub(super) fn night(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(width, height, |x, y| {
        if x % 20 < 2 && y % 20 < 2 {
            Rgb([255, 240, 200])
        } else {
            let v = 4 + ((x * 7 + y * 13) % 24) as u8;
            Rgb([v, v, v + 2])
        }
    }))
}

/// A subject on a pure white background: the middle fifth of the frame is a subject whose shadows reach near black,
/// and the rest is white.
pub(super) fn on_white(width: u32, height: u32) -> DynamicImage {
    let subject = ramp(width, height, 0.02).into_rgb16();

    DynamicImage::ImageRgb16(ImageBuffer::from_fn(width, height, |x, y| {
        if (2 * height / 5..3 * height / 5).contains(&y) {
            *subject.get_pixel(x, y)
        } else {
            Rgb([u16::MAX; 3])
        }
    }))
}

/// The middle fifth of [`scene`] cut out onto a fully transparent surround.
pub(super) fn cut_out(width: u32, height: u32) -> DynamicImage {
    let subject = scene(width, height).into_rgb16();

    DynamicImage::ImageRgba16(ImageBuffer::from_fn(width, height, |x, y| {
        let Rgb([r, g, b]) = *subject.get_pixel(x, y);
        let inside = (2 * height / 5..3 * height / 5).contains(&y);

        if inside { Rgba([r, g, b, u16::MAX]) } else { Rgba([0, 0, 0, 0]) }
    }))
}

/// A colourful photograph that is correctly balanced: most of the frame is a saturated green, as a forest is, and the
/// rest is neutral surfaces over the whole brightness ramp.
pub(super) fn foliage(width: u32, height: u32) -> DynamicImage {
    let neutral = greyscale(width, height).into_rgb16();

    DynamicImage::ImageRgb16(ImageBuffer::from_fn(width, height, |x, y| {
        if y < 3 * height / 4 {
            let level = 0.25 + 0.3 * f64::from(x) / f64::from(width - 1);
            Rgb([0.35, 1.0, 0.25].map(|t| encode(srgb_to_linear(level) * t)))
        } else {
            *neutral.get_pixel(x, y)
        }
    }))
}

/// A deterministic source of pseudo-random numbers, so that a synthetic photograph is the same on every run and every
/// platform: SplitMix64.
pub(super) struct Random(u64);

impl Random {
    pub(super) fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform on `[0, 1)`.
    pub(super) fn uniform(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// Standard normal, by Box-Muller.
    pub(super) fn normal(&mut self) -> f64 {
        let u = self.uniform().max(f64::MIN_POSITIVE);
        let v = self.uniform();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}

/// `source` with Gaussian noise added to each encoded channel on its own, as a sensor's is: `sigma` on the 0 to 255
/// scale, at 16 bits. The noise shows in both the brightness and the colour.
pub(super) fn grainy(source: &DynamicImage, sigma: f64, seed: u64) -> DynamicImage {
    let sampler = Sampler::new(source);
    let mut random = Random::new(seed);
    let sigma = sigma / 255.0;

    DynamicImage::ImageRgb16(ImageBuffer::from_fn(source.width(), source.height(), |x, y| {
        Rgb(sampler.rgb(x, y).map(|sample| quantise(f64::from(sample) / 65535.0 + sigma * random.normal())))
    }))
}

/// `source` with Gaussian noise of `sigma` on the 0 to 255 scale added to its red and blue, and green moved against
/// them so that the Rec. 709 luma of every pixel is unchanged: grain in the colour alone.
pub(super) fn colour_grainy(source: &DynamicImage, sigma: f64, seed: u64) -> DynamicImage {
    let sampler = Sampler::new(source);
    let mut random = Random::new(seed);
    let sigma = sigma / 255.0;

    DynamicImage::ImageRgb16(ImageBuffer::from_fn(source.width(), source.height(), |x, y| {
        let [r, g, b] = sampler.rgb(x, y).map(|sample| f64::from(sample) / 65535.0);
        let (red, blue) = (sigma * random.normal(), sigma * random.normal());
        let green = -(0.2126 * red + 0.0722 * blue) / 0.7152;

        Rgb([quantise(r + red), quantise(g + green), quantise(b + blue)])
    }))
}

fn quantise(encoded: f64) -> u16 {
    (encoded.clamp(0.0, 1.0) * 65535.0).round() as u16
}

/// A sharp photograph: flat surfaces of many tones, meeting at hard edges at every scale, over a mid-grey ground.
pub(super) fn detailed(width: u32, height: u32) -> DynamicImage {
    let mut random = Random::new(0x5eed);
    let mut canvas = vec![[0.45, 0.45, 0.45]; (width * height) as usize];

    // From large to small, so the small ones stay on top and every scale of edge is present.
    for index in 0..(width * height / 900) {
        let extent = 80.0 / (1.0 + f64::from(index) / 40.0) + 4.0;
        let (cx, cy) = (random.uniform() * f64::from(width), random.uniform() * f64::from(height));
        let (half_w, half_h) = (extent * (0.3 + random.uniform()), extent * (0.3 + random.uniform()));
        let tone = [0.1 + 0.8 * random.uniform(), 0.1 + 0.8 * random.uniform(), 0.1 + 0.8 * random.uniform()];
        let disk = random.uniform() < 0.5;

        let (x0, x1) = ((cx - half_w).max(0.0) as u32, (cx + half_w).min(f64::from(width)) as u32);
        let (y0, y1) = ((cy - half_h).max(0.0) as u32, (cy + half_h).min(f64::from(height)) as u32);
        for y in y0..y1 {
            for x in x0..x1 {
                let (dx, dy) = ((f64::from(x) + 0.5 - cx) / half_w, (f64::from(y) + 0.5 - cy) / half_h);
                if !disk || dx * dx + dy * dy <= 1.0 {
                    canvas[(y * width + x) as usize] = tone;
                }
            }
        }
    }

    DynamicImage::ImageRgb16(ImageBuffer::from_fn(width, height, |x, y| {
        Rgb(canvas[(y * width + x) as usize].map(quantise))
    }))
}

/// A clean photograph whose surfaces are all covered in fine, high-contrast texture, as grass, fabric or foliage are:
/// patches of woven pattern, each at its own period, orientation, tone and contrast.
pub(super) fn textured(width: u32, height: u32) -> DynamicImage {
    const PATCH: u32 = 48;
    let mut random = Random::new(0x7e47);
    let (across, down) = (width.div_ceil(PATCH), height.div_ceil(PATCH));

    // Period, orientation, tone and contrast of every patch.
    let patches: Vec<[f64; 4]> = (0..across * down)
        .map(|_| {
            [
                2.5 + 8.0 * random.uniform(),
                std::f64::consts::PI * random.uniform(),
                0.3 + 0.4 * random.uniform(),
                0.15 + 0.2 * random.uniform(),
            ]
        })
        .collect();

    DynamicImage::ImageRgb16(ImageBuffer::from_fn(width, height, |x, y| {
        let [period, angle, tone, contrast] = patches[((y / PATCH) * across + x / PATCH) as usize];
        let (x, y) = (f64::from(x), f64::from(y));
        let (u, v) = (x * angle.cos() + y * angle.sin(), y * angle.cos() - x * angle.sin());
        let omega = std::f64::consts::TAU / period;
        let level = tone + contrast * (omega * u).sin() * (omega * v).sin();

        Rgb([1.0, 0.92, 0.8].map(|tint| quantise(level * tint)))
    }))
}

/// `source` convolved with `kernel`, a list of offsets and weights summing to one, with the edges extended. At 16
/// bits.
fn convolved(source: &DynamicImage, kernel: &[(i32, i32, f64)]) -> DynamicImage {
    let sampler = Sampler::new(source);
    let (width, height) = (source.width() as i32, source.height() as i32);

    DynamicImage::ImageRgb16(ImageBuffer::from_fn(source.width(), source.height(), |x, y| {
        let mut sum = [0.0; 3];
        for (dx, dy, weight) in kernel {
            let sx = (x as i32 + dx).clamp(0, width - 1) as u32;
            let sy = (y as i32 + dy).clamp(0, height - 1) as u32;
            for (total, sample) in sum.iter_mut().zip(sampler.rgb(sx, sy)) {
                *total += weight * f64::from(sample);
            }
        }

        Rgb(sum.map(|total| total.round().clamp(0.0, 65535.0) as u16))
    }))
}

/// A normalised kernel from the offsets `inside` accepts within `reach` of the centre.
fn kernel(reach: i32, inside: impl Fn(f64, f64) -> bool) -> Vec<(i32, i32, f64)> {
    let taps: Vec<(i32, i32)> = (-reach..=reach)
        .flat_map(|dy| (-reach..=reach).map(move |dx| (dx, dy)))
        .filter(|(dx, dy)| inside(f64::from(*dx), f64::from(*dy)))
        .collect();
    let weight = 1.0 / taps.len() as f64;

    taps.into_iter().map(|(dx, dy)| (dx, dy, weight)).collect()
}

/// `source` blurred as a lens out of focus blurs it: a disk of `radius` pixels.
pub(super) fn defocused(source: &DynamicImage, radius: f64) -> DynamicImage {
    let reach = radius.ceil() as i32;
    convolved(source, &kernel(reach, |dx, dy| dx * dx + dy * dy <= radius * radius))
}

/// `source` blurred along one direction as camera shake blurs it: a line `length` pixels long at `angle` degrees
/// from the horizontal.
pub(super) fn shaken(source: &DynamicImage, length: f64, angle: f64) -> DynamicImage {
    let (sin, cos) = angle.to_radians().sin_cos();
    let reach = (length / 2.0).ceil() as i32;

    // A tap is on the line when it is within half a pixel of it and within half the length along it.
    convolved(
        source,
        &kernel(reach, |dx, dy| {
            (dx * sin - dy * cos).abs() <= 0.5 && (dx * cos + dy * sin).abs() <= length / 2.0
        }),
    )
}

/// `sharp` in the middle half of the frame across and down, and `blurred` around it: a subject in focus against a
/// background that is not.
pub(super) fn in_focus_against(sharp: &DynamicImage, blurred: &DynamicImage) -> DynamicImage {
    let (sharp, blurred) = (sharp.to_rgb16(), blurred.to_rgb16());
    let (width, height) = sharp.dimensions();

    DynamicImage::ImageRgb16(ImageBuffer::from_fn(width, height, |x, y| {
        let inside = (width / 4..3 * width / 4).contains(&x) && (height / 4..3 * height / 4).contains(&y);
        if inside { *sharp.get_pixel(x, y) } else { *blurred.get_pixel(x, y) }
    }))
}

/// The middle half of `source`, across and down, cut out onto a fully transparent surround.
pub(super) fn cut_out_of(source: &DynamicImage) -> DynamicImage {
    let subject = source.to_rgb16();
    let (width, height) = subject.dimensions();

    DynamicImage::ImageRgba16(ImageBuffer::from_fn(width, height, |x, y| {
        let Rgb([r, g, b]) = *subject.get_pixel(x, y);
        let inside = (width / 4..3 * width / 4).contains(&x) && (height / 4..3 * height / 4).contains(&y);

        if inside { Rgba([r, g, b, u16::MAX]) } else { Rgba([0, 0, 0, 0]) }
    }))
}

/// `source` repeated `across` by `down` times, every other copy mirrored so that no seam is an edge: a photograph many
/// times larger whose every part, at its own resolution, is a part of `source`.
pub(super) fn mirror_tiled(source: &DynamicImage, across: u32, down: u32) -> DynamicImage {
    let tile = source.to_rgb16();
    let (width, height) = tile.dimensions();

    DynamicImage::ImageRgb16(ImageBuffer::from_fn(width * across, height * down, |x, y| {
        let (column, row) = (x / width, y / height);
        let (x, y) = (x % width, y % height);
        let x = if column % 2 == 0 { x } else { width - 1 - x };
        let y = if row % 2 == 0 { y } else { height - 1 - y };

        *tile.get_pixel(x, y)
    }))
}

/// A photograph with no edge anywhere: smooth gradients of brightness and hue across the whole frame, as a clear sky or
/// a fog is.
pub(super) fn smooth(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgb16(ImageBuffer::from_fn(width, height, |x, y| {
        let (u, v) = (f64::from(x) / f64::from(width - 1), f64::from(y) / f64::from(height - 1));
        Rgb([0.35 + 0.3 * u, 0.45 + 0.2 * v, 0.6 + 0.25 * u * v].map(quantise))
    }))
}
