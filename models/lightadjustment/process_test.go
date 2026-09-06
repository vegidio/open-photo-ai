package lightadjustment

import (
	"bytes"
	"image"
	"image/color"
	"math"
	"testing"
)

// genericImage wraps an image without exposing a concrete RGBA-family type, forcing RgbPixBuffer to report ok=false
// and pushing buildResult onto its generic At() fallback. It is the reference the fast path must match.
type genericImage struct{ src image.Image }

func (g genericImage) ColorModel() color.Model { return g.src.ColorModel() }
func (g genericImage) Bounds() image.Rectangle { return g.src.Bounds() }
func (g genericImage) At(x, y int) color.Color { return g.src.At(x, y) }

// synthNRGBA builds a deterministic test image covering the full channel range, including non-opaque alpha (which is
// where the premultiply in Sample16 has to agree with NRGBA.RGBA()).
func synthNRGBA(w, h int, alpha bool) *image.NRGBA {
	img := image.NewNRGBA(image.Rect(0, 0, w, h))
	for y := range h {
		for x := range w {
			i := y*img.Stride + x*4
			img.Pix[i] = uint8((x*7 + y*3) % 256)
			img.Pix[i+1] = uint8((x*13 + y*11) % 256)
			img.Pix[i+2] = uint8((x*29 + y*17) % 256)
			img.Pix[i+3] = 255
			if alpha {
				img.Pix[i+3] = uint8((x*5 + y*23) % 256)
			}
		}
	}
	return img
}

func synthRGBA(w, h int) *image.RGBA {
	img := image.NewRGBA(image.Rect(0, 0, w, h))
	for y := range h {
		for x := range w {
			i := y*img.Stride + x*4
			img.Pix[i] = uint8((x*7 + y*3) % 256)
			img.Pix[i+1] = uint8((x*13 + y*11) % 256)
			img.Pix[i+2] = uint8((x*29 + y*17) % 256)
			img.Pix[i+3] = 255
		}
	}
	return img
}

// TestBuildResultFastPathMatchesGeneric is the guard on the pixel fast path: swapping interface dispatch for direct
// Pix indexing is purely a performance change, so both paths must produce byte-identical output.
func TestBuildResultFastPathMatchesGeneric(t *testing.T) {
	const fullW, fullH = 97, 61 // deliberately not a multiple of anything
	const lowW, lowH = 31, 19

	tests := []struct {
		name string
		img  image.Image
	}{
		{"NRGBA opaque", synthNRGBA(fullW, fullH, false)},
		{"NRGBA with alpha", synthNRGBA(fullW, fullH, true)},
		{"RGBA premultiplied", synthRGBA(fullW, fullH)},
	}

	resized := synthNRGBA(lowW, lowH, false)
	outLR := synthRGBA(lowW, lowH)

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			fast := buildResult(tt.img, resized, outLR).(*image.RGBA)
			slow := buildResult(genericImage{tt.img}, resized, outLR).(*image.RGBA)

			if !bytes.Equal(fast.Pix, slow.Pix) {
				diff := 0
				for i := range fast.Pix {
					if fast.Pix[i] != slow.Pix[i] {
						diff++
					}
				}
				t.Fatalf("fast path diverges from generic path in %d/%d bytes", diff, len(fast.Pix))
			}
		})
	}
}

