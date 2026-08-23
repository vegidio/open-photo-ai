package utils

import (
	"image"
	"image/color"
	"testing"

	"github.com/disintegration/imaging"
)

// TestPrepareTileScratchMatchesCrop pins the reused-buffer fast path against the imaging.Crop it replaced. The two
// must agree for every source type the drivers hand in - the translucent case matters most, because Crop and
// draw.Draw reach NRGBA by different routes and only agree if both un-premultiply the same way.
func TestPrepareTileScratchMatchesCrop(t *testing.T) {
	const tileSize = 64

	for _, src := range []image.Image{synthRGBA(200, 150), synthNRGBA(200, 150), synthTranslucent(200, 150)} {
		scratch := &tileScratch{tile: image.NewNRGBA(image.Rect(0, 0, tileSize, tileSize))}

		for _, at := range []image.Point{{X: 0, Y: 0}, {X: 64, Y: 32}, {X: 136, Y: 86}} {
			want := imaging.Crop(src, image.Rect(at.X, at.Y, at.X+tileSize, at.Y+tileSize))
			got := prepareTileForInference(src, scratch, at.X, at.Y, tileSize, tileSize, tileSize)

			for y := range tileSize {
				for x := range tileSize {
					if want.At(x, y) != got.At(x, y) {
						t.Fatalf("%T tile at %v, px(%d,%d): %v vs %v", src, at, x, y, want.At(x, y), got.At(x, y))
					}
				}
			}
		}
	}
}

// TestCHWToImageIntoMatchesCHWToImage pins the reused-buffer decode against the allocating one, on a destination
// deliberately larger than the tile so a stride the caller does not own is exercised.
func TestCHWToImageIntoMatchesCHWToImage(t *testing.T) {
	const w, h = 37, 23

	data := make([]float32, 3*w*h)
	for i := range data {
		data[i] = float32(i%271) / 271.0
	}

	for _, standardize := range []bool{false, true} {
		want := CHWToImage(data, w, h, standardize)
		got := CHWToImageInto(image.NewRGBA(image.Rect(0, 0, w+11, h+7)), data, w, h, standardize)

		for y := range h {
			for x := range w {
				if want.At(x, y) != got.At(x, y) {
					t.Fatalf("standardize=%v px(%d,%d): %v vs %v", standardize, x, y, want.At(x, y), got.At(x, y))
				}
			}
		}
	}
}

func synthRGBA(w, h int) *image.RGBA {
	img := image.NewRGBA(image.Rect(0, 0, w, h))
	for y := range h {
		for x := range w {
			img.Set(x, y, color.RGBA{R: uint8(x * 3), G: uint8(y * 5), B: uint8(x ^ y), A: 255})
		}
	}

	return img
}

func synthNRGBA(w, h int) *image.NRGBA {
	img := image.NewNRGBA(image.Rect(0, 0, w, h))
	for y := range h {
		for x := range w {
			img.Set(x, y, color.NRGBA{R: uint8(x * 7), G: uint8(y * 11), B: uint8(x + y), A: 255})
		}
	}

	return img
}

func synthTranslucent(w, h int) *image.NRGBA {
	img := image.NewNRGBA(image.Rect(0, 0, w, h))
	for y := range h {
		for x := range w {
			img.Set(x, y, color.NRGBA{R: uint8(x * 3), G: uint8(y * 5), B: uint8(x * y), A: uint8(128 + x%128)})
		}
	}

	return img
}
