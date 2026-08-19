package osaka

import (
	"math"
	"testing"
)

// allLayouts is every permutation of the three groups. Only layoutNoiseCondTask is used in production; the rest
// exist so the packing below is tested as a general function of its layout rather than in the one arrangement that
// happens to be correct.
var allLayouts = []channelLayout{
	layoutNoiseCondTask,
	layoutCondNoiseTask,
	layoutTaskNoiseCond,
	layoutTaskCondNoise,
	layoutNoiseTaskCond,
	layoutCondTaskNoise,
}

// Every layout must place the three groups in disjoint channel ranges that exactly fill the 33.
func TestLayoutOffsetsPartitionTheChannels(t *testing.T) {
	for _, layout := range allLayouts {
		t.Run(layout.String(), func(t *testing.T) {
			noise, cond, task := layout.offsets()

			seen := make([]int, ditChannels)
			for c := range latentChannels {
				seen[noise+c]++
				seen[cond+c]++
			}
			seen[task]++

			for c, n := range seen {
				if n != 1 {
					t.Fatalf("channel %d claimed %d times by %s", c, n, layout)
				}
			}
		})
	}
}

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

	for _, layout := range allLayouts {
		t.Run(layout.String(), func(t *testing.T) {
			cfg := ditConfig{layout: layout, taskValue: 0.5, latentScale: 1}
			got := packVidInput(cond, noise, plane, cfg)

			if len(got) != ditChannels*plane {
				t.Fatalf("got %d values, want %d", len(got), ditChannels*plane)
			}

			noiseAt, condAt, taskAt := layout.offsets()

			for c := range latentChannels {
				if v := got[(condAt+c)*plane]; v != float32(100+c) {
					t.Fatalf("condition channel %d landed at %f", c, v)
				}
				if v := got[(noiseAt+c)*plane]; v != float32(-(100 + c)) {
					t.Fatalf("noise channel %d landed at %f", c, v)
				}
			}

			for i := range plane {
				if v := got[taskAt*plane+i]; v != 0.5 {
					t.Fatalf("task channel value %f", v)
				}
			}
		})
	}
}

func TestPackVidInputAppliesTheLatentScale(t *testing.T) {
	const plane = 2

	cond := make([]float32, latentChannels*plane)
	noise := make([]float32, latentChannels*plane)

	for i := range cond {
		cond[i] = 2
	}

	scaled := packVidInput(cond, noise, plane, ditConfig{taskValue: 1, latentScale: 0.9152})
	_, condAt, _ := layoutNoiseCondTask.offsets()

	if got := scaled[condAt*plane]; math.Abs(float64(got)-2*0.9152) > 1e-6 {
		t.Fatalf("latent scale not applied: %f", got)
	}

	// A zero scale is a caller that never set the field; it must mean "unscaled", not "zero everything out".
	unset := packVidInput(cond, noise, plane, ditConfig{taskValue: 1})
	if got := unset[condAt*plane]; got != 2 {
		t.Fatalf("an unset latent scale changed the condition: %f", got)
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
