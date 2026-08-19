package osaka

import (
	"math"
	"math/rand/v2"
	"testing"
)

// detailPattern is a plane of pure high-frequency content: alternating pixels, which no low-pass at any of the
// levels used here can follow.
func detailPattern(w, h int, seed uint64) []float32 {
	r := rand.New(rand.NewPCG(seed, 0xA5A5))
	data := make([]float32, 3*w*h)

	for i := range data {
		data[i] = r.Float32()*2 - 1
	}

	return data
}

func addTint(src []float32, w, h int, tint [3]float32) []float32 {
	out := make([]float32, len(src))
	copy(out, src)
	plane := w * h

	for c := range 3 {
		for i := range plane {
			out[c*plane+i] += tint[c]
		}
	}

	return out
}

func meanOf(data []float32, w, h, channel int) float64 {
	plane := w * h
	var sum float64

	for i := range plane {
		sum += float64(data[channel*plane+i])
	}

	return sum / float64(plane)
}

// A constant cast is pure low frequency, so the fix must remove essentially all of it.
func TestWaveletColorFixRemovesAGlobalTint(t *testing.T) {
	const w, h = 128, 96

	reference := detailPattern(w, h, 1)
	tinted := addTint(reference, w, h, [3]float32{0.30, -0.20, 0.10})

	before := [3]float64{
		meanOf(tinted, w, h, 0) - meanOf(reference, w, h, 0),
		meanOf(tinted, w, h, 1) - meanOf(reference, w, h, 1),
		meanOf(tinted, w, h, 2) - meanOf(reference, w, h, 2),
	}

	fixed := waveletColorFix(tinted, reference, w, h, defaultColorFixLevels)

	for c := range 3 {
		after := meanOf(fixed, w, h, c) - meanOf(reference, w, h, c)

		if math.Abs(after) > 0.02 {
			t.Fatalf("channel %d: cast %.4f -> %.4f, want ~0", c, before[c], after)
		}
	}
}

// The fix must not touch detail: it exists to correct colour, and a version that also pulled high frequencies towards
// the reference would be undoing the model's whole contribution.
func TestWaveletColorFixPreservesDetail(t *testing.T) {
	const w, h = 128, 96

	reference := detailPattern(w, h, 2)
	restored := detailPattern(w, h, 3) // completely different detail, same statistics

	original := make([]float32, len(restored))
	copy(original, restored)

	fixed := waveletColorFix(restored, reference, w, h, defaultColorFixLevels)

	// High-frequency energy is measured as the mean absolute difference between horizontal neighbours.
	energy := func(data []float32) float64 {
		var sum float64
		plane := w * h

		for c := range 3 {
			for y := range h {
				for x := 1; x < w; x++ {
					i := c*plane + y*w + x
					sum += math.Abs(float64(data[i] - data[i-1]))
				}
			}
		}

		return sum
	}

	got, want := energy(fixed), energy(original)

	if math.Abs(got-want)/want > 0.02 {
		t.Fatalf("detail energy changed by %.1f%%, want under 2%%", math.Abs(got-want)/want*100)
	}
}

// With nothing to correct the transform must be a no-op, which is the cheapest guard against a sign error or an
// off-by-one in the dilation.
func TestWaveletColorFixIsIdentityAgainstItself(t *testing.T) {
	const w, h = 64, 64

	data := detailPattern(w, h, 4)
	original := make([]float32, len(data))
	copy(original, data)

	fixed := waveletColorFix(data, original, w, h, defaultColorFixLevels)

	for i := range fixed {
		if d := math.Abs(float64(fixed[i] - original[i])); d > 1e-5 {
			t.Fatalf("index %d changed by %g", i, d)
		}
	}
}

// A smooth gradient is entirely low frequency, so correcting towards a flat reference must flatten it.
func TestWaveletColorFixCorrectsASmoothGradient(t *testing.T) {
	const w, h = 128, 128

	flat := make([]float32, 3*w*h)
	gradient := make([]float32, 3*w*h)

	for c := range 3 {
		for y := range h {
			for x := range w {
				gradient[c*w*h+y*w+x] = float32(x)/w - 0.5
			}
		}
	}

	fixed := waveletColorFix(gradient, flat, w, h, defaultColorFixLevels)

	// The ramp spans 0.5 either side of zero. Five levels reach a support of about 62 pixels, so on a 128-wide image
	// the border columns are the one place the low-pass cannot fully see the ramp - it is clamped there. Measure the
	// reduction rather than an absolute floor, and check the interior separately, where the filter has full support.
	var worst, worstInterior float64
	plane := w * h

	for c := range 3 {
		for y := range h {
			for x := range w {
				d := math.Abs(float64(fixed[c*plane+y*w+x]))

				if d > worst {
					worst = d
				}
				if x >= 62 && x < w-62 && d > worstInterior {
					worstInterior = d
				}
			}
		}
	}

	if worst > 0.1 {
		t.Fatalf("gradient reduced only from 0.5 to %.4f, want at least an 80%% reduction", worst)
	}
	if worstInterior > 0.02 {
		t.Fatalf("interior residual %.4f, want ~0 where the filter has full support", worstInterior)
	}
}
