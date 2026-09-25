//! The feathered elliptical alpha plane a restored face is composited through.
//!
//! An `f32` plane rather than an image, which is the one place this diverges from the reference by choice rather
//! than by language. The reference builds an NRGBA mask because Go's blur takes an image, so its feathered rim —
//! the one region the mask exists to make smooth — is quantised to 256 levels of alpha and then read back out of a
//! byte. A float plane has nothing to round-trip through, the composite reads a weight rather than a colour, and
//! the separable Gaussian below is a dozen lines over it.
//!
//! The kernel is written here rather than taken from `image::imageops::blur`, which takes an `ImageBuffer` of a
//! pixel type and would mean materialising the mask as an image purely to blur it. It differs from
//! `disintegration/imaging`'s by fractions of a percent at the rim — the same class of difference as New York's
//! resampler, and on the more accurate side of it.
//!
//! Nothing here names a variant: the plane is a function of the square it is built for, with the
//! [`LazyLock`] holding the one size both of the family's models run at beside it.

use std::sync::LazyLock;

/// The square the shipping variants restore at, and the size the cached plane below is built for.
const CACHED_EDGE: u32 = 512;

/// Full opacity within this fraction of the half-extent.
const INNER_RADIUS: f32 = 0.7;

/// Zero opacity beyond this fraction of the half-extent — the ellipse inscribed in the square.
const OUTER_RADIUS: f32 = 1.0;

/// The Gaussian's standard deviation, in pixels, at [`CACHED_EDGE`].
const BLUR_SIGMA: f32 = 15.0;

/// How many standard deviations the kernel is truncated at. Beyond three the weights are under 1.2% of the peak and
/// the tail contributes less than the `f32` mantissa carries across a 512-pixel row.
const KERNEL_RADIUS_SIGMAS: f32 = 3.0;

/// The plane for the square the shipping variants restore at, built once for the process.
///
/// The direct analogue of the reference's package-level `sync.Once`. It is a megabyte, immutable, and outlives any
/// run; building it costs tens of milliseconds of separable convolution, and rebuilding it per face would put that
/// inside every measurement this model is ever benchmarked on and inside every face of every export.
static CACHED: LazyLock<Vec<f32>> = LazyLock::new(|| feathered(CACHED_EDGE, BLUR_SIGMA));

/// The feathered alpha plane for an `edge`-pixel square, row-major, `edge * edge` values in `[0, 1]`.
///
/// The cached plane where `edge` is the size the shipping variants run at, and a freshly built one otherwise — so a
/// variant at another square gets a correct mask rather than the wrong one stretched, and pays for it.
pub(crate) fn plane(edge: u32) -> std::borrow::Cow<'static, [f32]> {
    if edge == CACHED_EDGE {
        return std::borrow::Cow::Borrowed(CACHED.as_slice());
    }

    std::borrow::Cow::Owned(feathered(edge, BLUR_SIGMA))
}

/// The same, at a stated `sigma`.
///
/// Split out for the tests alone, and only because [`BLUR_SIGMA`] is a pixel count tuned against the 512-pixel
/// square: at a 64-pixel one it is most of the plane's half-extent and blurs the ellipse away entirely, so the
/// shape properties below are checked at a sigma in the same proportion to their square. The shipping plane's own
/// sigma is pinned separately.
fn feathered(edge: u32, sigma: f32) -> Vec<f32> {
    let mut alpha = ellipse(edge);
    blur(&mut alpha, edge, sigma);

    // The kernel's weights sum to one to within the `f32` mantissa rather than exactly, so a run of opaque taps can
    // accumulate a hair above it. An alpha over one brightens the restoration it weights and one under zero
    // subtracts the background, neither of which anything downstream checks — so the plane is bounded here, once,
    // rather than at every one of the million reads the composite makes of it.
    for value in &mut alpha {
        *value = value.clamp(0.0, 1.0);
    }

    alpha
}

