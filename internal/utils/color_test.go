package utils

import (
	"math"
	"testing"
)

// TestRgbToLabReference pins RgbToLab against OpenCV's float32 Lab values (D65). These catch the classic porting bug
// of implementing the 8-bit Lab encoding (L scaled by 255/100, a/b offset by 128) instead of the float convention.
func TestRgbToLabReference(t *testing.T) {
	cases := []struct {
		name       string
		r, g, b    float32
		l, a, lb32 float64
	}{
		{"black", 0, 0, 0, 0, 0, 0},
		{"white", 1, 1, 1, 100, 0, 0},
		{"red", 1, 0, 0, 53.241, 80.092, 67.203},
		{"green", 0, 1, 0, 87.735, -86.183, 83.179},
		{"blue", 0, 0, 1, 32.297, 79.188, -107.860},
		{"mid-gray", 0.5, 0.5, 0.5, 53.389, 0, 0},
	}

	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			l, a, b := RgbToLab(c.r, c.g, c.b)
			if math.Abs(float64(l)-c.l) > 0.05 || math.Abs(float64(a)-c.a) > 0.05 || math.Abs(float64(b)-c.lb32) > 0.05 {
				t.Errorf("RgbToLab(%v, %v, %v) = (%.3f, %.3f, %.3f), want (%.3f, %.3f, %.3f)",
					c.r, c.g, c.b, l, a, b, c.l, c.a, c.lb32)
			}
		})
	}
}

// TestLabRgbRoundTrip converts a grid of sRGB colors to Lab and back, requiring near-identity.
func TestLabRgbRoundTrip(t *testing.T) {
	const steps = 16
	for ri := 0; ri <= steps; ri++ {
		for gi := 0; gi <= steps; gi++ {
			for bi := 0; bi <= steps; bi++ {
				r := float32(ri) / steps
				g := float32(gi) / steps
				b := float32(bi) / steps

				l, la, lb := RgbToLab(r, g, b)
				rr, gg, bb := LabToRgb(l, la, lb)

				if math.Abs(float64(rr-r)) > 1e-3 || math.Abs(float64(gg-g)) > 1e-3 || math.Abs(float64(bb-b)) > 1e-3 {
					t.Fatalf("round trip (%v, %v, %v) -> (%v, %v, %v)", r, g, b, rr, gg, bb)
				}
			}
		}
	}
}

// TestLabToRgbClamps verifies out-of-gamut Lab values (a colorization model can emit any ab pair) come back clamped
// to the [0, 1] sRGB range instead of overflowing.
func TestLabToRgbClamps(t *testing.T) {
	for _, c := range [][3]float32{{50, 200, -200}, {100, 128, 128}, {0, -128, 127}, {120, 0, 0}, {-10, 0, 0}} {
		r, g, b := LabToRgb(c[0], c[1], c[2])
		for _, v := range []float32{r, g, b} {
			if v < 0 || v > 1 || math.IsNaN(float64(v)) {
				t.Errorf("LabToRgb(%v) = (%v, %v, %v): channel out of [0, 1]", c, r, g, b)
			}
		}
	}
}

// TestGrayHasZeroChroma verifies every neutral input maps to a/b == 0, which is what the colorization pipeline relies
// on when it rebuilds the model input as (L, 0, 0).
func TestGrayHasZeroChroma(t *testing.T) {
	for i := 0; i <= 255; i++ {
		v := float32(i) / 255
		_, a, b := RgbToLab(v, v, v)
		if math.Abs(float64(a)) > 0.01 || math.Abs(float64(b)) > 0.01 {
			t.Fatalf("gray %v has chroma (%v, %v)", v, a, b)
		}
	}
}

// TestRgbToLabLMatchesRgbToLab pins the L-only fast path against the full conversion. Colorization uses it at full
// photo resolution, so any divergence would be a silent, image-wide luminance shift.
func TestRgbToLabLMatchesRgbToLab(t *testing.T) {
	for r := 0; r < 256; r += 3 {
		for g := 0; g < 256; g += 5 {
			for b := 0; b < 256; b += 7 {
				fr := float32(uint32(r)*257) / 65535.0
				fg := float32(uint32(g)*257) / 65535.0
				fb := float32(uint32(b)*257) / 65535.0

				want, _, _ := RgbToLab(fr, fg, fb)

				if got := RgbToLabL(fr, fg, fb); got != want {
					t.Fatalf("RgbToLabL(%d,%d,%d) = %v, want %v", r, g, b, got, want)
				}

				if got := RgbToLabLBytes(uint8(r), uint8(g), uint8(b)); got != want {
					t.Fatalf("RgbToLabLBytes(%d,%d,%d) = %v, want %v", r, g, b, got, want)
				}
			}
		}
	}
}

