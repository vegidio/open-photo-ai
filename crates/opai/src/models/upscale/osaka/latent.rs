//! The geometry and the latent contract: what size the model is driven at, how many channels it takes, how they are
//! packed, and what its output means.
//!
//! **None of this is discoverable from the graphs**, which is why it is written down here rather than read off them.
//! All 33 of the transformer's input channels go through a single projection, so nothing in its structure says which
//! group is which; the values below are SeedVR2's reference implementation's conventions, and getting one wrong does
//! not fail — it returns a worse image.
//!
//! What the published FP16 exports *do* declare, and what this module is checked against:
//!
//! ```text
//!   encoder      pixel_image      [1, 3,960,960] f32  ->  latent          [1,16,120,120] f32
//!   transformer  vid_input        [1,33,120,120] f32  ->  denoised_latent [1,16,120,120] f32
//!   decoder      latent           [1,16,120,120] f32  ->  pixel_image     [1, 3,960,960] f32
//! ```
//!
//! Two things that follow from those shapes rather than from the reference's word: the VAE's stride is 8 in both
//! directions, and the transformer takes **one** input with no timestep beside it. The timestep is folded into the
//! graph as a constant because this is a one-step model — which is the whole reason [`scheduler_step`] collapses to a
//! subtraction, so the two would have to change together to run this model at more than one step.
//!
//! **Nothing is rescaled on the way in.** SeedVR2's VAE declares a scaling factor of 0.9152 and its reference
//! pipeline does not apply it, so the encoder's output goes into the transformer untouched. Stated because its
//! absence looks like an omission and is not.

use crate::models::Scale;

/// How many channels the VAE compresses an image into.
pub(crate) const LATENT_CHANNELS: usize = 16;

/// How many channels the diffusion transformer takes: a noise latent, a condition latent and one mask plane.
pub(crate) const TRANSFORMER_CHANNELS: usize = 2 * LATENT_CHANNELS + 1;

/// The VAE's spatial compression, in both directions. Confirmed by the exports above: 960 pixels become 120 latents.
pub(crate) const VAE_STRIDE: usize = 8;

/// The one region size the diffusion transformer accepts, in pixels.
///
/// Not a tile size this project chose. The published graphs are frozen at this geometry with no symbolic dimensions
/// on any axis, so a region of any other size is rejected by the runtime rather than run slowly.
pub(crate) const REGION_EDGE: usize = 960;

/// [`REGION_EDGE`] as the geometry wants it.
///
/// The `usize` form above is what the latent arithmetic indexes with; every caller that places a region needs a
/// `u32`, and each one was converting it back with its own `try_from` and its own expect message. Declared once as a
/// constant, which also makes it a compile-time fact rather than thirteen runtime checks of the same literal.
pub(crate) const REGION_EDGE_PX: u32 = REGION_EDGE as u32;

/// How much adjacent regions are asked to share, in pixels.
///
/// The reference's figure, and it is **not** derived from [`REGION_EDGE`]: it is set by the decoder's edge influence
/// and by how far shifted-window attention moves information, neither of which scales with the region. So it stays at
/// the 128 it was tuned at whatever the region size is — an eighth of a 960-pixel region rather than the quarter it
/// was of the smaller one, which is why the larger region wastes less.
///
/// What two regions *actually* share is a different number again, and larger down the last column and row — see
/// [`Layout::overlaps_x`](imaging::grid::Layout::overlaps_x), which is what the accumulator feathers over.
pub(crate) const REGION_OVERLAP: u32 = 128;

/// The pixel multiple every dimension handed to the model must be.
///
/// The reference's figure and the product of two the exports already declare: the VAE compresses [`VAE_STRIDE`] and
/// the transformer patchifies 2x on top of that. [`REGION_EDGE`] is itself a multiple of it, so no tile the grid
/// places can be misaligned.
pub(crate) const ALIGNMENT: u32 = 16;

/// The size the image is resampled to before anything runs: the requested scale applied to `width` and `height` and
/// rounded.
///
/// **The same arithmetic `correct_overshoot` performs**, deliberately, so that which contract served a request is not
/// observable in the size of what comes back — which is why both go through [`Scale::applied_to`] rather than each
/// spelling the rounding out.
///
/// This is the whole of what a resolution-preserving model does about scale: the network does not change the token
/// grid, so there is no factor it could apply and the resample is how the scale is reached. At a scale of 1 the
/// target is the source's own dimensions and the run is a restoration rather than a no-op.
pub(crate) fn target_size(width: u32, height: u32, scale: Scale) -> (u32, u32) {
    scale.applied_to(width, height)
}

