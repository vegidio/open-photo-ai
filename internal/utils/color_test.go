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
