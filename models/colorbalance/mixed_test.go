package colorbalance

import (
	"bytes"
	"image"
	"math"
	"testing"
)

// synthWeights builds a canvas-sized set of weight planes that is a partition of unity over its top-left
// cropW x cropH region. The values vary smoothly across the frame so the bilinear sampling is actually exercised,
// and the padded region is filled with something deliberately different so a crop mistake shows up as a wrong result
// rather than as a harmless one.
func synthWeights(settings, canvasSize, cropW, cropH int) []float32 {
	plane := canvasSize * canvasSize
	out := make([]float32, settings*plane)

	for i := range out {
		out[i] = -7 // poison: nothing may read the padding
	}

	for y := range cropH {
		for x := range cropW {
			raw := make([]float32, settings)
			var sum float32

			for c := range settings {
				v := float32(0.25 + 0.5*math.Abs(math.Sin(float64(x*(c+2))*0.031+float64(y*(c+1))*0.017)))
				raw[c] = v
				sum += v
			}

			for c := range settings {
				out[c*plane+y*canvasSize+x] = raw[c] / sum
			}
		}
	}

	return out
}

// synthMaps builds one polynomial mapping per rendering. The first is a mild warm shift, the second a mild cool one,
// which is roughly what shade and tungsten do to a photo.
func synthMaps() [][11][3]float32 {
	var warm, cool [11][3]float32

	// Identity on the linear terms, then a small per-channel gain and offset.
	for c := range 3 {
		warm[c][c] = 1
		cool[c][c] = 1
	}
	warm[0][0], warm[2][2] = 1.12, 0.88
	warm[10][0] = 0.02
	cool[0][0], cool[2][2] = 0.9, 1.15
	cool[10][2] = 0.03

	return [][11][3]float32{warm, cool}
}

// TestBlendWeightedMatchesReference is the guard on the fused pass. Materialising the renderings, upsampling every
// weight plane separately and combining afterwards is what the code used to have to do; fusing all of that into one
// loop is a performance change, so the two must be byte-identical.
func TestBlendWeightedMatchesReference(t *testing.T) {
	const canvasSize, cropW, cropH = 64, 64, 40

	maps := synthMaps()
	weights := synthWeights(len(maps)+1, canvasSize, cropW, cropH)

	for _, tc := range []struct {
		name string
		img  image.Image
	}{
		{"rgba", synth(97, 61, image.Point{}, false, false)},
		{"nrgba", synth(97, 61, image.Point{}, true, false)},
		{"downscale", synth(31, 19, image.Point{}, false, false)},
	} {
		t.Run(tc.name, func(t *testing.T) {
			got := blendWeighted(tc.img, maps, weights, canvasSize, cropW, cropH)
			want := referenceBlend(tc.img, maps, weights, canvasSize, cropW, cropH)

			if !bytes.Equal(got.(*image.RGBA).Pix, want.(*image.RGBA).Pix) {
				t.Error("fused blend does not match the reference implementation")
			}
		})
	}
}

// TestBlendWeightedFastPathMatchesGeneric guards the pixel fast path the same way applyMapping's test does. The
// non-origin case is the one that actually catches RgbPixBuffer offset mistakes, and the premultiplied and
// straight-alpha cases are what force Sample16 down its un-premultiplying branch.
func TestBlendWeightedFastPathMatchesGeneric(t *testing.T) {
	const canvasSize, cropW, cropH = 48, 48, 32

	maps := synthMaps()
	weights := synthWeights(len(maps)+1, canvasSize, cropW, cropH)

	for _, tc := range []struct {
		name   string
		origin image.Point
		nrgba  bool
		alpha  bool
	}{
		{"rgba", image.Point{}, false, false},
		{"rgba premultiplied alpha", image.Point{}, false, true},
		{"nrgba", image.Point{}, true, false},
		{"nrgba straight alpha", image.Point{}, true, true},
		{"non-origin bounds", image.Point{X: 13, Y: 7}, false, false},
		{"non-origin nrgba", image.Point{X: 5, Y: 11}, true, true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			img := synth(53, 37, tc.origin, tc.nrgba, tc.alpha)

			fast := blendWeighted(img, maps, weights, canvasSize, cropW, cropH)
			generic := blendWeighted(genericImage{src: img}, maps, weights, canvasSize, cropW, cropH)

			if !bytes.Equal(fast.(*image.RGBA).Pix, generic.(*image.RGBA).Pix) {
				t.Error("fast path and generic path disagree")
			}
		})
	}
}

