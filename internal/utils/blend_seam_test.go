package utils

import (
	"image"
	"math"
	"testing"
)

// The worked example from TileGrid.OverlapsX: at Size 256 / Overlap 16 over a 600px side the offsets are
// [0, 240, 344], so the last tile overlaps its predecessor by 152 - not by the configured 16. Blending that seam with
// a 16px ramp is what left a hard edge down the last column.
func TestTileGridOverlapsReportTheRealOverlap(t *testing.T) {
	g := TileGrid{Size: 256, Overlap: 16, Width: 600, Height: 600}

	got := g.OverlapsX()
	want := []int{0, 16, 152}

	if len(got) != len(want) {
		t.Fatalf("OverlapsX() = %v, want %v", got, want)
	}

	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("OverlapsX() = %v, want %v", got, want)
		}
	}
}

// Every overlap must be consistent with the offsets the grid actually produces, at any geometry: tile i starts where
// tile i-1 ends, minus the reported overlap.
func TestTileGridOverlapsAgreeWithOffsets(t *testing.T) {
	for _, g := range []TileGrid{
		{Size: 256, Overlap: 16, Width: 600, Height: 401},
		{Size: 512, Overlap: 128, Width: 1280, Height: 1280},
		{Size: 256, Overlap: 32, Width: 257, Height: 1000},
		{Size: 64, Overlap: 8, Width: 4000, Height: 63},
	} {
		for _, axis := range []struct {
			name     string
			length   int
			overlaps []int
		}{
			{"x", g.Width, g.OverlapsX()},
			{"y", g.Height, g.OverlapsY()},
		} {
			offs := g.offsets(axis.length)
			extent := g.extent(axis.length)

			if len(offs) != len(axis.overlaps) {
				t.Fatalf("%v %s: %d offsets but %d overlaps", g, axis.name, len(offs), len(axis.overlaps))
			}

			if len(axis.overlaps) > 0 && axis.overlaps[0] != 0 {
				t.Errorf("%v %s: first overlap is %d, want 0", g, axis.name, axis.overlaps[0])
			}

			for i := 1; i < len(offs); i++ {
				want := offs[i-1] + extent - offs[i]
				if axis.overlaps[i] != want {
					t.Errorf("%v %s: overlap[%d] = %d, want %d", g, axis.name, i, axis.overlaps[i], want)
				}
			}
		}
	}
}

// A flat tile blended over a differently-coloured background must not leave a step: with the ramp derived from the
// real overlap, every adjacent pair of columns across the seam differs by a small amount. The pre-fix code ramped over
// 16px and then jumped the remaining ~119 levels in a single column.
func TestBlendOverWideOverlapLeavesNoStep(t *testing.T) {
	const (
		width   = 200
		height  = 8
		overlap = 120
	)

	dst := image.NewRGBA(image.Rect(0, 0, width, height))
	fill(dst, 0)

	tile := image.NewRGBA(image.Rect(0, 0, width, height))
	fill(tile, 240)

	// Placed so its left ramp covers [0, overlap) of the destination, the wide-overlap case.
	blendTileWithOverlap(dst, tile, 0, 0, overlap, 0)

	maxStep := 0
	for x := 1; x < width; x++ {
		step := int(dst.Pix[dst.PixOffset(x, height/2)]) - int(dst.Pix[dst.PixOffset(x-1, height/2)])
		maxStep = max(maxStep, int(math.Abs(float64(step))))
	}

	// A raised cosine over 120 columns spanning 240 levels peaks at ~2*240/120 = 4 levels per column.
	if maxStep > 6 {
		t.Errorf("largest single-column step is %d; the seam is not smooth", maxStep)
	}

	// And it must actually reach the tile's value by the end of the ramp, or the blend is merely gentle rather than
	// complete.
	if got := dst.Pix[dst.PixOffset(width-1, height/2)]; got != 240 {
		t.Errorf("past the ramp the tile should replace the background; got %d, want 240", got)
	}
}

// The bulk row copy has to be output-identical to the per-pixel path it replaces. NRGBA takes the per-pixel path, RGBA
// takes the bulk one, and over an opaque tile the two must agree exactly.
func TestBlendBulkRowMatchesPerPixel(t *testing.T) {
	const w, h = 64, 16

	rgba := image.NewRGBA(image.Rect(0, 0, w, h))
	nrgba := image.NewNRGBA(image.Rect(0, 0, w, h))

	for y := range h {
		for x := range w {
			r, g, b := uint8(x*3), uint8(y*7), uint8(x+y)

			i := rgba.PixOffset(x, y)
			rgba.Pix[i], rgba.Pix[i+1], rgba.Pix[i+2], rgba.Pix[i+3] = r, g, b, 255

			j := nrgba.PixOffset(x, y)
			nrgba.Pix[j], nrgba.Pix[j+1], nrgba.Pix[j+2], nrgba.Pix[j+3] = r, g, b, 255
		}
	}

	fast := image.NewRGBA(image.Rect(0, 0, w, h))
	slow := image.NewRGBA(image.Rect(0, 0, w, h))
	fill(fast, 30)
	fill(slow, 30)

	blendTileWithOverlap(fast, rgba, 0, 0, 8, 4)
	blendTileWithOverlap(slow, nrgba, 0, 0, 8, 4)

	for i := range fast.Pix {
		if fast.Pix[i] != slow.Pix[i] {
			t.Fatalf("byte %d: bulk path wrote %d, per-pixel path wrote %d", i, fast.Pix[i], slow.Pix[i])
		}
	}
}

func fill(img *image.RGBA, v uint8) {
	for i := 0; i < len(img.Pix); i += 4 {
		img.Pix[i], img.Pix[i+1], img.Pix[i+2], img.Pix[i+3] = v, v, v, 255
	}
}
