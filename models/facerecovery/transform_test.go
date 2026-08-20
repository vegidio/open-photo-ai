package facerecovery

import (
	"image"
	"image/color"
	"testing"
)

// genericImage wraps an image without exposing a concrete RGBA-family type, forcing RgbPixBuffer to report ok=false
// so the sampler takes its At() fallback. It is the reference the direct-Pix path must match.
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

var samplerCases = []struct {
	name string
	img  image.Image
}{
	{"NRGBA opaque", synth(41, 29, image.Point{}, true, false)},
	{"NRGBA with alpha", synth(41, 29, image.Point{}, true, true)},
	{"RGBA premultiplied", synth(41, 29, image.Point{}, false, false)},
	{"NRGBA non-origin bounds", synth(41, 29, image.Point{X: 11, Y: 5}, true, true)},
}

// TestSamplerFastPathMatchesGeneric checks the primitive both warpAffine and blendFace now rely on: direct Pix
// indexing must return exactly what At().RGBA() returns, for every pixel and every source type.
func TestSamplerFastPathMatchesGeneric(t *testing.T) {
	for _, tt := range samplerCases {
		t.Run(tt.name, func(t *testing.T) {
			fast := newSampler(tt.img)
			slow := newSampler(genericImage{tt.img})

			if !fast.fast {
				t.Fatal("expected the fast path to be taken for a concrete RGBA-family image")
			}
			if slow.fast {
				t.Fatal("expected the generic path for a wrapped image")
			}

			b := tt.img.Bounds()
			for y := b.Min.Y; y < b.Max.Y; y++ {
				for x := b.Min.X; x < b.Max.X; x++ {
					fr, fg, fb, fa := fast.at(x, y)
					sr, sg, sb, sa := slow.at(x, y)

					if fr != sr || fg != sg || fb != sb || fa != sa {
						t.Fatalf("at(%d,%d): fast (%d,%d,%d,%d) != generic (%d,%d,%d,%d)",
							x, y, fr, fg, fb, fa, sr, sg, sb, sa)
					}
				}
			}
		})
	}
}

// TestBilinearInterpolateFastPathMatchesGeneric covers both padding modes end to end, including the out-of-bounds
// coordinates where the reflect/clamp mapping decides which pixels get read.
func TestBilinearInterpolateFastPathMatchesGeneric(t *testing.T) {
	coords := []struct{ x, y float32 }{
		{0, 0}, {0.5, 0.5}, {12.25, 7.75}, {40.9, 28.9},
		{-3.5, -2.25}, {60.5, 44.5}, {-0.5, 15.5}, {20.5, -0.5},
	}

	for _, tt := range samplerCases {
		for _, reflect := range []bool{true, false} {
			t.Run(tt.name, func(t *testing.T) {
				fast := newSampler(tt.img)
				slow := newSampler(genericImage{tt.img})

				for _, c := range coords {
					got := bilinearInterpolate(fast, c.x, c.y, reflect)
					want := bilinearInterpolate(slow, c.x, c.y, reflect)

					if got != want {
						t.Fatalf("reflect=%v at (%g,%g): fast %v != generic %v", reflect, c.x, c.y, got, want)
					}
				}
			})
		}
	}
}