/// The unblurred feathered ellipse: 1.0 inside [`INNER_RADIUS`] of the half-extent, falling linearly to 0.0 at
/// [`OUTER_RADIUS`].
///
/// The radii are fractions of the half-extent rather than pixels, so the shape is inscribed in the square rather
/// than being a true circle on one that is not square. Elliptical rather than rectangular because the corners of an
/// aligned face hold background, hair and whatever else the transform swept in, and they should contribute nothing.
fn ellipse(edge: u32) -> Vec<f32> {
    let extent = edge as usize;
    let mut alpha = vec![0.0_f32; extent * extent];

    let centre = edge as f32 / 2.0;
    let inverse_centre = 1.0 / centre;

    let falloff = OUTER_RADIUS - INNER_RADIUS;
    let inner_squared = INNER_RADIUS * INNER_RADIUS;
    let outer_squared = OUTER_RADIUS * OUTER_RADIUS;

    for y in 0..extent {
        let dy = (y as f32 - centre) * inverse_centre;
        let dy_squared = dy * dy;

        for x in 0..extent {
            let dx = (x as f32 - centre) * inverse_centre;
            let distance_squared = dx * dx + dy_squared;

            alpha[y * extent + x] = if distance_squared <= inner_squared {
                1.0
            } else if distance_squared <= outer_squared {
                // The square root only where the falloff needs it, which is the reference's own arrangement.
                (OUTER_RADIUS - distance_squared.sqrt()) / falloff
            } else {
                0.0
            };
        }
    }

    alpha
}

/// Blurs `alpha` in place with a separable Gaussian at `sigma`, horizontally then vertically.
///
/// Separable because a 2D Gaussian is the product of two 1D ones: two passes of `2r + 1` taps rather than one of
/// `(2r + 1)^2`, which at this radius is 91 multiplies a pixel rather than 8281.
///
/// The edges are handled by **clamping** the tap to the plane's own boundary, not by reflecting it. The plane is
/// zero out to its corners by construction — the ellipse is inscribed in it — so what a clamp repeats at the border
/// is zero, and reflecting it back in would fold the rim's own falloff onto itself.
fn blur(alpha: &mut [f32], edge: u32, sigma: f32) {
    if sigma <= 0.0 || edge == 0 {
        return;
    }

    let extent = edge as usize;
    let kernel = gaussian_kernel(sigma);
    let radius = (kernel.len() / 2) as isize;
    let last = extent as isize - 1;

    let mut scratch = vec![0.0_f32; alpha.len()];

    // The span of columns whose every tap lands inside the row without being clamped. Empty when the plane is
    // narrower than the kernel, in which case every column takes the clamped path below.
    let reach = radius as usize;
    let (interior_start, interior_end) = if extent > 2 * reach { (reach, extent - reach) } else { (0, 0) };

    // Horizontally, into the scratch.
    //
    // Split into a clamped margin and a branch-free interior, which is the treatment `osaka::colorfix` gives its
    // own separable pass and for the same reason: at sigma 15 the kernel is 91 taps, so the clamp inside the
    // innermost loop is tens of millions of signed comparisons for a plane whose interior needs none of them. The
    // taps are summed in kernel order in both halves, so the result is bit-identical to the single loop.
    for y in 0..extent {
        let row = y * extent;

        for x in (0..interior_start).chain(interior_end..extent) {
            let mut total = 0.0_f32;

            for (index, weight) in kernel.iter().enumerate() {
                let tap = (x as isize + index as isize - radius).clamp(0, last) as usize;
                total += alpha[row + tap] * weight;
            }

            scratch[row + x] = total;
        }

        for x in interior_start..interior_end {
            let mut total = 0.0_f32;
            let first = row + x - reach;

            for (index, weight) in kernel.iter().enumerate() {
                total += alpha[first + index] * weight;
            }

            scratch[row + x] = total;
        }
    }

    // Vertically, back into the caller's plane.
    //
    // The row each tap reads from depends only on the output row, so the clamp and the stride multiply are hoisted
    // out of the column loop — `colorfix::convolve_vertical`'s own move. The alternative pays them once per column
    // per tap while striding kilobytes between reads.
    let mut rows = vec![0_usize; kernel.len()];

    for y in 0..extent {
        for (offset, index) in rows.iter_mut().zip(0..kernel.len()) {
            *offset = (y as isize + index as isize - radius).clamp(0, last) as usize * extent;
        }

        for x in 0..extent {
            let mut total = 0.0_f32;

            for (&offset, weight) in rows.iter().zip(kernel.iter()) {
                total += scratch[offset + x] * weight;
            }

            alpha[y * extent + x] = total;
        }
    }
}

