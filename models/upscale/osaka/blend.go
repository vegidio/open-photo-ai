package osaka

import (
	"image"
	"math"
)

// canvas accumulates weighted tile contributions in planar CHW float32 and normalizes once at the end, so every
// pixel is a proper weighted average of every tile that covered it.
//
// The shared blendTileWithOverlap is not reusable here for two reasons. It ramps only the left and top edges and
// relies on later tiles overwriting earlier ones for the right and bottom, which - combined with the rule that shifts
// an edge tile back in-bounds rather than shrinking it - leaves a hard discontinuity along the last row and column.
// And its ramp is linear, which is continuous but not smooth: the slope jumps at both ends of the overlap, and with
// per-tile stochastic content that reads as a visible band.
//
// Working in CHW rather than in an image also means the decoder's output is accumulated as it comes out, with no
// per-tile conversion to and from 8-bit.
type canvas struct {
	width  int
	height int
	sum    []float32 // 3 planes of width*height
	weight []float32 // one plane of width*height, shared by the three channels
}

func newCanvas(width, height int) *canvas {
	return &canvas{
		width:  width,
		height: height,
		sum:    make([]float32, 3*width*height),
		weight: make([]float32, width*height),
	}
}

// add accumulates one tile. rect is where the tile sits on the canvas, and feather is how many pixels the weight
// ramps over at each edge that borders another tile.
//
// Edges lying on the image border are deliberately not feathered: nothing overlaps them, so ramping there would drive
// the accumulated weight towards zero along the outside of the image and amplify whatever noise survived the
// division. Only interior edges - the ones another tile also covers - are ramped.
func (c *canvas) add(tile []float32, rect image.Rectangle, feather int) {
	tw, th := rect.Dx(), rect.Dy()
	if tw <= 0 || th <= 0 {
		return
	}

	rowWeights := edgeWeights(tw, feather, rect.Min.X > 0, rect.Max.X < c.width)
	colWeights := edgeWeights(th, feather, rect.Min.Y > 0, rect.Max.Y < c.height)

	tilePlane := tw * th
	canvasPlane := c.width * c.height

	for y := range th {
		dstY := rect.Min.Y + y
		if dstY < 0 || dstY >= c.height {
			continue
		}

		for x := range tw {
			dstX := rect.Min.X + x
			if dstX < 0 || dstX >= c.width {
				continue
			}

			w := rowWeights[x] * colWeights[y]
			src := y*tw + x
			dst := dstY*c.width + dstX

			c.sum[dst] += tile[src] * w
			c.sum[canvasPlane+dst] += tile[tilePlane+src] * w
			c.sum[2*canvasPlane+dst] += tile[2*tilePlane+src] * w
			c.weight[dst] += w
		}
	}
}

// resolve divides the accumulated sums by their weights, returning planar CHW data ready for CHWToImage.
//
// The division is in place: the accumulator is the result, and the canvas is dropped by its caller immediately after.
// A second buffer would double peak float32 image memory - around 2.3 GB on a 4x pass over a 12 MP photo - on a
// machine that is already holding a 7 GB model.
func (c *canvas) resolve() []float32 {
	plane := c.width * c.height

	for i := range plane {
		w := c.weight[i]
		if w <= 0 {
			// Unreachable while every pixel is covered by at least one tile, which the grid guarantees. Leaving the
			// pixel at its accumulated zero rather than dividing keeps a hole visible instead of an infinity.
			continue
		}

		c.sum[i] /= w
		c.sum[plane+i] /= w
		c.sum[2*plane+i] /= w
	}

	return c.sum
}

// edgeWeights builds the per-axis weight ramp for a tile of the given length.
//
// The ramp is a raised cosine, which is smooth at both ends where a linear ramp has a slope discontinuity. Two
// abutting raised-cosine ramps of equal width sum to exactly one, so where tiles overlap by exactly the feather width
// the weights already form a partition of unity and the normalization in resolve is a no-op. Where they overlap by
// more - which the shift-an-edge-tile-back-in-bounds rule guarantees for the last row and column - the normalization
// is what makes the result correct.
func edgeWeights(length, feather int, rampStart, rampEnd bool) []float32 {
	weights := make([]float32, length)
	for i := range weights {
		weights[i] = 1
	}

	if feather <= 0 {
		return weights
	}

	if feather > length/2 {
		feather = length / 2
	}

	for i := range feather {
		// The half-pixel offset keeps the outermost weight strictly positive, so a pixel covered by exactly one
		// feathered tile still normalizes back to its own value.
		w := float32(0.5 - 0.5*math.Cos(math.Pi*(float64(i)+0.5)/float64(feather)))

		if rampStart {
			weights[i] = w
		}
		if rampEnd {
			weights[length-1-i] *= w
		}
	}

	return weights
}
