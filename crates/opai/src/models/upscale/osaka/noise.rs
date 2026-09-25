//! The per-region noise field: deterministic, seeded from where the region is rather than from when it ran.
//!
//! # Why it is seeded at all
//!
//! A diffusion model with a fresh seed each run hands the user a different image every time — and the per-operation
//! run cache would memoize the first one, so the cached result and a re-run would disagree, and an A/B comparison of
//! two settings would be comparing two noise fields as much as two settings. The seed is what makes a run
//! reproducible, which is the property the cache's correctness rests on.
//!
//! # Why it is keyed on the region's origin
//!
//! Deriving the key from `(seed, origin_x, origin_y)` rather than drawing from one stream across the image gives a
//! second property: a region's noise does not depend on **how many regions preceded it**, so changing the tile
//! geometry does not reshuffle the noise of every region that kept its position. What that buys is a tuning change
//! to the overlap costing only the regions it actually moved, and it is the reason the derivation is keyed on the
//! origin rather than counted off a single stream.
//!
//! # It does not reproduce the Go application's field, and that is deliberate
//!
//! Go seeds with `math/rand/v2.NewChaCha8`, which is not the plain ChaCha8 stream but Go's own **ChaCha8Rand**
//! construction, with a reseeding schedule of its own; no Rust crate produces it. Reproducing it would mean
//! implementing ChaCha8Rand, and no property that matters here would be bought by it — every use of the seed above is
//! about determinism *within* one implementation. The consequence is that the same image at the same settings is not
//! pixel-identical between the two applications. Nothing in this project claims it is. See `openspec` design D6.
//!
//! The primitive is the same; only the construction differs.

use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};

/// The seed the noise field is derived from, fixed across runs.
///
/// A constant rather than a setting: what it buys is reproducibility, and a seed a user could change would be a
/// control over nothing they can perceive, attached to a cache key they cannot see.
pub(crate) const NOISE_SEED: u64 = 0x5EED_BEEF;

/// The smallest uniform draw the log below is taken of.
///
/// Box-Muller's radius is `sqrt(-2 ln u)`, which is infinite at `u = 0`, and an infinity in the latent poisons the
/// whole region through the transformer's projection. The floor is what the reference uses and costs nothing: the
/// probability of drawing below it is about one in a trillion.
const MIN_UNIFORM: f64 = 1e-12;

/// Fills `dest` with a standard normal field for the region at `(origin_x, origin_y)`.
///
/// The same origin always produces the same field, whatever ran before it, and two origins produce different ones.
///
/// Shaped by Box-Muller, two samples per pair of uniform draws. The uniforms are built here from
/// [`Rng::next_u64`] rather than taken from `rand`'s distributions, and that is the point of not depending on
/// `rand` at all: a distribution's exact bit-level output is not something this project should depend on staying
/// fixed, and the whole purpose of this generator is that its output must not move. Owning the conversion makes
/// reproducibility rest on `rand_chacha`'s documented stream and nothing else.
pub(crate) fn gaussian(dest: &mut [f32], origin_x: u32, origin_y: u32, seed: u64) {
    let mut rng = ChaCha8Rng::from_seed(key(seed, origin_x, origin_y));

    // Two at a time, because Box-Muller produces a pair from one pair of uniforms. An odd length takes the first of
    // the pair and discards the second rather than leaving the last element unwritten.
    let mut index = 0;
    while index < dest.len() {
        let radius = (-2.0 * unit(&mut rng).max(MIN_UNIFORM).ln()).sqrt();
        let angle = 2.0 * std::f64::consts::PI * unit(&mut rng);

        // One argument reduction for the pair rather than two: `sin_cos` is the same value on every platform Rust
        // supports, and this sits on the serial path between the encoder and the transformer.
        let (sin, cos) = angle.sin_cos();

        dest[index] = (radius * cos) as f32;
        if index + 1 < dest.len() {
            dest[index + 1] = (radius * sin) as f32;
        }

        index += 2;
    }
}

