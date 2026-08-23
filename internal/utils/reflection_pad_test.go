package utils

import (
	"image"
	"image/color"
	"image/draw"
	"math/rand"
	"testing"
)

// referenceReflectionPad is the pre-optimisation implementation, kept here only to prove the rewritten one is
// bit-identical to it.
func referenceReflectionPad(img image.Image, left, top, right, bottom int) image.Image {
	bounds := img.Bounds()
	width, height := bounds.Dx(), bounds.Dy()

	paddedWidth := width + left + right
	paddedHeight := height + top + bottom
	padded := image.NewRGBA(image.Rect(0, 0, paddedWidth, paddedHeight))

	reflectIndex := func(idx, max int) int {
		if idx < 0 {
			return -idx - 1
		}
		if idx >= max {
			return 2*max - idx - 1
		}
		return idx
	}

	draw.Draw(padded, image.Rect(left, top, left+width, top+height), img, bounds.Min, draw.Src)

	for y := range height {
		srcY := bounds.Min.Y + y
		dstY := y + top

		for x := range left {
			srcX := bounds.Min.X + reflectIndex(left-x-1, width)
			padded.Set(x, dstY, img.At(srcX, srcY))
		}

		for x := range right {
			srcX := bounds.Min.X + reflectIndex(width+x, width)
			padded.Set(width+left+x, dstY, img.At(srcX, srcY))
		}
	}

	for x := range paddedWidth {
		for y := range top {
			srcY := reflectIndex(top-y-1, height) + top
			padded.Set(x, y, padded.At(x, srcY))
		}

		for y := range bottom {
			srcY := reflectIndex(height+y, height) + top
			padded.Set(x, height+top+y, padded.At(x, srcY))
		}
	}

	return padded
}

func TestReflectionPadMatchesReference(t *testing.T) {
	rng := rand.New(rand.NewSource(1))

	makeNRGBA := func(w, h int, opaque bool) *image.NRGBA {
		m := image.NewNRGBA(image.Rect(0, 0, w, h))
		for i := 0; i < len(m.Pix); i += 4 {
			m.Pix[i] = uint8(rng.Intn(256))
			m.Pix[i+1] = uint8(rng.Intn(256))
			m.Pix[i+2] = uint8(rng.Intn(256))
			if opaque {
				m.Pix[i+3] = 0xff
			} else {
				m.Pix[i+3] = uint8(rng.Intn(256))
			}
		}
		return m
	}

	makeRGBA := func(w, h int) *image.RGBA {
		m := image.NewRGBA(image.Rect(0, 0, w, h))
		for i := range m.Pix {
			m.Pix[i] = uint8(rng.Intn(256))
		}
		return m
	}

	makeGray := func(w, h int) *image.Gray {
		m := image.NewGray(image.Rect(0, 0, w, h))
		for i := range m.Pix {
			m.Pix[i] = uint8(rng.Intn(256))
		}
		return m
	}

	// All 13x9, because the pad list below goes up to 13x9 and the reference only agrees where a true mirror exists
	// - beyond that it read out of bounds and produced transparent black, which is the bug the clamp fixes.
	sources := map[string]image.Image{
		"nrgba-opaque": makeNRGBA(13, 9, true),
		"nrgba-alpha":  makeNRGBA(13, 9, false),
		"rgba":         makeRGBA(13, 9),
		"gray":         makeGray(13, 9),
	}

	// Includes the (0, 0, r, b) shape the tile driver actually uses, plus every other combination.
	pads := [][4]int{
		{0, 0, 0, 0},
		{0, 0, 5, 7},
		{4, 6, 0, 0},
		{3, 3, 3, 3},
		{1, 2, 3, 4},
		{13, 9, 13, 9}, // padding equal to the source dimensions, the largest pad with a true mirror
	}

	for name, src := range sources {
		for _, p := range pads {
			got := ReflectionPad(src, p[0], p[1], p[2], p[3]).(*image.RGBA)
			want := referenceReflectionPad(src, p[0], p[1], p[2], p[3]).(*image.RGBA)

			if got.Bounds() != want.Bounds() {
				t.Fatalf("%s pad %v: bounds %v, want %v", name, p, got.Bounds(), want.Bounds())
			}

			for y := range got.Bounds().Dy() {
				for x := range got.Bounds().Dx() {
					if g, w := got.RGBAAt(x, y), want.RGBAAt(x, y); g != w {
						t.Fatalf("%s pad %v: pixel (%d,%d) = %v, want %v", name, p, x, y, g, w)
					}
				}
			}
		}
	}
}

// A pad wider than the source is what a very small tile asks for, and it used to fall out of bounds and produce a
// black border. Every padded pixel must now come from the source.
func TestReflectionPadWiderThanSource(t *testing.T) {
	src := image.NewNRGBA(image.Rect(0, 0, 2, 3))
	for i := 0; i < len(src.Pix); i += 4 {
		src.Pix[i], src.Pix[i+1], src.Pix[i+2], src.Pix[i+3] = 10, 20, 30, 0xff
	}

	padded := ReflectionPad(src, 7, 5, 9, 11).(*image.RGBA)

	if got := padded.Bounds(); got != image.Rect(0, 0, 18, 19) {
		t.Fatalf("bounds = %v, want 18x19", got)
	}

	want := color.RGBA{R: 10, G: 20, B: 30, A: 0xff}
	for y := range padded.Bounds().Dy() {
		for x := range padded.Bounds().Dx() {
			if got := padded.RGBAAt(x, y); got != want {
				t.Fatalf("pixel (%d,%d) = %v, want %v", x, y, got, want)
			}
		}
	}
}