/// The extent the model is actually driven over: `width` and `height` raised to a multiple of [`ALIGNMENT`] and to at
/// least one [`REGION_EDGE`] on each axis.
///
/// Two constraints, both the model's rather than this pipeline's. Everything the graphs accept is a multiple of 16,
/// and the transformer accepts exactly one region size — so an image smaller than a region is padded up to one rather
/// than run at its own size, and a full region's work is spent on it. The difference between this and the target is
/// mirrored out of the image's own edge pixels and trimmed off the result.
pub(crate) fn padded_extent(width: u32, height: u32) -> (u32, u32) {
    let axis = |length: u32| REGION_EDGE_PX.max(length.div_ceil(ALIGNMENT) * ALIGNMENT);

    (axis(width), axis(height))
}

/// What the mask plane carries. One everywhere: the whole region is being restored, and the plane exists so the
/// transformer's projection sees a constant it can key the task off.
const MASK_VALUE: f32 = 1.0;

/// Packs the transformer's 33-channel input from a condition latent and a noise latent.
///
/// The layout is `[noise | condition | ones]` — **noise first**, then the latent of the image being restored, then a
/// mask plane of ones. Taken from SeedVR2's reference implementation rather than inferred: all 33 channels go through
/// one projection, so the graph's structure says nothing about which group is which, and a wrong order returns a
/// worse image rather than an error. Task 5.2 is what holds this order to being the right one, by showing that
/// swapping the two latent groups scores worse against the input.
///
/// All three tensors are planar, so each group is a contiguous run of channel planes and packing is a set of copies
/// rather than an interleaving.
///
/// # Panics
///
/// Panics when `condition` or `noise` is not [`LATENT_CHANNELS`] planes of `plane`, or `dest` not
/// [`TRANSFORMER_CHANNELS`] of them. Every one of those is a caller that allocated its scratch from a different
/// geometry than the one it is packing at, which is a mistake in this module's own driver rather than anything a
/// user or a model can cause.
pub(crate) fn pack(dest: &mut [f32], condition: &[f32], noise: &[f32], plane: usize) {
    assert_eq!(dest.len(), TRANSFORMER_CHANNELS * plane, "the packed buffer is not 33 planes");
    assert_eq!(condition.len(), LATENT_CHANNELS * plane, "the condition latent is not 16 planes");
    assert_eq!(noise.len(), LATENT_CHANNELS * plane, "the noise latent is not 16 planes");

    let (noise_at, condition_at, mask_at) = (0, LATENT_CHANNELS * plane, 2 * LATENT_CHANNELS * plane);

    dest[noise_at..noise_at + noise.len()].copy_from_slice(noise);
    dest[condition_at..condition_at + condition.len()].copy_from_slice(condition);
    dest[mask_at..].fill(MASK_VALUE);
}