/// The 32-byte key for the region at `(origin_x, origin_y)`, little-endian throughout.
///
/// Three little-endian `u64`s and then zeros, which is the reference's own layout — kept because the layout is what
/// decides the stream, so changing it would change every noise field for no gain. The origins are widened to 64 bits
/// each rather than packed together, so two regions cannot collide by one's coordinates carrying into the other's.
fn key(seed: u64, origin_x: u32, origin_y: u32) -> [u8; 32] {
    let mut key = [0_u8; 32];

    key[0..8].copy_from_slice(&seed.to_le_bytes());
    key[8..16].copy_from_slice(&u64::from(origin_x).to_le_bytes());
    key[16..24].copy_from_slice(&u64::from(origin_y).to_le_bytes());

    key
}

/// One uniform draw in `[0, 1)`.
///
/// The top 53 bits of a `u64` scaled by `2^-53`, which is the standard construction and gives every value the `f64`
/// mantissa can represent in that range exactly once. Written out rather than taken from a distribution crate; see
/// [`gaussian`].
fn unit(rng: &mut ChaCha8Rng) -> f64 {
    ((rng.next_u64() >> 11) as f64) * (1.0 / ((1_u64 << 53) as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A field of `len` for the region at `(x, y)`, at the seed the pipeline uses.
    fn field(len: usize, x: u32, y: u32) -> Vec<f32> {
        let mut dest = vec![f32::NAN; len];
        gaussian(&mut dest, x, y, NOISE_SEED);

        dest
    }

    #[test]
    fn the_same_origin_gives_the_same_field_twice() {
        // The property the run cache's correctness rests on: a memoized result the user cannot reproduce is worse
        // than no cache at all.
        assert_eq!(field(4096, 960, 480), field(4096, 960, 480));
    }

    #[test]
    fn two_origins_give_different_fields() {
        let here = field(4096, 0, 0);

        assert_ne!(here, field(4096, 960, 0), "two columns of one row share a noise field");
        assert_ne!(here, field(4096, 0, 960), "two rows of one column share a noise field");
        // And the two axes are not interchangeable, which packing the origins into one word could make them.
        assert_ne!(field(4096, 960, 0), field(4096, 0, 960), "the two axes produced one field");
    }

    #[test]
    fn a_regions_field_does_not_depend_on_how_many_regions_preceded_it() {
        // The whole reason the key carries the origin rather than the field being drawn from one stream across the
        // image: changing the tile geometry must not reshuffle the noise of the regions that kept their position.
        let alone = field(4096, 1920, 960);

        let mut walked = Vec::new();
        for (x, y) in [(0, 0), (960, 0), (1920, 0), (0, 960), (960, 960), (1920, 960)] {
            walked = field(4096, x, y);
        }

        assert_eq!(walked, alone, "the last region of a walk differed from the same region drawn on its own");
    }

    #[test]
    fn the_field_is_standard_normal() {
        // Mean near 0 and variance near 1 over a large draw. The bounds are loose on purpose — this is a check that
        // the Box-Muller transform is the right way round and the uniforms are in `[0, 1)`, not a test of ChaCha8.
        let field = field(1 << 20, 0, 0);
        let count = field.len() as f64;

        let mean = field.iter().map(|value| f64::from(*value)).sum::<f64>() / count;
        let variance = field.iter().map(|value| (f64::from(*value) - mean).powi(2)).sum::<f64>() / count;

        assert!(mean.abs() < 0.01, "the field's mean is {mean}, not near 0");
        assert!((variance - 1.0).abs() < 0.01, "the field's variance is {variance}, not near 1");
    }

    #[test]
    fn every_element_is_written_and_finite() {
        // An odd length is the case the pairing has to handle: the second of the final pair has nowhere to go, and
        // the element before it must still be written. A `NaN` or an infinity anywhere here reaches the transformer's
        // projection and poisons the whole region rather than one value.
        for len in [1, 2, 3, 15, 4096, 4097] {
            let field = field(len, 12, 34);

            assert_eq!(field.len(), len);
            assert!(field.iter().all(|value| value.is_finite()), "a {len}-element field held a non-finite value");
        }
    }

    #[test]
    fn a_different_seed_gives_a_different_field() {
        // The seed is a constant today, so this is what would catch it being folded away as one — the origin alone
        // deciding the stream would pass every other test here.
        let mut other = vec![f32::NAN; 4096];
        gaussian(&mut other, 0, 0, NOISE_SEED ^ 1);

        assert_ne!(other, field(4096, 0, 0));
    }
}
