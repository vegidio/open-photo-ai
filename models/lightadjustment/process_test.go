package lightadjustment

import (
	"bytes"
	"image"
	"image/color"
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