/// The normalised 1D Gaussian at `sigma`, truncated at [`KERNEL_RADIUS_SIGMAS`] and of odd length.
///
/// Normalised to sum to exactly one over the taps it keeps rather than over the continuous curve, which is what
/// keeps a fully opaque interior fully opaque: a kernel whose weights summed to 0.994 would dim the centre of every
/// restored face by six parts in a thousand, uniformly and invisibly.
fn gaussian_kernel(sigma: f32) -> Vec<f32> {
    let radius = (sigma * KERNEL_RADIUS_SIGMAS).ceil().max(1.0) as usize;

    let denominator = 2.0 * sigma * sigma;
    let mut weights: Vec<f32> = (0..=2 * radius)
        .map(|index| {
            let offset = index as f32 - radius as f32;

            (-(offset * offset) / denominator).exp()
        })
        .collect();

    let total: f32 = weights.iter().sum();
    for weight in &mut weights {
        *weight /= total;
    }

    weights
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `blur`'s single-loop form: every tap clamped, in kernel order, with no margin split and no hoisted rows.
    ///
    /// Kept as the definition the optimised pass is measured against. The split and the hoist are performance
    /// changes and nothing else, so anything they alter in the output is a defect — which is only checkable
    /// against the shape the optimisation replaced.
    fn blur_unoptimised(alpha: &mut [f32], edge: u32, sigma: f32) {
        if sigma <= 0.0 || edge == 0 {
            return;
        }

        let extent = edge as usize;
        let kernel = gaussian_kernel(sigma);
        let radius = (kernel.len() / 2) as isize;
        let last = extent as isize - 1;

        let mut scratch = vec![0.0_f32; alpha.len()];

        for y in 0..extent {
            let row = y * extent;

            for x in 0..extent {
                let mut total = 0.0_f32;

                for (index, weight) in kernel.iter().enumerate() {
                    let tap = (x as isize + index as isize - radius).clamp(0, last) as usize;
                    total += alpha[row + tap] * weight;
                }

                scratch[row + x] = total;
            }
        }

        for y in 0..extent {
            for x in 0..extent {
                let mut total = 0.0_f32;

                for (index, weight) in kernel.iter().enumerate() {
                    let tap = (y as isize + index as isize - radius).clamp(0, last) as usize;
                    total += scratch[tap * extent + x] * weight;
                }

                alpha[y * extent + x] = total;
            }
        }
    }

    #[test]
    fn the_split_and_hoisted_blur_is_bit_identical_to_the_single_loop_it_replaced() {
        // Sigmas either side of the plane's half-width, so the case where the kernel is wider than the plane — the
        // one where the interior span is empty and every column takes the clamped path — is covered as well as the
        // ordinary one.
        for edge in [1_u32, 2, 7, 16, 33] {
            for sigma in [0.5_f32, 1.5, 4.0, 15.0] {
                // A deterministic plane with no symmetry, so a tap read from the wrong side of centre shows up
                // rather than cancelling out.
                let mut optimised: Vec<f32> =
                    (0..edge * edge).map(|index| ((index as f32) * 0.7).sin().mul_add(0.5, 0.5)).collect();
                let mut reference = optimised.clone();

                blur(&mut optimised, edge, sigma);
                blur_unoptimised(&mut reference, edge, sigma);

                // Exact equality, not a tolerance: the taps are summed in the same order, so the two are the same
                // sequence of operations on the same values and any drift is a reordering that should not be here.
                assert_eq!(optimised, reference, "the blur drifted at edge {edge}, sigma {sigma}");
            }
        }
    }

    /// The alpha at `(x, y)` of an `edge`-square plane.
    fn at(alpha: &[f32], edge: u32, x: u32, y: u32) -> f32 {
        alpha[(y * edge + x) as usize]
    }

    /// [`BLUR_SIGMA`] scaled to a square other than the one it was tuned for, so a small test plane feathers in the
    /// same proportion the shipping one does rather than being blurred away.
    fn proportional_sigma(edge: u32) -> f32 {
        BLUR_SIGMA * edge as f32 / CACHED_EDGE as f32
    }

    #[test]
    fn the_unblurred_plane_is_opaque_at_the_centre_empty_at_the_corners_and_symmetric() {
        // The shape the blur then softens. Opaque over the middle so a restored face is the restoration rather than
        // a blend of it with what was there; empty at the corners because an aligned square's corners hold
        // background and hair rather than face.
        let edge = 64;
        let alpha = ellipse(edge);

        assert_eq!(at(&alpha, edge, edge / 2, edge / 2), 1.0, "the centre is not fully opaque");

        for (x, y) in [(0, 0), (edge - 1, 0), (0, edge - 1), (edge - 1, edge - 1)] {
            assert_eq!(at(&alpha, edge, x, y), 0.0, "({x}, {y}) is a corner and contributes");
        }

        // Symmetric about both axes, which is what makes the ellipse inscribed rather than offset: a mask half a
        // pixel off centre feathers one side of every face more than the other.
        //
        // The centre is the square's own midpoint — `edge / 2`, which lands on a pixel *boundary* — so the mirror of
        // column `x` is column `edge - x`, and column 0 has none. That is the reference's arithmetic, and the
        // alternative reading of "symmetric" would quietly require a different formula.
        for y in 1..edge {
            for x in 1..edge {
                assert!(
                    (at(&alpha, edge, x, y) - at(&alpha, edge, edge - x, y)).abs() < 1e-6,
                    "({x}, {y}) is not the mirror of ({}, {y})",
                    edge - x
                );
                assert!(
                    (at(&alpha, edge, x, y) - at(&alpha, edge, x, edge - y)).abs() < 1e-6,
                    "({x}, {y}) is not the mirror of ({x}, {})",
                    edge - y
                );
            }
        }
    }

    #[test]
    fn the_blur_preserves_the_symmetry_and_leaves_nothing_outside_the_unit_range() {
        // An alpha above one brightens the restoration it weights and one below zero subtracts the background,
        // neither of which is an error anything reports — they are a bright rim and a dark one around every
        // restored face.
        let edge = 48;
        let alpha = feathered(edge, proportional_sigma(edge));

        for value in &alpha {
            assert!((0.0..=1.0).contains(value), "the blurred plane holds {value}");
        }

        // The ellipse is centred on the square's midpoint, which lands on a pixel *boundary*, so its symmetry pairs
        // column `x` with column `edge - x` and leaves column 0 unpaired. The blur's own edge rule is symmetric
        // about the pixel grid's centre instead, which is half a pixel away — so no edge rule can carry the
        // ellipse's symmetry exactly all the way to the border, and what is asserted is that the blur does not
        // *introduce* an asymmetry anywhere it has two real taps to work with.
        let radius = (proportional_sigma(edge) * KERNEL_RADIUS_SIGMAS).ceil() as u32;

        for y in radius + 1..edge - radius {
            for x in radius + 1..edge - radius {
                assert!(
                    (at(&alpha, edge, x, y) - at(&alpha, edge, edge - x, y)).abs() < 1e-5,
                    "the blur broke the horizontal symmetry at ({x}, {y})"
                );
                assert!(
                    (at(&alpha, edge, x, y) - at(&alpha, edge, x, edge - y)).abs() < 1e-5,
                    "the blur broke the vertical symmetry at ({x}, {y})"
                );
            }
        }

        // And at the border, where the two centres are half a pixel apart, the disagreement is that half pixel of
        // the ramp and nothing more. Measured on the **shipping** plane, because that is where a half pixel is a
        // real quantity: the ramp spans some 77 pixels there, so half of one is well under a percent of the alpha,
        // while on a 48-pixel square the same half pixel is a tenth of the whole falloff.
        let shipping = plane(CACHED_EDGE);

        for y in 1..CACHED_EDGE {
            for x in 1..CACHED_EDGE {
                assert!(
                    (at(&shipping, CACHED_EDGE, x, y) - at(&shipping, CACHED_EDGE, CACHED_EDGE - x, y)).abs() < 0.01,
                    "the asymmetry at ({x}, {y}) is more than the half pixel between the two centres"
                );
            }
        }
    }

    #[test]
    fn the_blurred_rim_falls_away_monotonically_from_the_centre_outwards() {
        // What the mask is for: the restoration differs from the surrounding skin in colour and in detail, and a
        // weight that rose again anywhere on the way out would draw a ring around every restored face.
        //
        // Driven over the **shipping** plane rather than a small one, because this is the one property whose numbers
        // are the sigma's own rather than the shape's: at 512 pixels a 15-pixel Gaussian feathers the ramp, and at
        // 64 it would be most of the half-extent.
        let edge = CACHED_EDGE;
        let alpha = plane(edge);
        let middle = edge / 2;

        for x in middle..edge - 1 {
            let here = at(&alpha, edge, x, middle);
            let next = at(&alpha, edge, x + 1, middle);

            assert!(next <= here + 1e-6, "the alpha rose from {here} to {next} going outwards at x = {x}");
        }

        for y in middle..edge - 1 {
            let here = at(&alpha, edge, middle, y);
            let next = at(&alpha, edge, middle, y + 1);

            assert!(next <= here + 1e-6, "the alpha rose from {here} to {next} going outwards at y = {y}");
        }

        // And it has actually feathered rather than staying flat or being washed away: the interior is opaque, the
        // ramp's own start still carries most of its weight, and the rim has nearly given it up. The ellipse is
        // *inscribed*, so it reaches exactly zero at the square's boundary rather than at its last pixel — the
        // corners are where it is zero on the grid.
        assert_eq!(at(&alpha, edge, middle, middle), 1.0, "the blur washed the opaque interior out");
        assert!(
            at(&alpha, edge, middle + middle * 7 / 10, middle) > 0.8,
            "the ramp's start lost most of its weight"
        );
        assert!(at(&alpha, edge, edge - 1, middle) < 0.1, "the rim still carries real weight at the boundary");
        assert_eq!(at(&alpha, edge, edge - 1, edge - 1), 0.0, "the corner contributes after the blur");
    }

    #[test]
    fn the_plane_for_the_shipping_square_is_built_once_and_handed_out_borrowed() {
        // A megabyte of separable convolution per face would land inside every measurement this model is ever
        // benchmarked on. The cache is what the reference's package-level `sync.Once` is, and what makes it checkable
        // is that the borrowed plane is literally the same memory each time rather than an equal copy.
        let first = plane(CACHED_EDGE);
        let second = plane(CACHED_EDGE);

        assert!(matches!(first, std::borrow::Cow::Borrowed(_)), "the shipping square's plane was rebuilt");
        assert_eq!(first.as_ptr(), second.as_ptr(), "two calls produced two planes");
        assert_eq!(first.len(), (CACHED_EDGE * CACHED_EDGE) as usize);

        // A square the cache was not built for is built rather than being served the wrong size, which is what makes
        // the file a function of its square rather than of Athens.
        let other = plane(64);
        assert!(matches!(other, std::borrow::Cow::Owned(_)), "another square was served the cached plane");
        assert_eq!(other.len(), 64 * 64);
    }

    #[test]
    fn the_kernel_sums_to_one_so_the_opaque_interior_stays_opaque() {
        // A kernel normalised over the continuous curve rather than over the taps it keeps would sum slightly below
        // one, and every restored face would be dimmed towards the photograph beneath it by that fraction —
        // uniformly, across the whole interior, with nothing to report it.
        for sigma in [1.0_f32, 5.0, 15.0] {
            let kernel = gaussian_kernel(sigma);

            assert_eq!(kernel.len() % 2, 1, "the kernel has no centre tap at sigma {sigma}");
            assert!((kernel.iter().sum::<f32>() - 1.0).abs() < 1e-6, "the kernel does not sum to one at {sigma}");

            let centre = kernel.len() / 2;
            for offset in 1..=centre {
                assert!((kernel[centre - offset] - kernel[centre + offset]).abs() < 1e-9, "the kernel is lopsided");
                assert!(kernel[centre - offset] <= kernel[centre - offset + 1], "the kernel does not fall outwards");
            }
        }
    }
}
