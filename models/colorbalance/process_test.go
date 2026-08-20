package colorbalance

import (
	"bytes"
	"image"
	"image/color"
	"testing"
)

// genericImage wraps an image without exposing a concrete RGBA-family type, forcing RgbPixBuffer to report ok=false
// and pushing applyMapping onto its generic At() fallback. It is the reference the fast path must match.
type genericImage struct{ src image.Image }

func (g genericImage) ColorModel() color.Model { return g.src.ColorModel() }
func (g genericImage) Bounds() image.Rectangle { return g.src.Bounds() }
func (g genericImage) At(x, y int) color.Color { return g.src.At(x, y) }

func synth(w, h int, origin image.Point, nrgba, alpha bool) image.Image {
	r := image.Rectangle{Min: origin, Max: origin.Add(image.Point{X: w, Y: h})}

	fill := func(pix []uint8, stride int) {
		for y := range h {
			for x := range w {
				i := y*stride + x*4
				pix[i] = uint8((x*7 + y*3) % 256)
				pix[i+1] = uint8((x*13 + y*11) % 256)
				pix[i+2] = uint8((x*29 + y*17) % 256)
				pix[i+3] = 255
				if alpha {
					pix[i+3] = uint8((x*5 + y*23) % 256)
				}
			}
		}
	}

	if nrgba {
		img := image.NewNRGBA(r)
		fill(img.Pix, img.Stride)
		return img
	}

	img := image.NewRGBA(r)
	fill(img.Pix, img.Stride)
	return img
}

// TestApplyMappingFastPathMatchesGeneric guards the pixel fast path: swapping interface dispatch for direct Pix
// indexing is purely a performance change, so both paths must produce byte-identical output. The non-origin case
// matters because the fast path relies on RgbPixBuffer offsets already being relative to Bounds().Min.
func TestApplyMappingFastPathMatchesGeneric(t *testing.T) {
	const w, h = 97, 61

	var weights [11][3]float32
	for i := range weights {
		weights[i][0] = float32(i)*0.03 - 0.2
		weights[i][1] = float32(i)*-0.02 + 0.15
		weights[i][2] = float32(i)*0.017 + 0.05
	}

	tests := []struct {
		name string
		img  image.Image
	}{
		{"NRGBA opaque", synth(w, h, image.Point{}, true, false)},
		{"NRGBA with alpha", synth(w, h, image.Point{}, true, true)},
		{"RGBA premultiplied", synth(w, h, image.Point{}, false, false)},
		{"NRGBA non-origin bounds", synth(w, h, image.Point{X: 13, Y: 7}, true, true)},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			fast := applyMapping(tt.img, weights).(*image.RGBA)
			slow := applyMapping(genericImage{tt.img}, weights).(*image.RGBA)

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
