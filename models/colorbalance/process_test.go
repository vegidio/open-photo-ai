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

// TestPlanCanvasSquare covers rio's geometry: the longest side lands exactly on Canvas.Size whichever way the image
// is oriented, and the short side is padded out to the square.
func TestPlanCanvasSquare(t *testing.T) {
	c := Canvas{Size: 656}

	tests := []struct {
		name string
		w, h int
		want plan
	}{
		{"landscape 3:2", 6000, 4000, plan{scaledW: 656, scaledH: 437, padW: 0, padH: 219}},
		{"portrait 2:3", 4000, 6000, plan{scaledW: 437, scaledH: 656, padW: 219, padH: 0}},
		{"already square", 2048, 2048, plan{scaledW: 656, scaledH: 656, padW: 0, padH: 0}},
		// Smaller than the canvas is still enlarged: a fixed-shape graph accepts one size and nothing else. This is
		// the one case the old FitWithinMaxSize geometry handled differently, where it ran at the image's own size.
		{"smaller than canvas", 400, 300, plan{scaledW: 656, scaledH: 492, padW: 0, padH: 164}},
		{"very wide", 4000, 100, plan{scaledW: 656, scaledH: 16, padW: 0, padH: 640}},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := planCanvas(tt.w, tt.h, c)
			if got != tt.want {
				t.Errorf("planCanvas(%d, %d) = %+v, want %+v", tt.w, tt.h, got, tt.want)
			}
			if got.scaledW+got.padW != c.Size || got.scaledH+got.padH != c.Size {
				t.Errorf("geometry does not fill the %d square: %+v", c.Size, got)
			}
			if max(got.scaledW, got.scaledH) != c.Size {
				t.Errorf("longest side is %d, want %d", max(got.scaledW, got.scaledH), c.Size)
			}
		})
	}
}

// TestChwToHWCCrops guards the one thing the fit depends on: chwToHWC must return the un-padded top-left region of
// the canvas, in row-major order, with the three planes recombined per pixel. Reading the padding back into the fit
// re-weights the image's border against the rest of the photo, and because the fit is global that moves every pixel
// of the result.
func TestChwToHWCCrops(t *testing.T) {
	const canvasW, canvasH, cropW, cropH = 5, 4, 3, 2

	plane := canvasW * canvasH
	data := make([]float32, 3*plane)
	for i := range plane {
		data[i] = float32(i) // R carries the canvas index, so a wrong offset is visible
		data[plane+i] = float32(100 + i)
		data[2*plane+i] = float32(200 + i)
	}

	got := chwToHWC(data, canvasW, canvasH, cropW, cropH)

	want := [][3]float32{
		{0, 100, 200}, {1, 101, 201}, {2, 102, 202},
		{5, 105, 205}, {6, 106, 206}, {7, 107, 207},
	}

	if len(got) != len(want) {
		t.Fatalf("got %d pixels, want %d", len(got), len(want))
	}
	for i := range want {
		if got[i] != want[i] {
			t.Errorf("pixel %d = %v, want %v", i, got[i], want[i])
		}
	}
}