// TestSrgbByteMatchesPow checks the threshold table at every step boundary and on either side of it. The encode is
// monotonic, so pinning all 255 boundaries pins the mapping for every input in between.
func TestSrgbByteMatchesPow(t *testing.T) {
	// The table is built on first use rather than at init, and this test reads it directly.
	buildSrgbByteThreshold()

	for k := 1; k <= 255; k++ {
		at := srgbByteThreshold[k-1]
		below := math.Nextafter(at, 0)

		if got, want := SrgbByte(at), srgbByteSlow(at); got != want || got != uint8(k) {
			t.Fatalf("at threshold %d: SrgbByte = %d, slow = %d, want %d", k, got, want, k)
		}

		if got, want := SrgbByte(below), srgbByteSlow(below); got != want || got != uint8(k-1) {
			t.Fatalf("below threshold %d: SrgbByte = %d, slow = %d, want %d", k, got, want, k-1)
		}
	}

	// Out-of-range inputs must saturate the same way Clamp255 did.
	for _, c := range []float64{-1, -0.0001, 0, 1, 1.5, 42} {
		if got, want := SrgbByte(c), srgbByteSlow(c); got != want {
			t.Fatalf("SrgbByte(%v) = %d, want %d", c, got, want)
		}
	}

	// A dense sweep across the whole range, as a second opinion on the boundary argument.
	for i := range 200001 {
		c := -0.05 + float64(i)*(1.1/200000.0)

		if got, want := SrgbByte(c), srgbByteSlow(c); got != want {
			t.Fatalf("SrgbByte(%v) = %d, want %d", c, got, want)
		}
	}
}

// TestLabToLinearRgbMatchesLabToRgb pins the split conversion: encoding LabToLinearRgb's output must reproduce
// LabToRgb's channels exactly.
func TestLabToLinearRgbMatchesLabToRgb(t *testing.T) {
	for l := 0; l <= 100; l += 2 {
		for a := -120; a <= 120; a += 15 {
			for b := -120; b <= 120; b += 15 {
				fl, fa, fb := float32(l), float32(a), float32(b)

				wr, wg, wb := LabToRgb(fl, fa, fb)
				lr, lg, lb := LabToLinearRgb(fl, fa, fb)

				for i, pair := range [][2]float64{{lr, float64(wr)}, {lg, float64(wg)}, {lb, float64(wb)}} {
					want := uint8(Clamp255(float32(pair[1])*255.0 + 0.5))

					if got := SrgbByte(pair[0]); got != want {
						t.Fatalf("Lab(%d,%d,%d) channel %d: SrgbByte = %d, want %d", l, a, b, i, got, want)
					}
				}
			}
		}
	}
}

// TestGrayFromRgbBytesMatchesLabRoundTrip pins the collapsed form against the Lab round-trip it replaced. The two are
// not required to be bit-identical - the point is that the difference is far below the 8-bit quantization the value
// feeds into, which is what makes skipping the round-trip safe.
func TestGrayFromRgbBytesMatchesLabRoundTrip(t *testing.T) {
	var worst float64

	check := func(r, g, b uint8) {
		t.Helper()

		l := RgbToLabLBytes(r, g, b)
		want, _, _ := LabToRgb(l, 0, 0)
		got := GrayFromRgbBytes(r, g, b)

		d := math.Abs(float64(got) - float64(want))
		worst = math.Max(worst, d)

		// One 8-bit step is 1/255 = 0.0039, so this leaves four orders of magnitude of headroom.
		if d > 1e-4 {
			t.Fatalf("rgb(%d,%d,%d): got %v, want %v (delta %g)", r, g, b, got, want, d)
		}
	}

	// The gray diagonal exhaustively, then a coprime-strided sweep of the cube so no channel's stride aliases another.
	for v := range 256 {
		check(uint8(v), uint8(v), uint8(v))
	}
	for r := 0; r < 256; r += 3 {
		for g := 0; g < 256; g += 5 {
			for b := 0; b < 256; b += 7 {
				check(uint8(r), uint8(g), uint8(b))
			}
		}
	}

	t.Logf("worst delta %g against a quantization step of %g", worst, 1.0/255.0)
}