// planCanvas replaced two constants in Process with per-variant data. The risk that carries is a silent change to
// paris, whose geometry had no test at all - so this reproduces the original arithmetic literally and requires the
// new code to agree with it at every size that hits a different branch.
func TestPlanCanvasParisMatchesLegacyGeometry(t *testing.T) {
	// The code Process used before Canvas existed, transcribed rather than called.
	legacy := func(fullW, fullH int) plan {
		const maxSize = 1024
		roundUpTo16 := func(v int) int {
			if v%16 == 0 {
				return v
			}
			return v + (16 - v%16)
		}
		fitToMaxSize := func(w, h, maxSize int) (int, int) {
			longest := max(h, w)
			ratio := float64(maxSize) / float64(longest)
			return roundUpTo16(int(math.Round(float64(w) * ratio))), roundUpTo16(int(math.Round(float64(h) * ratio)))
		}

		scaledW, scaledH := fullW, fullH
		resize := max(fullW, fullH) > maxSize
		if resize {
			scaledW, scaledH = fitToMaxSize(fullW, fullH, maxSize)
		}
		return plan{
			scaledW: scaledW, scaledH: scaledH,
			padW:   roundUpTo16(scaledW) - scaledW,
			padH:   roundUpTo16(scaledH) - scaledH,
			resize: resize,
		}
	}

	sizes := [][2]int{
		{800, 600},   // inside the ceiling, both sides need padding
		{1000, 750},  // the size measured in the comment in Process
		{1024, 1024}, // exactly the ceiling, already aligned
		{1024, 768},  // at the ceiling, no resize
		{2000, 1500}, // over the ceiling, landscape
		{1080, 1920}, // over the ceiling, portrait
		{1920, 1080},
		{100, 100},
		{16, 16},
		{1, 1},
	}

	paris := Canvas{MaxSize: 1024, Align: 16}
	for _, s := range sizes {
		w, h := s[0], s[1]
		got, want := planCanvas(w, h, paris), legacy(w, h)
		if got != want {
			t.Errorf("planCanvas(%d, %d) = %+v, legacy geometry = %+v", w, h, got, want)
		}
	}
}

// TestPlanCanvasSquare covers lyon: the longest side lands exactly on MaxSize whichever way the image is oriented,
// the short side is padded out to the square, and the result is always the gain-map path.
func TestPlanCanvasSquare(t *testing.T) {
	c := Canvas{MaxSize: 1024, Square: true}

	tests := []struct {
		name string
		w, h int
		want plan
	}{
		{"landscape 3:2", 6000, 4000, plan{scaledW: 1024, scaledH: 683, padW: 0, padH: 341, resize: true}},
		{"portrait 2:3", 4000, 6000, plan{scaledW: 683, scaledH: 1024, padW: 341, padH: 0, resize: true}},
		{"already square", 2048, 2048, plan{scaledW: 1024, scaledH: 1024, padW: 0, padH: 0, resize: true}},
		{"exactly the canvas", 1024, 683, plan{scaledW: 1024, scaledH: 683, padW: 0, padH: 341, resize: true}},
		// Smaller than the canvas is still enlarged: a fixed-shape graph accepts one size and nothing else.
		{"smaller than canvas", 400, 300, plan{scaledW: 1024, scaledH: 768, padW: 0, padH: 256, resize: true}},
		{"very wide", 4000, 100, plan{scaledW: 1024, scaledH: 26, padW: 0, padH: 998, resize: true}},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := planCanvas(tt.w, tt.h, c)
			if got != tt.want {
				t.Errorf("planCanvas(%d, %d) = %+v, want %+v", tt.w, tt.h, got, tt.want)
			}
			if got.scaledW+got.padW != c.MaxSize || got.scaledH+got.padH != c.MaxSize {
				t.Errorf("geometry does not fill the %d square: %+v", c.MaxSize, got)
			}
			if max(got.scaledW, got.scaledH) != c.MaxSize {
				t.Errorf("longest side is %d, want %d", max(got.scaledW, got.scaledH), c.MaxSize)
			}
		})
	}
}

func TestAlignUp(t *testing.T) {
	tests := []struct{ v, n, want int }{
		{100, 16, 112}, {112, 16, 112}, {1, 16, 16},
		{100, 32, 128}, {128, 32, 128}, {683, 32, 704},
		{7, 1, 7}, // n == 1 is already aligned
		{7, 0, 7}, // a variant that declares no alignment must not divide by zero
		{0, 16, 0},
	}
	for _, tt := range tests {
		if got := alignUp(tt.v, tt.n); got != tt.want {
			t.Errorf("alignUp(%d, %d) = %d, want %d", tt.v, tt.n, got, tt.want)
		}
	}
}
