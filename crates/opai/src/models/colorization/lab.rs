//! The round trip through CIELab that every colorization graph's contract is built on.
//!
//! The constants and piecewise thresholds follow OpenCV's float Lab conversion (sRGB, D65), which is the convention the
//! graphs were trained against: L on `[0, 100]` and a/b zero-centred. It is **not** the 8-bit encoding that scales L by
//! 255/100 and offsets a/b by 128. That encoding renders a plausible photograph and raises no error, which is why the
//! first test in this module pins OpenCV's own values.
//!
//! The sRGB transfer is here only because Lab needs it.
//!
//! # The fast paths
//!
//! The compose runs these conversions twice per photograph pixel at full resolution, where the transcendental calls
//! dominate. Two properties make most of them avoidable without changing a single output value:
//!
//! 1. Only L is ever needed from the forward conversion at full resolution, and L depends only on Y. Two of the three
//!    cube roots and both chroma terms are dead work there, which is what [`lightness`] skips.
//! 2. Every channel that reaches the forward path is the `u16` sample
//!    [`Sampler::rgb`](imaging::tensor::Sampler::rgb) returns, so the linearization has at most 65,536
//!    distinct arguments and one table holds them all. The reverse path's 8-bit result has only 256 distinct values,
//!    so [`srgb_u8`] finds it among 255 step boundaries rather than through a `powf`.
//!
//! Each fast path is pinned **bit for bit** against the general expression it replaces, never within a tolerance. A
//! fast path that drifted would render a slightly wrong photograph with no error.

use std::sync::LazyLock;

use imaging::tensor::Channel;

/// The D65 white point's X.
const XN: f64 = 0.950456;

/// The D65 white point's Z.
const ZN: f64 = 1.088754;

/// `(6/29)^3`, where the cube root gives way to the linear segment.
const T0: f64 = 0.008856;

/// `29^3 / 3^3`, the slope of L below [`T0`].
const K: f64 = 903.3;

/// The CIE forward transfer `f(t)`, with OpenCV's linear-segment approximation below the threshold.
fn f(t: f64) -> f64 {
    if t > T0 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 }
}

/// Inverts [`f`].
fn f_inv(ft: f64) -> f64 {
    let t = ft * ft * ft;

    if t > T0 { t } else { (ft - 16.0 / 116.0) / 7.787 }
}

/// Removes the sRGB gamma from a `[0, 1]` channel.
fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// Applies the sRGB gamma to a linear-light channel, clamped to `[0, 1]`.
fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.0 {
        return 0.0;
    }

    let encoded = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };

    // Not `f64::min`, which answers 1 for a NaN. A NaN stays NaN here, as it does in the reference, and then encodes to
    // zero through `Channel::from_unit`, which is also what `srgb_u8`'s search answers for it; the two cannot disagree.
    if encoded > 1.0 { 1.0 } else { encoded }
}

/// An sRGB colour's CIELab, D65. Channels are on `[0, 1]`; L comes back on `[0, 100]` and a/b zero-centred, roughly
/// `[-128, 127]`.
pub(super) fn rgb_to_lab([r, g, b]: [f32; 3]) -> [f32; 3] {
    let lr = srgb_to_linear(f64::from(r));
    let lg = srgb_to_linear(f64::from(g));
    let lb = srgb_to_linear(f64::from(b));

    let x = (0.412453 * lr + 0.357580 * lg + 0.180423 * lb) / XN;
    let y = 0.212671 * lr + 0.715160 * lg + 0.072169 * lb;
    let z = (0.019334 * lr + 0.119193 * lg + 0.950227 * lb) / ZN;

    let (fx, fy, fz) = (f(x), f(y), f(z));
    let l = if y > T0 { 116.0 * fy - 16.0 } else { K * y };

    [l as f32, (500.0 * (fx - fy)) as f32, (200.0 * (fy - fz)) as f32]
}

/// The inverse of [`rgb_to_lab`] stopped one step short: linear-light channels, unclamped and still in `f64`, for a
/// caller that finishes with [`srgb_u8`] or [`srgb_u16`] rather than a float gamma encode.
pub(super) fn lab_to_linear_rgb(l: f32, a: f32, b: f32) -> [f64; 3] {
    let l = f64::from(l);

    let (y, fy) = if l > K * T0 {
        let fy = (l + 16.0) / 116.0;
        (fy * fy * fy, fy)
    } else {
        let y = l / K;
        (y, 7.787 * y + 16.0 / 116.0)
    };

    let x = XN * f_inv(fy + f64::from(a) / 500.0);
    let z = ZN * f_inv(fy - f64::from(b) / 200.0);

    [
        3.240479 * x - 1.537150 * y - 0.498535 * z,
        -0.969256 * x + 1.875992 * y + 0.041556 * z,
        0.055648 * x - 0.204043 * y + 1.057311 * z,
    ]
}

