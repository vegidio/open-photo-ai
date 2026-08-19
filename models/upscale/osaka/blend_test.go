package osaka

import (
	"math"
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
)

// reference builds a deterministic CHW image with content at every frequency, so a blending bug shows up as a
// mismatch rather than being hidden by a flat field.
func reference(w, h int) []float32 {
	data := make([]float32, 3*w*h)
	plane := w * h

	for y := range h {
		for x := range w {
			i := y*w + x
			data[i] = float32(math.Sin(float64(x)*0.31) * math.Cos(float64(y)*0.17))
			data[plane+i] = float32(x%7)/7*2 - 1
			data[2*plane+i] = float32((x+y)%2)*2 - 1 // per-pixel checkerboard: worst case for a smooth blend
		}
	}

	return data
}

// crop extracts a tile from planar CHW data.
func crop(src []float32, w, h, x0, y0, tw, th int) []float32 {
	out := make([]float32, 3*tw*th)
	srcPlane, dstPlane := w*h, tw*th

	for y := range th {
		for x := range tw {
			s, d := (y0+y)*w+(x0+x), y*tw+x
			out[d] = src[s]
			out[dstPlane+d] = src[srcPlane+s]
			out[2*dstPlane+d] = src[2*srcPlane+s]
		}
	}

	return out
}

// When every tile carries the true content, the blended result must reproduce it exactly, whatever the tiling. This
// is the property that matters: it fails if the weights don't form a partition of unity after normalization, which
// is precisely the bug that shows up as seams.
func TestCanvasReproducesContentForEveryGeometry(t *testing.T) {
	tests := []struct {
		name                       string
		w, h, size, overlap, feath int
	}{
		{"single tile", 100, 80, 256, 128, 64},
		{"even grid", 480, 480, 256, 64, 32},
		{"ragged edges force shifted tiles", 700, 530, 256, 64, 32},
		{"osaka geometry", 3000, 2000, 1024, 128, 128},
		{"overlap larger than the feather", 600, 600, 256, 128, 32},
		{"no feather at all", 600, 600, 256, 64, 0},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			want := reference(tt.w, tt.h)
			c := newCanvas(tt.w, tt.h)

			grid := utils.TileGrid{Size: tt.size, Overlap: tt.overlap, Width: tt.w, Height: tt.h}
			for _, rect := range grid.Tiles() {
				tile := crop(want, tt.w, tt.h, rect.Min.X, rect.Min.Y, rect.Dx(), rect.Dy())
				c.add(tile, rect, tt.feath)
			}

			// Every pixel must have received some weight, or resolve would punch a hole in the image.
			for i, w := range c.weight {
				if w <= 0 {
					t.Fatalf("pixel (%d,%d) accumulated no weight", i%tt.w, i/tt.w)
				}
			}

			got := c.resolve()
			for i := range want {
				if d := math.Abs(float64(got[i] - want[i])); d > 1e-4 {
					px := i % (tt.w * tt.h)
					t.Fatalf("channel %d pixel (%d,%d): got %f want %f (delta %g)",
						i/(tt.w*tt.h), px%tt.w, px/tt.w, got[i], want[i], d)
				}
			}
		})
	}
}

// Two tiles overlapping by exactly the feather width should already sum to one before normalization: that is the
// property that makes the raised cosine the right ramp, and it keeps resolve from having to rescue the common case.
func TestFeatherIsAPartitionOfUnityAtMatchedOverlap(t *testing.T) {
	const length, feather = 64, 16

	left := edgeWeights(length, feather, false, true)  // image edge on the left, another tile on the right
	right := edgeWeights(length, feather, true, false) // another tile on the left, image edge on the right

	for i := range feather {
		sum := left[length-feather+i] + right[i]

		if math.Abs(float64(sum)-1) > 1e-6 {
			t.Fatalf("overlap position %d sums to %f, want 1", i, sum)
		}
	}
}

// An edge lying on the image border must not be ramped: there is nothing overlapping it to make up the difference.
func TestBorderEdgesAreNotFeathered(t *testing.T) {
	w := edgeWeights(64, 16, false, false)

	for i, v := range w {
		if v != 1 {
			t.Fatalf("border-only tile ramped at %d: %f", i, v)
		}
	}

	interior := edgeWeights(64, 16, true, true)
	if interior[0] >= 1 || interior[63] >= 1 {
		t.Fatalf("interior edges were not ramped: first=%f last=%f", interior[0], interior[63])
	}
}

// A feather wider than half the tile would make the two ramps overlap and drive the middle towards zero; it must be
// clamped instead.
func TestFeatherWiderThanTileIsClamped(t *testing.T) {
	w := edgeWeights(10, 40, true, true)

	for i, v := range w {
		if v <= 0 || v > 1 {
			t.Fatalf("weight %d out of range: %f", i, v)
		}
	}
}
