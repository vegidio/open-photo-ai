package osaka

import (
	"encoding/binary"
	"math"
	"math/rand/v2"
)

// The DiT takes 33 channels: a noise latent, the latent of the image being restored, and a mask channel. The
// convention is not discoverable from the graph - all 33 channels go through a single projection, so nothing in the
// structure says which group is which - and getting it wrong does not fail loudly, it just returns a worse image.
//
// The values are taken from SeedVR2's reference implementation, which builds the input as
// concat([noise, encoded_latent, ones_mask]) and drives it at t=1000. ditConfig remains a struct rather than four
// constants only so the sweep in sweep_manual_test.go can still enumerate alternatives if a future export changes
// the convention.
type ditConfig struct {
	layout    channelLayout
	taskValue float32
	timestep  float32

	// latentScale multiplies the condition latent on the way into the DiT. SeedVR2's VAE declares a scaling factor
	// of 0.9152, but the reference pipeline does not apply it - the encoder's output goes in untouched - so this is
	// 1 in practice and exists only to make that explicit rather than implicit.
	latentScale float32
}

// channelLayout is the order of the three groups within vid_input's 33 channels.
type channelLayout int

const (
	layoutNoiseCondTask channelLayout = iota
	layoutCondNoiseTask
	layoutTaskNoiseCond
	layoutTaskCondNoise
	layoutNoiseTaskCond
	layoutCondTaskNoise
)

func (l channelLayout) String() string {
	switch l {
	case layoutNoiseCondTask:
		return "noise|cond|task"
	case layoutCondNoiseTask:
		return "cond|noise|task"
	case layoutTaskNoiseCond:
		return "task|noise|cond"
	case layoutTaskCondNoise:
		return "task|cond|noise"
	case layoutNoiseTaskCond:
		return "noise|task|cond"
	case layoutCondTaskNoise:
		return "cond|task|noise"
	default:
		return "unknown"
	}
}

// offsets returns the starting channel of the noise, condition and task groups for this layout.
func (l channelLayout) offsets() (noise, cond, task int) {
	switch l {
	case layoutNoiseCondTask:
		return 0, latentChannels, 2 * latentChannels
	case layoutCondNoiseTask:
		return latentChannels, 0, 2 * latentChannels
	case layoutTaskNoiseCond:
		return 1, 1 + latentChannels, 0
	case layoutTaskCondNoise:
		return 1 + latentChannels, 1, 0
	case layoutNoiseTaskCond:
		return 0, 1 + latentChannels, latentChannels
	case layoutCondTaskNoise:
		return 1 + latentChannels, 0, latentChannels
	default:
		return 0, latentChannels, 2 * latentChannels
	}
}

const (
	latentChannels = 16
	ditChannels    = 2*latentChannels + 1 // 33
	vaeStride      = 8                    // the VAE's spatial compression
)

// packVidInput builds the DiT's 33-channel input from a condition latent and a noise latent.
//
// All three tensors are planar, so each group is a contiguous run of channel planes and packing is a set of copies
// rather than an interleave.
func packVidInput(cond, noise []float32, plane int, cfg ditConfig) []float32 {
	out := make([]float32, ditChannels*plane)
	noiseAt, condAt, taskAt := cfg.layout.offsets()

	scale := cfg.latentScale
	if scale == 0 {
		scale = 1
	}

	for c := range latentChannels {
		copy(out[(noiseAt+c)*plane:], noise[c*plane:(c+1)*plane])

		dst := out[(condAt+c)*plane : (condAt+c+1)*plane]
		src := cond[c*plane : (c+1)*plane]

		for i := range dst {
			dst[i] = src[i] * scale
		}
	}

	task := out[taskAt*plane : (taskAt+1)*plane]
	for i := range task {
		task[i] = cfg.taskValue
	}

	return out
}

// gaussianNoise generates the noise latent for one region.
//
// It is seeded from the region's position rather than from a global source so that a run is reproducible. The image
// cache memoizes results by input hash and operation, so a nondeterministic model would hand the user a result they
// could not reproduce, and would make any A/B comparison of tiling settings meaningless. Deriving the seed from the
// origin also means a tile's noise does not depend on how many tiles preceded it, so changing the tile size does not
// reshuffle the noise of the tiles that kept their position.
func gaussianNoise(n, originX, originY int, seed uint64) []float32 {
	var key [32]byte
	binary.LittleEndian.PutUint64(key[0:], seed)
	binary.LittleEndian.PutUint64(key[8:], uint64(int64(originX)))
	binary.LittleEndian.PutUint64(key[16:], uint64(int64(originY)))

	r := rand.New(rand.NewChaCha8(key))
	out := make([]float32, n)

	// Box-Muller, two samples at a time.
	for i := 0; i < n; i += 2 {
		u1 := r.Float64()
		if u1 < 1e-12 {
			u1 = 1e-12
		}

		radius := math.Sqrt(-2 * math.Log(u1))
		angle := 2 * math.Pi * r.Float64()

		out[i] = float32(radius * math.Cos(angle))
		if i+1 < n {
			out[i+1] = float32(radius * math.Sin(angle))
		}
	}

	return out
}

// cropCHW extracts a rectangular region from planar CHW data.
func cropCHW(src []float32, width, height, x0, y0, w, h, channels int) []float32 {
	out := make([]float32, channels*w*h)
	srcPlane, dstPlane := width*height, w*h

	for c := range channels {
		for y := range h {
			copy(
				out[c*dstPlane+y*w:c*dstPlane+(y+1)*w],
				src[c*srcPlane+(y0+y)*width+x0:c*srcPlane+(y0+y)*width+x0+w],
			)
		}
	}

	return out
}