/// The inverse of [`rgb_to_lab`], each channel clamped to `[0, 1]`.
///
/// Only the tests' oracle: nothing in the reference's production path calls it, and nothing here does either.
#[cfg(test)]
pub(super) fn lab_to_rgb(l: f32, a: f32, b: f32) -> [f32; 3] {
    lab_to_linear_rgb(l, a, b).map(|c| linear_to_srgb(c) as f32)
}

// Keyed by the `u16` sample rather than by the byte, as the reference's 256 entries are, so that one path serves both
// depths. An 8-bit channel `v` arrives as `v * 257`, and the entry there is the reference's entry for `v` bit for bit;
// a 16-bit source, including a translucent 8-bit pixel premultiplied off the `v * 257` grid, is covered with no second
// path. The `f32` division is deliberate: it is the value `rgb_to_lab` receives from a caller holding unit floats, and
// computing the table through the same expression is what lets `lightness` match `rgb_to_lab`'s L bit for bit.
//
// Lazy, because it is 65,536 `powf` calls and only a process that colorizes something should pay for them; and not a
// constant, because `powf` is not `const` and a 65,536-entry float literal is unreviewable.
/// The linear-light value of every sample [`Sampler::rgb`](imaging::tensor::Sampler::rgb) can return.
static LINEAR: LazyLock<Box<[f64; 65536]>> = LazyLock::new(|| {
    (0..=u16::MAX)
        .map(|w| srgb_to_linear(f64::from(f32::from(w) / 65535.0)))
        .collect::<Box<[f64]>>()
        .try_into()
        .expect("one entry per u16")
});

/// The linear-light value of one sample.
fn linear(sample: u16) -> f64 {
    LINEAR[usize::from(sample)]
}

/// The CIELab L of one pixel, bit for bit the first channel of [`rgb_to_lab`] on the same pixel as unit floats.
///
/// L depends only on the luminance Y, so this is the forward conversion with two of its three cube roots and both
/// chroma terms left out, and its linearization read from a table.
pub(super) fn lightness(pixel: [u16; 3]) -> f32 {
    let y = luminance(pixel);

    if y > T0 { (116.0 * f(y) - 16.0) as f32 } else { (K * y) as f32 }
}

/// The sRGB-encoded gray level carrying the same luminance as one pixel, on `[0, 1]`. Written to all three channels,
/// it is the gray rendering Delhi's and Mumbai's graphs are fed.
///
/// This is `lab_to_rgb(lightness(pixel), 0, 0)` collapsed to one gamma encode of Y. At zero chroma the Lab round trip
/// is the identity on the luminance: the inverse undoes the very cube root the forward conversion applied, and the rows
/// of the XYZ-to-linear matrix sum to 1.000003, 1.000004 and 0.999993 against the D65 white point. So the L scaling
/// cancels and all three channels collapse to one encode. Going through Lab would cost a cube root, two cubes and three
/// `powf` per pixel to compute a value the luminance already had.
pub(super) fn gray(pixel: [u16; 3]) -> f32 {
    linear_to_srgb(luminance(pixel)) as f32
}

/// The luminance Y of one pixel, from the linearization table: what [`lightness`] and [`gray`] are both functions of.
fn luminance([r, g, b]: [u16; 3]) -> f64 {
    0.212671 * linear(r) + 0.715160 * linear(g) + 0.072169 * linear(b)
}

/// The 8-bit encode [`srgb_u8`] replaces, and the one its thresholds are found against.
///
/// This crate's own rounding rule rather than the reference's `x * 255 + 0.5` truncation, so that the fast path and
/// every other family's general path cannot disagree.
fn srgb_u8_direct(c: f64) -> u8 {
    u8::from_unit(linear_to_srgb(c) as f32)
}