// TestBlendedWeightsArePartitionOfUnity is the assertion behind blendWeighted not renormalising after it interpolates.
// Bilinear interpolation is a convex combination, so it commutes with the sum over channels - but that is the kind of
// claim that is easy to write in a comment and quietly break, so it is checked at every destination pixel including
// the corners, which are where the edge clamping could have gone wrong.
func TestBlendedWeightsArePartitionOfUnity(t *testing.T) {
	const canvasSize, cropW, cropH, settings = 32, 32, 21, 3

	weights := synthWeights(settings, canvasSize, cropW, cropH)
	plane := canvasSize * canvasSize

	const dstW, dstH = 77, 45
	x0s, x1s, fxs := sampleAxis(cropW, dstW)
	y0s, y1s, fys := sampleAxis(cropH, dstH)

	for y := range dstH {
		for x := range dstW {
			var sum float32

			for c := range settings {
				base := c * plane
				top := weights[base+y0s[y]*canvasSize+x0s[x]]*(1-fxs[x]) + weights[base+y0s[y]*canvasSize+x1s[x]]*fxs[x]
				bottom := weights[base+y1s[y]*canvasSize+x0s[x]]*(1-fxs[x]) + weights[base+y1s[y]*canvasSize+x1s[x]]*fxs[x]
				sum += top*(1-fys[y]) + bottom*fys[y]
			}

			if math.Abs(float64(sum)-1) > 1e-6 {
				t.Fatalf("weights at (%d,%d) sum to %v, want 1", x, y, sum)
			}
		}
	}
}

// TestBlendWeightedClampsRenderingsBeforeBlending pins the ordering that separates this from a plausible-looking
// mistake. A mapping that sends a bright pixel well past 1 must be clipped before it is weighted, the way the
// reference pipeline clips the renderings it materialises - clamping only the finished blend instead lets that
// rendering drag the result somewhere the reference never goes.
func TestBlendWeightedClampsRenderingsBeforeBlending(t *testing.T) {
	const canvasSize, cropW, cropH = 16, 16, 16

	// One rendering that doubles every channel, so any pixel above 0.5 leaves gamut, and one that leaves it alone.
	var blown, identity [11][3]float32
	for c := range 3 {
		blown[c][c] = 2
		identity[c][c] = 1
	}
	maps := [][11][3]float32{blown, identity}

	// Put all the weight on the blown rendering so the clamp is the only thing standing between it and the output.
	plane := canvasSize * canvasSize
	weights := make([]float32, 3*plane)
	for i := range plane {
		weights[plane+i] = 1
	}

	img := synth(24, 16, image.Point{}, false, false)

	got := blendWeighted(img, maps, weights, canvasSize, cropW, cropH)
	want := referenceBlend(img, maps, weights, canvasSize, cropW, cropH)

	if !bytes.Equal(got.(*image.RGBA).Pix, want.(*image.RGBA).Pix) {
		t.Fatal("blend does not clip the rendering the way the reference does")
	}

	// And the result must actually be saturated, or the case proves nothing.
	pix := got.(*image.RGBA).Pix
	var saturated int
	for i := 0; i < len(pix); i += 4 {
		if pix[i] == 255 {
			saturated++
		}
	}
	if saturated == 0 {
		t.Error("no pixel left gamut; the test case does not exercise the clamp")
	}
}

// TestPlanCanvasContiguousCrop covers the property both the fit and the weight sampling rely on: the padding is only
// ever on the right and bottom, so the region of interest starts at the origin and every row of it is contiguous.
func TestPlanCanvasContiguousCrop(t *testing.T) {
	canvas := Canvas{Size: 656}

	for _, size := range [][2]int{{4000, 3000}, {3000, 4000}, {640, 640}, {1920, 1080}, {100, 3000}, {37, 11}} {
		p := planCanvas(size[0], size[1], canvas)

		if p.scaledW > canvas.Size || p.scaledH > canvas.Size {
			t.Errorf("%v: scaled to %dx%d, larger than the %d canvas", size, p.scaledW, p.scaledH, canvas.Size)
		}
		if p.padW != canvas.Size-p.scaledW || p.padH != canvas.Size-p.scaledH {
			t.Errorf("%v: padding %dx%d does not fill the canvas", size, p.padW, p.padH)
		}
		if max(p.scaledW, p.scaledH) != canvas.Size {
			t.Errorf("%v: longest side is %d, want %d", size, max(p.scaledW, p.scaledH), canvas.Size)
		}
	}
}