/// Turns the transformer's prediction into the clean latent the decoder is given.
///
/// **The graph's output is named `denoised_latent` and is not the latent.** It is the flow-matching velocity; the
/// reference pipeline calls the same tensor "noise" and hands it to a scheduler. For a single step the schedule runs
/// from `t=1000` to `t=0`, so the normalized time is 1 and the whole update collapses to
///
/// ```text
///   x0 = sample - t_norm * prediction = noise - prediction
/// ```
///
/// Skipping it is not a subtle error, but it is a silent one: the decode returns the image buried under the velocity
/// field, which scores worse than the input it was given while still being an image. That is why task 5.2 checks it
/// by differential test rather than by inspection.
///
/// # Panics
///
/// Panics when the three buffers are not the same length, which is a caller mixing two geometries.
pub(crate) fn scheduler_step(dest: &mut [f32], prediction: &[f32], noise: &[f32]) {
    assert_eq!(dest.len(), prediction.len(), "the destination and the prediction disagree");
    assert_eq!(noise.len(), prediction.len(), "the noise and the prediction disagree");

    for (out, (noise, prediction)) in dest.iter_mut().zip(noise.iter().zip(prediction)) {
        *out = noise - prediction;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The latent geometry one 960x960 region runs at, which is what every buffer here is shaped from.
    const PLANE: usize = (REGION_EDGE / VAE_STRIDE) * (REGION_EDGE / VAE_STRIDE);

    #[test]
    fn the_constants_are_the_ones_the_published_graphs_declare() {
        // Read off the FP16 exports rather than taken from the reference: the encoder is `[1,3,960,960]` in and
        // `[1,16,120,120]` out, the transformer `[1,33,120,120]` in and `[1,16,120,120]` out.
        assert_eq!(LATENT_CHANNELS, 16);
        assert_eq!(TRANSFORMER_CHANNELS, 33, "two latents and a mask plane");
        assert_eq!(REGION_EDGE / VAE_STRIDE, 120, "960 pixels did not compress to the 120 latents the graph takes");
        assert_eq!(PLANE, 120 * 120);
    }

    /// A scale, which every geometry test below states as the number a user asked for.
    fn scale(factor: f64) -> Scale {
        Scale::new(factor).unwrap_or_else(|error| panic!("{factor} is in range: {error}"))
    }

    #[test]
    fn a_scale_of_one_leaves_the_target_at_the_sources_own_dimensions() {
        // The restoration request: the model still runs, at the size the image already has. Checked on an odd
        // dimension too, where a rounding that was not exact would move it by a pixel.
        assert_eq!(target_size(1920, 1080, scale(1.0)), (1920, 1080));
        assert_eq!(target_size(301, 201, scale(1.0)), (301, 201));
        assert_eq!(target_size(1, 1, scale(1.0)), (1, 1));
    }

    #[test]
    fn the_target_is_the_requested_scale_applied_to_each_axis_and_rounded() {
        assert_eq!(target_size(300, 200, scale(2.0)), (600, 400));
        assert_eq!(target_size(300, 200, scale(4.0)), (1200, 800));
        assert_eq!(target_size(300, 200, scale(1.5)), (450, 300));
    }

    #[test]
    fn the_rounding_is_the_one_the_convolutional_correction_performs() {
        // 301 * 1.5 is 451.5 and 201 * 1.5 is 301.5 — the half a truncation gets wrong by a pixel on every odd
        // dimension. `correct_overshoot` rounds, so which contract served a request must not be observable in the
        // size of what comes back; this is that equality, driven through the other contract's own arithmetic.
        for (width, height, factor) in
            [(301, 201, 1.5), (399, 299, 2.5), (1023, 767, 3.3), (17, 13, 7.7), (301, 201, 1.0)]
        {
            let corrected = ((f64::from(width) * factor).round() as u32, (f64::from(height) * factor).round() as u32);

            assert_eq!(
                target_size(width, height, scale(factor)),
                corrected,
                "{width}x{height} at {factor}x disagrees with the correcting resample's arithmetic"
            );
        }
    }

    #[test]
    fn an_axis_already_aligned_and_at_least_one_region_is_left_alone() {
        assert_eq!(
            padded_extent(REGION_EDGE_PX, REGION_EDGE_PX),
            (REGION_EDGE_PX, REGION_EDGE_PX),
            "one whole region was padded"
        );
        assert_eq!(padded_extent(1920, 1088), (1920, 1088));
        // And every multiple of the alignment above one region, so the identity is a property rather than two cases.
        for steps in 60..80u32 {
            let length = steps * ALIGNMENT;
            assert_eq!(padded_extent(length, length), (length, length), "{length} is already aligned");
        }
    }

    #[test]
    fn an_axis_that_is_not_a_multiple_of_the_alignment_is_raised_to_the_next_one() {
        assert_eq!(padded_extent(1000, 1001), (1008, 1008));
        assert_eq!(padded_extent(1919, 1081), (1920, 1088));

        for length in 961..1100u32 {
            let (padded, _) = padded_extent(length, length);
            assert!(padded >= length, "{length} was cropped rather than extended");
            assert!(padded - length < ALIGNMENT, "{length} was raised past the next multiple, to {padded}");
            assert_eq!(padded % ALIGNMENT, 0, "{length} was raised to {padded}, which is not aligned");
        }
    }

    #[test]
    fn an_axis_below_one_region_is_raised_to_exactly_one() {
        // The transformer accepts one region size, so a small image is padded up to it and costs a full region. It
        // is raised to exactly one rather than beyond it — the work is inherent, and more of it is not.
        for length in [1, 16, 200, 640, REGION_EDGE_PX - 1] {
            assert_eq!(
                padded_extent(length, length),
                (REGION_EDGE_PX, REGION_EDGE_PX),
                "{length} was not raised to one region"
            );
        }

        // Each axis answers for itself: a wide, short image is padded on height alone.
        assert_eq!(padded_extent(1920, 200), (1920, REGION_EDGE_PX));
        assert_eq!(padded_extent(200, 1920), (REGION_EDGE_PX, 1920));
    }

    #[test]
    fn the_two_figures_are_the_reference_implementations_and_the_region_is_a_multiple_of_the_alignment() {
        // Pinned rather than derived: neither scales with the region. The overlap is set by the decoder's edge
        // influence and by how far shifted-window attention moves information; the alignment is the VAE's 8x and the
        // transformer's 2x patchify on top of it.
        assert_eq!(REGION_OVERLAP, 128);
        assert_eq!(ALIGNMENT, 16);
        assert_eq!(
            ALIGNMENT as usize,
            2 * VAE_STRIDE,
            "the alignment stopped being the VAE stride and the patchify"
        );
        assert_eq!(REGION_EDGE % (ALIGNMENT as usize), 0, "a region is not itself aligned, so no tile could be");
    }

    #[test]
    fn the_packed_buffer_holds_its_three_groups_at_the_offsets_the_transformer_reads_them_from() {
        // Each group filled with a value of its own, so a swapped pair is visible as a value in the wrong third
        // rather than only as a worse image — which is what a wrong order actually costs at run time.
        let condition = vec![2.0_f32; LATENT_CHANNELS * PLANE];
        let noise = vec![-3.0_f32; LATENT_CHANNELS * PLANE];
        let mut packed = vec![f32::NAN; TRANSFORMER_CHANNELS * PLANE];

        pack(&mut packed, &condition, &noise, PLANE);

        let (noise_group, rest) = packed.split_at(LATENT_CHANNELS * PLANE);
        let (condition_group, mask) = rest.split_at(LATENT_CHANNELS * PLANE);

        assert!(noise_group.iter().all(|value| *value == -3.0), "the noise is not the first sixteen planes");
        assert!(
            condition_group.iter().all(|value| *value == 2.0),
            "the condition is not the second sixteen planes"
        );
        assert_eq!(mask.len(), PLANE, "the mask is not exactly one plane");
        assert!(mask.iter().all(|value| *value == 1.0), "the mask plane is not all ones");
    }

    #[test]
    fn packing_preserves_each_planes_contents_rather_than_interleaving_them() {
        // The groups are contiguous runs of planes, so the copy has to be plane-for-plane: an interleaving would pass
        // the test above, every value still being in the right third, and produce nonsense.
        let condition: Vec<f32> = (0..LATENT_CHANNELS * PLANE).map(|index| index as f32).collect();
        let noise: Vec<f32> = (0..LATENT_CHANNELS * PLANE).map(|index| -(index as f32)).collect();
        let mut packed = vec![f32::NAN; TRANSFORMER_CHANNELS * PLANE];

        pack(&mut packed, &condition, &noise, PLANE);

        assert_eq!(&packed[..noise.len()], noise.as_slice());
        assert_eq!(&packed[noise.len()..noise.len() + condition.len()], condition.as_slice());
    }

    #[test]
    fn the_packed_buffer_is_fully_written_whatever_it_held_before() {
        // It is scratch reused across regions, so anything left unwritten is the previous region's numbers fed to the
        // transformer as this region's.
        let mut packed = vec![f32::NAN; TRANSFORMER_CHANNELS * PLANE];

        pack(&mut packed, &vec![0.0; LATENT_CHANNELS * PLANE], &vec![0.0; LATENT_CHANNELS * PLANE], PLANE);

        assert!(packed.iter().all(|value| value.is_finite()), "the packed buffer was left partly unwritten");
    }

    #[test]
    fn the_scheduler_step_subtracts_the_prediction_from_the_noise() {
        // The one-step collapse, over a known pair. Written this way round deliberately: `prediction - noise` is the
        // plausible transposition, and it is the sign error that returns the velocity field as an image.
        let noise = [1.0_f32, 0.0, -2.5, 4.0];
        let prediction = [0.25_f32, -1.0, 0.5, 4.0];
        let mut denoised = [f32::NAN; 4];

        scheduler_step(&mut denoised, &prediction, &noise);

        assert_eq!(denoised, [0.75, 1.0, -3.0, 0.0]);
    }

    #[test]
    fn skipping_the_scheduler_step_is_not_the_same_as_taking_it() {
        // What task 5.2 checks against the real model, asserted here as the arithmetic it rests on: the prediction is
        // not the latent, so handing the decoder the raw output is a different buffer rather than a shortcut.
        let noise: Vec<f32> = (0..64).map(|index| (index as f32) * 0.1).collect();
        let prediction: Vec<f32> = (0..64).map(|index| (index as f32) * 0.3 - 1.0).collect();
        let mut denoised = vec![f32::NAN; 64];

        scheduler_step(&mut denoised, &prediction, &noise);

        assert_ne!(denoised, prediction, "the scheduler step returned the prediction unchanged");
    }
}