// Each boundary is found by bisecting the `f64` bit pattern rather than the value: for positive floats the bit order
// is the value order, so the search converges on the exact float at which the encoded byte changes. About 62
// iterations each, some 16,000 evaluations in all, which is why it is lazy: the reference deferred it out of its
// `init()` for the same reason, because every binary linking the package paid for it whether it encoded a pixel or
// not.
/// `THRESHOLDS[k]` is the smallest linear-light value that encodes to byte `k + 1`.
static THRESHOLDS: LazyLock<[f64; 255]> = LazyLock::new(|| {
    std::array::from_fn(|index| {
        let step = index as u8 + 1;
        let (mut lo, mut hi) = (0.0_f64.to_bits(), 1.0_f64.to_bits());

        while lo < hi {
            let mid = lo + (hi - lo) / 2;

            if srgb_u8_direct(f64::from_bits(mid)) >= step {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }

        f64::from_bits(lo)
    })
});

/// Gamma-encodes a linear-light channel straight to the byte it rounds to: eight comparisons in place of a `powf`.
///
/// Equal to `u8::from_unit(linear_to_srgb(c) as f32)` for every `c`. The encode is monotonic, so counting the step
/// boundaries at or below `c` is the byte.
pub(super) fn srgb_u8(c: f64) -> u8 {
    let thresholds = &*THRESHOLDS;
    let (mut lo, mut hi) = (0, thresholds.len());

    while lo < hi {
        let mid = (lo + hi) / 2;

        if thresholds[mid] <= c {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    lo as u8
}

// No threshold table: 65,535 boundaries would be some 4 million evaluations to find, which is more than the table
// would save on one photograph.
/// Gamma-encodes a linear-light channel to the 16-bit value it rounds to.
pub(super) fn srgb_u16(c: f64) -> u16 {
    u16::from_unit(linear_to_srgb(c) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every 8-bit level widened to the sample [`Sampler::rgb`](imaging::tensor::Sampler::rgb) returns for
    /// it.
    fn widened(v: u8) -> u16 {
        u16::from(v) * 257
    }

    /// A coprime-strided sweep of the 8-bit cube, so that no channel's stride aliases another's.
    fn strided_bytes() -> impl Iterator<Item = [u8; 3]> {
        (0..=255_u8)
            .step_by(3)
            .flat_map(|r| (0..=255_u8).step_by(5).flat_map(move |g| (0..=255_u8).step_by(7).map(move |b| [r, g, b])))
    }

    #[test]
    fn rgb_to_lab_agrees_with_opencv_float_lab() {
        // OpenCV's own float Lab values. These catch the classic porting bug of implementing the 8-bit encoding, L
        // scaled by 255/100 and a/b offset by 128, instead of the float convention.
        let cases = [
            ("black", [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
            ("white", [1.0, 1.0, 1.0], [100.0, 0.0, 0.0]),
            ("red", [1.0, 0.0, 0.0], [53.241, 80.092, 67.203]),
            ("green", [0.0, 1.0, 0.0], [87.735, -86.183, 83.179]),
            ("blue", [0.0, 0.0, 1.0], [32.297, 79.188, -107.860]),
            ("mid-gray", [0.5, 0.5, 0.5], [53.389, 0.0, 0.0]),
        ];

        for (name, rgb, want) in cases {
            let got = rgb_to_lab(rgb);

            assert!(
                got.iter().zip(want).all(|(got, want)| (f64::from(*got) - want).abs() <= 0.05),
                "{name}: rgb_to_lab({rgb:?}) = {got:?}, want {want:?}"
            );
        }
    }

    #[test]
    fn a_colour_survives_the_round_trip_through_lab() {
        const STEPS: u8 = 16;

        for r in 0..=STEPS {
            for g in 0..=STEPS {
                for b in 0..=STEPS {
                    let rgb = [r, g, b].map(|c| f32::from(c) / f32::from(STEPS));
                    let [l, a, lb] = rgb_to_lab(rgb);
                    let back = lab_to_rgb(l, a, lb);

                    assert!(
                        rgb.iter().zip(back).all(|(one, two)| (one - two).abs() <= 1e-3),
                        "{rgb:?} came back as {back:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn out_of_gamut_lab_comes_back_inside_the_unit_range() {
        // A colorization graph can emit any a/b pair.
        for [l, a, b] in [
            [50.0, 200.0, -200.0],
            [100.0, 128.0, 128.0],
            [0.0, -128.0, 127.0],
            [120.0, 0.0, 0.0],
            [-10.0, 0.0, 0.0],
        ] {
            let rgb = lab_to_rgb(l, a, b);

            assert!(rgb.iter().all(|c| (0.0..=1.0).contains(c)), "Lab({l}, {a}, {b}) = {rgb:?}");
        }
    }

    #[test]
    fn a_gray_has_no_chroma() {
        // What the pipeline relies on when it rebuilds the graph's input as (L, 0, 0).
        for v in 0..=255_u8 {
            let level = f32::from(v) / 255.0;
            let [_, a, b] = rgb_to_lab([level; 3]);

            assert!(a.abs() <= 0.01 && b.abs() <= 0.01, "gray {level} has chroma ({a}, {b})");
        }
    }

    #[test]
    fn the_table_is_the_transfer_at_every_sample() {
        for w in 0..=u16::MAX {
            assert_eq!(linear(w).to_bits(), srgb_to_linear(f64::from(f32::from(w) / 65535.0)).to_bits(), "sample {w}");
        }
    }

    #[test]
    fn the_table_holds_the_reference_eight_bit_entries_where_a_byte_lands() {
        // The reference's own formula, `float64(float32(uint32(v)*257) / 65535.0)`, spelled from the byte.
        for v in 0..=255_u8 {
            let reference = srgb_to_linear(f64::from((u32::from(v) * 257) as f32 / 65535.0));

            assert_eq!(linear(widened(v)).to_bits(), reference.to_bits(), "byte {v}");
        }
    }

    #[test]
    fn lightness_is_the_first_channel_of_the_full_conversion() {
        // At full resolution, so any divergence would be a silent, image-wide luminance shift. The 8-bit sweep is the
        // reference's; the 16-bit one reaches samples not of the form `v * 257`.
        let bytes = strided_bytes().map(|pixel| pixel.map(widened));
        let samples = (0..=u16::MAX).step_by(997).flat_map(|r| {
            (0..=u16::MAX)
                .step_by(1009)
                .flat_map(move |g| (0..=u16::MAX).step_by(1013).map(move |b| [r, g, b]))
        });

        for pixel in bytes.chain(samples).chain([[u16::MAX; 3]]) {
            let [want, ..] = rgb_to_lab(pixel.map(|w| f32::from(w) / 65535.0));

            assert_eq!(lightness(pixel).to_bits(), want.to_bits(), "{pixel:?}: {} != {want}", lightness(pixel));
        }
    }

    #[test]
    fn gray_is_the_lab_round_trip_at_zero_chroma() {
        // Not bit-identical, and not required to be: the point is that the difference is far below the quantization
        // the value feeds into, which is what makes skipping the round trip safe. One 8-bit step is 1/255, so this
        // leaves four orders of magnitude of headroom.
        let diagonal = (0..=255_u8).map(|v| [v; 3]);

        for pixel in diagonal.chain(strided_bytes()).map(|pixel| pixel.map(widened)) {
            let [want, ..] = lab_to_rgb(lightness(pixel), 0.0, 0.0);
            let got = gray(pixel);

            assert!((got - want).abs() <= 1e-4, "{pixel:?}: got {got}, want {want}");
        }
    }

    #[test]
    fn srgb_u8_is_the_direct_encode_at_every_step_boundary() {
        // The encode is monotonic, so pinning all 255 boundaries, and either side of each, pins it everywhere.
        for (index, &at) in THRESHOLDS.iter().enumerate() {
            let step = index as u8 + 1;
            let below = f64::from_bits(at.to_bits() - 1);

            assert_eq!(srgb_u8(at), srgb_u8_direct(at), "at threshold {step}");
            assert_eq!(srgb_u8(at), step, "at threshold {step}");
            assert_eq!(srgb_u8(below), srgb_u8_direct(below), "below threshold {step}");
            assert_eq!(srgb_u8(below), step - 1, "below threshold {step}");
        }

        // Out-of-range inputs saturate the way the direct encode does.
        for c in [-1.0, -0.0001, 0.0, 1.0, 1.5, 42.0, f64::NAN] {
            assert_eq!(srgb_u8(c), srgb_u8_direct(c), "{c}");
        }

        // A dense sweep across the whole range, as a second opinion on the boundary argument.
        for i in 0..=200_000 {
            let c = -0.05 + f64::from(i) * (1.1 / 200_000.0);

            assert_eq!(srgb_u8(c), srgb_u8_direct(c), "{c}");
        }
    }

    #[test]
    fn the_split_conversion_encodes_to_what_the_whole_one_does() {
        for l in (0..=100_u8).step_by(2) {
            for a in (-120..=120_i8).step_by(15) {
                for b in (-120..=120_i8).step_by(15) {
                    let (l, a, b) = (f32::from(l), f32::from(a), f32::from(b));
                    let whole = lab_to_rgb(l, a, b).map(u8::from_unit);
                    let split = lab_to_linear_rgb(l, a, b).map(srgb_u8);

                    assert_eq!(split, whole, "Lab({l}, {a}, {b})");
                }
            }
        }
    }

    #[test]
    fn srgb_u16_gives_back_every_sample_it_was_handed() {
        // The scalar form of a 16-bit photograph coming back from the compose unchanged when no chroma is added.
        for w in 0..=u16::MAX {
            assert_eq!(srgb_u16(linear(w)), w, "sample {w}");
        }
    }
}
