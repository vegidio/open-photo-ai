package osaka

import (
	"math"
	"testing"
)

// The three groups must occupy disjoint channel ranges that exactly fill the 33, and each must carry the values the
// caller handed over. Getting this wrong is silent - all 33 channels go through one projection, so a misplaced group
// degrades the image rather than failing - which is why it is pinned here.
func TestPackVidInputPlacesEachGroup(t *testing.T) {
	const plane = 4

	cond := make([]float32, latentChannels*plane)
	noise := make([]float32, latentChannels*plane)

	for c := range latentChannels {
		for i := range plane {
			cond[c*plane+i] = float32(100 + c)
			noise[c*plane+i] = float32(-(100 + c))
		}
	}

	got := packVidInput(cond, noise, plane)
	if len(got) != ditChannels*plane {
		t.Fatalf("got %d values, want %d", len(got), ditChannels*plane)
	}

	noiseAt, condAt, taskAt := 0, latentChannels, 2*latentChannels

	seen := make([]int, ditChannels)
	for c := range latentChannels {
		seen[noiseAt+c]++
		seen[condAt+c]++

		if v := got[(condAt+c)*plane]; v != float32(100+c) {
			t.Fatalf("condition channel %d landed at %f", c, v)
		}
		if v := got[(noiseAt+c)*plane]; v != float32(-(100 + c)) {
			t.Fatalf("noise channel %d landed at %f", c, v)
		}
	}
	seen[taskAt]++

	for c, n := range seen {
		if n != 1 {
			t.Fatalf("channel %d claimed %d times", c, n)
		}
	}

	for i := range plane {
		if v := got[taskAt*plane+i]; v != taskValue {
			t.Fatalf("task channel value %f, want %f", v, taskValue)
		}
	}
}

// The reference pipeline feeds the encoder's output into the DiT untouched, despite the VAE declaring a 0.9152
// scaling factor. Applying it is a plausible-looking change that quietly degrades the output, so the absence of any
// rescaling is asserted rather than left implicit.
func TestPackVidInputDoesNotRescaleTheCondition(t *testing.T) {
	const plane = 2

	cond := make([]float32, latentChannels*plane)
	noise := make([]float32, latentChannels*plane)

	for i := range cond {
		cond[i] = 2
	}

	got := packVidInput(cond, noise, plane)
	if v := got[latentChannels*plane]; v != 2 {
		t.Fatalf("the condition latent was rescaled: %f", v)
	}
}

// The noise must be reproducible for a given position, and different between positions - otherwise every tile would
// receive an identical noise field, which correlates their artefacts.
func TestGaussianNoiseIsDeterministicPerPosition(t *testing.T) {
	a := gaussianNoise(4096, 0, 0, 7)
	b := gaussianNoise(4096, 0, 0, 7)

	for i := range a {
		if a[i] != b[i] {
			t.Fatalf("same position produced different noise at %d", i)
		}
	}

	c := gaussianNoise(4096, 128, 0, 7)
	same := 0

	for i := range a {
		if a[i] == c[i] {
			same++
		}
	}

	if same > len(a)/100 {
		t.Fatalf("different positions produced %d/%d identical samples", same, len(a))
	}
}

// Box-Muller must actually produce a standard normal: the model was trained against N(0,1).
func TestGaussianNoiseIsStandardNormal(t *testing.T) {
	const n = 1 << 18

	v := gaussianNoise(n, 3, 5, 11)

	var sum, sumSq float64
	for _, x := range v {
		sum += float64(x)
		sumSq += float64(x) * float64(x)
	}

	mean := sum / n
	variance := sumSq/n - mean*mean

	if math.Abs(mean) > 0.02 {
		t.Fatalf("mean %.4f, want ~0", mean)
	}
	if math.Abs(variance-1) > 0.02 {
		t.Fatalf("variance %.4f, want ~1", variance)
	}

	// An odd count must still be filled: the loop writes two samples per turn.
	if odd := gaussianNoise(7, 0, 0, 1); odd[6] == 0 {
		t.Fatal("the final sample of an odd-length buffer was not written")
	}
}

func TestCropCHW(t *testing.T) {
	const w, h, c = 8, 6, 3

	src := make([]float32, c*w*h)
	for ch := range c {
		for y := range h {
			for x := range w {
				src[ch*w*h+y*w+x] = float32(ch*1000 + y*10 + x)
			}
		}
	}

	got := cropCHW(src, w, h, 2, 1, 4, 3, c)

	for ch := range c {
		for y := range 3 {
			for x := range 4 {
				want := float32(ch*1000 + (y+1)*10 + (x + 2))

				if v := got[ch*12+y*4+x]; v != want {
					t.Fatalf("channel %d (%d,%d) = %f, want %f", ch, x, y, v, want)
				}
			}
		}
	}
}
