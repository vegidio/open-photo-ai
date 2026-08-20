package osaka

import (
	"encoding/binary"
	"math"
	"math/rand/v2"
)

// The DiT takes 33 channels, laid out as concat([noise, encoded_latent, ones_mask]) - noise first, then the latent of
// the image being restored, then a mask channel of ones - and driven at the first timestep of a 1000-step schedule.
//
// The convention is not discoverable from the graph: all 33 channels go through a single projection, so nothing in the
// structure says which group is which, and getting it wrong does not fail loudly - it just returns a worse image. The
// values below are taken from SeedVR2's reference implementation rather than guessed.
//
// Nothing is rescaled on the way in. SeedVR2's VAE declares a scaling factor of 0.9152, but the reference pipeline
// does not apply it - the encoder's output goes into the DiT untouched.
const (
	taskValue   float32 = 1
	ditTimestep float32 = 1000
)

const (
	latentChannels = 16
	ditChannels    = 2*latentChannels + 1 // 33
	vaeStride      = 8                    // the VAE's spatial compression
)

// packVidInput builds the DiT's 33-channel input from a condition latent and a noise latent.
//
// All three tensors are planar, so each group is a contiguous run of channel planes and packing is a set of copies
// rather than an interleaving.
func packVidInput(cond, noise []float32, plane int) []float32 {
	out := make([]float32, ditChannels*plane)
	noiseAt, condAt, taskAt := 0, latentChannels, 2*latentChannels

	for c := range latentChannels {
		copy(out[(noiseAt+c)*plane:], noise[c*plane:(c+1)*plane])
		copy(out[(condAt+c)*plane:], cond[c*plane:(c+1)*plane])
	}

	task := out[taskAt*plane : (taskAt+1)*plane]
	for i := range task {
		task[i] = taskValue
	}

	return out
}

// gaussianNoise generates the noise latent for one region.
//
// It is seeded from the region's position rather than from a global source so that a run is reproducible. The image
// cache memoizes results by input hash and operation, so a nondeterministic model would hand the user a result they
// could not reproduce and would make any A/B comparison of tiling settings meaningless. Deriving the seed from the
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
