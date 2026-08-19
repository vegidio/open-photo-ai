package osaka

import (
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
)

// ditRegionEdge is the ONLY region size the diffusion transformer accepts, in output pixels.
//
// The graph is traced at one size and its shape arithmetic is folded against it, so nothing else runs. 960 is not an
// arbitrary choice: at 16 output pixels per token it gives a 60x60 grid, which is 3600 tokens - exactly the 45x80
// grid SeedVR2 was trained on. It is also the only size in range whose attention windows divide evenly. The window
// is derived by rescaling the grid to that same 3600-token reference, so at 960 the regular windows are 3x3 of
// 20x20 tokens and the shifted ones 4x4, with no remainder; at 512 the 32-token grid was cut into 30+2 and 15+15+2,
// leaving windows two tokens wide that saw almost nothing.
//
// Tiling remains the only way to run the model, not a memory strategy - the size is still not ours to choose.
const ditRegionEdge = 960

// tileOverlap is how much adjacent regions share, in output pixels. It is set by the decoder's edge influence and by
// how far shifted-window attention moves information, neither of which scales with the region, so it stays at 128
// where it was tuned - now an eighth of the region rather than a quarter, which is why a larger region wastes less.
const tileOverlap = 128

// alignment is the pixel multiple every dimension fed to the model must be: the VAE compresses 8x and the DiT
// patchifies 2x on top of that. ditRegionEdge is itself a multiple of it, so every tile stays aligned.
const alignment = 16

// Per-pixel and per-token activation coefficients.
//
// THESE ARE ANALYTIC ESTIMATES, NOT MEASUREMENTS, and they no longer choose anything - the region size is fixed by
// the graph. They survive to warn when a run looks likely to exhaust the pool, which is worth saying before an hour
// of work rather than after.
const (
	// ditBytesPerToken: within one block the live tensors are the residual stream (2 x 2560), the qkv projection
	// (3 x 2560), the attention output and its projection (2 x 2560) and the gated MLP intermediate (2 x 4 x 2560),
	// at 2 bytes each - about 77 KB per token. Attention adds its scores on top, but this export runs each window
	// separately rather than materializing one masked matrix over the whole sequence, so that term is bounded by the
	// window (20x20 tokens plus 58 of text) instead of growing with the region. The peak is over one block, not the
	// sum of all of them, because the runtime reuses buffers between nodes. 1.5x covers the planner's slack.
	ditBytesPerToken = 165 << 10

	// vaeBytesPerPixel: the decoder's final stage runs at full output resolution. A 128-channel feature map is
	// 256 B/px, a resblock keeps around three live plus normalization workspace, and the upsample briefly holds both
	// its input and output. The channel count is inferred from the decoder's 302 MB of fp16 weights.
	vaeBytesPerPixel = 1280
)

// EstimateActivationBytes predicts the peak activation footprint of one pass over a region of the given output size,
// excluding the weights, which the registry already accounts for.
//
// The two terms are added rather than maxed: the DiT's peak and the decoder's peak do not coincide, but the latents
// bridging them stay live across both, and adding is the conservative reading.
func EstimateActivationBytes(width, height int) int64 {
	if width <= 0 || height <= 0 {
		return 0
	}

	// DiT tokens: the VAE compresses 8x and the DiT patchifies 2x on top of that.
	tokens := int64(width/alignment) * int64(height/alignment)
	pixels := int64(width) * int64(height)

	return tokens*ditBytesPerToken + pixels*vaeBytesPerPixel
}

// RegionSize reports the region edge the pipeline must use, and whether the memory pool is expected to cope.
//
// It takes no decision about the size - the graph has already made that - but it is the one place that notices when a
// run is heading for a pool it will not fit in, which on the host pool matters: exhausting device memory returns an
// error that can be handled, while exhausting host memory takes the process down through ONNX Runtime's allocator
// with nothing to catch.
func RegionSize(pool types.MemoryPool, available int64) (edge int, ok bool) {
	need := int64(float64(EstimateActivationBytes(ditRegionEdge, ditRegionEdge)) * marginFor(pool))

	if available > 0 && need > available {
		internal.Log().Warn("the model may not fit in the available memory for this pool",
			"pool", pool, "available", available, "estimated_need", need, "region", ditRegionEdge)

		return ditRegionEdge, false
	}

	return ditRegionEdge, true
}

// marginFor is how much slack to demand over the estimate. The host pool gets more because its failure mode is worse:
// an allocation it cannot serve aborts the process rather than returning an error.
func marginFor(pool types.MemoryPool) float64 {
	if pool == types.MemoryPoolDevice {
		return 2.0
	}

	return 3.0
}

// Available reports the memory the registry believes is still free in the pool a given provider is charged to.
func Available(ep types.ExecutionProvider) (types.MemoryPool, int64) {
	pool := internal.PoolOf(ep)

	return pool, internal.Registry.Available(pool)
}

// alignUp rounds a dimension up to the model's required multiple.
func alignUp(v int) int {
	if r := v % alignment; r != 0 {
		return v + alignment - r
	}

	return v
}
