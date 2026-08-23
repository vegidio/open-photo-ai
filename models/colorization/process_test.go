package colorization

import (
	"image"
	"image/color"
	"math"
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
)

// genericImage wraps an image without exposing a concrete RGBA-family type, forcing RgbPixBuffer to report ok=false
// and pushing lPlane onto its generic At() fallback. It is the reference the fast path must match.
type genericImage struct{ src image.Image }

func (g genericImage) ColorModel() color.Model { return g.src.ColorModel() }
func (g genericImage) Bounds() image.Rectangle { return g.src.Bounds() }
func (g genericImage) At(x, y int) color.Color { return g.src.At(x, y) }

func synth(w, h int) *image.NRGBA {
	img := image.NewNRGBA(image.Rect(0, 0, w, h))
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

// TestLPlaneFastPathMatchesGeneric verifies the RgbPixBuffer fast path and the At() fallback extract identical
// luminance planes.
func TestLPlaneFastPathMatchesGeneric(t *testing.T) {
	img := synth(33, 21)

	fast := lPlane(img)
	generic := lPlane(genericImage{src: img})

	for i := range fast {
		if fast[i] != generic[i] {
			t.Fatalf("index %d: fast %v != generic %v", i, fast[i], generic[i])
		}
	}
}

// TestResizePlaneIdentity verifies a same-size resize is a plain copy.
func TestResizePlaneIdentity(t *testing.T) {
	src := []float32{1, 2, 3, 4, 5, 6}
	out := resizePlane(src, 3, 2, 3, 2)

	for i := range src {
		if out[i] != src[i] {
			t.Fatalf("index %d: got %v, want %v", i, out[i], src[i])
		}
	}

	out[0] = 42
	if src[0] == 42 {
		t.Fatal("identity resize must copy, not alias, the source")
	}
}

// TestResizePlaneConstant verifies interpolation of a constant plane stays constant at any output size.
func TestResizePlaneConstant(t *testing.T) {
	src := make([]float32, 4*4)
	for i := range src {
		src[i] = 7.5
	}

	out := resizePlane(src, 4, 4, 11, 5)
	for i, v := range out {
		if v != 7.5 {
			t.Fatalf("index %d: got %v, want 7.5", i, v)
		}
	}
}

// TestResizePlaneUpscale checks the center-aligned bilinear weights on a known 2x2 -> 4x4 upscale.
func TestResizePlaneUpscale(t *testing.T) {
	src := []float32{0, 10, 20, 30}
	out := resizePlane(src, 2, 2, 4, 4)

	// Row 0 samples sy=-0.25 (clamped to row 0); sx = -0.25, 0.25, 0.75, 1.25 -> clamped/interpolated.
	want := []float32{0, 2.5, 7.5, 10}
	for i := range want {
		if math.Abs(float64(out[i]-want[i])) > 1e-5 {
			t.Fatalf("row 0, index %d: got %v, want %v", i, out[i], want[i])
		}
	}

	// The last row clamps to source row 1.
	wantLast := []float32{20, 22.5, 27.5, 30}
	for i := range wantLast {
		got := out[3*4+i]
		if math.Abs(float64(got-wantLast[i])) > 1e-5 {
			t.Fatalf("row 3, index %d: got %v, want %v", i, got, wantLast[i])
		}
	}
}

// TestComposeLabPreservesGray verifies composing with zero chroma reproduces the luminance it was given: feeding a
// gray image through lPlane + composeLab must round-trip.
func TestComposeLabPreservesGray(t *testing.T) {
	const size = 16
	img := image.NewNRGBA(image.Rect(0, 0, size, size))
	for y := range size {
		for x := range size {
			v := uint8((x*16 + y) % 256)
			i := y*img.Stride + x*4
			img.Pix[i] = v
			img.Pix[i+1] = v
			img.Pix[i+2] = v
			img.Pix[i+3] = 255
		}
	}

	l := lPlane(img)
	zero := make([]float32, size*size)
	out := composeLab(l, zero, zero, size, size).(*image.RGBA)

	for y := range size {
		for x := range size {
			i := y*img.Stride + x*4
			o := y*out.Stride + x*4
			// One 8-bit count of tolerance for the float round-trip.
			if d := int(out.Pix[o]) - int(img.Pix[i]); d < -1 || d > 1 {
				t.Fatalf("(%d, %d): got %d, want %d", x, y, out.Pix[o], img.Pix[i])
			}
			// The conversion constants keep the neutral axis gray only to ~1e-5, so channels may straddle a
			// rounding boundary by one count.
			for c := 1; c <= 2; c++ {
				if d := int(out.Pix[o]) - int(out.Pix[o+c]); d < -1 || d > 1 {
					t.Fatalf("(%d, %d): zero chroma produced a non-gray pixel", x, y)
				}
			}
		}
	}
}

// TestGrayLumaInputIsNeutral verifies the DeOldify input tensor holds the ITU-601 luma replicated across all three
// channels, in [0, 1].
func TestGrayLumaInputIsNeutral(t *testing.T) {
	resized := image.NewNRGBA(image.Rect(0, 0, deoldifySize, deoldifySize))
	for y := range deoldifySize {
		for x := range deoldifySize {
			i := y*resized.Stride + x*4
			resized.Pix[i] = uint8(x % 256)
			resized.Pix[i+1] = uint8(y % 256)
			resized.Pix[i+2] = uint8((x + y) % 256)
			resized.Pix[i+3] = 255
		}
	}

	data := grayLumaInput(resized, deoldifySize)
	plane := deoldifySize * deoldifySize

	for i := 0; i < plane; i += 997 {
		r, g, b := data[i], data[plane+i], data[2*plane+i]
		if r < 0 || r > 1 {
			t.Fatalf("index %d: channel %v out of [0, 1]", i, r)
		}
		if r != g || r != b {
			t.Fatalf("index %d: input is not neutral gray (%v, %v, %v)", i, r, g, b)
		}
	}

	// Spot-check pixel 0 against the ITU-601 weights.
	pr, pg, pb, _ := utils.Sample16(resized.Pix, 0, true)
	want := (0.299*float32(pr) + 0.587*float32(pg) + 0.114*float32(pb)) / 65535.0
	if data[0] != want {
		t.Fatalf("pixel 0: got %v, want %v", data[0], want)
	}
}

// TestAbFromRgbNeutralGray verifies a neutral gray RGB tensor produces (near-)zero chroma planes, which is what
// composeLab relies on to keep unmodeled regions gray.
func TestAbFromRgbNeutralGray(t *testing.T) {
	// The chroma planes always come back square, at the graph's export size.
	const size = 8
	plane := size * size
	data := make([]float32, 3*plane)
	for i := range plane {
		v := float32(i) / float32(plane)
		data[i], data[plane+i], data[2*plane+i] = v, v, v
	}

	aPlane, bPlane := abFromRgb(data, size)
	for i := range plane {
		if math.Abs(float64(aPlane[i])) > 0.01 || math.Abs(float64(bPlane[i])) > 0.01 {
			t.Fatalf("index %d: gray produced chroma (%v, %v)", i, aPlane[i], bPlane[i])
		}
	}

	// Out-of-range values must be clamped, not propagated.
	aPlane, bPlane = abFromRgb([]float32{1.5, -0.5, 0.5}, 1)
	if math.IsNaN(float64(aPlane[0])) || math.IsNaN(float64(bPlane[0])) {
		t.Fatal("out-of-range input produced NaN chroma")
	}
}

// TestGrayLabInputIsNeutral verifies the model input tensor is a gray image (identical R, G, B planes) regardless of
// the source image's colors.
func TestGrayLabInputIsNeutral(t *testing.T) {
	resized := image.NewNRGBA(image.Rect(0, 0, inputSize, inputSize))
	for y := range inputSize {
		for x := range inputSize {
			i := y*resized.Stride + x*4
			resized.Pix[i] = uint8(x % 256)
			resized.Pix[i+1] = uint8(y % 256)
			resized.Pix[i+2] = uint8((x + y) % 256)
			resized.Pix[i+3] = 255
		}
	}

	data := grayLabInput(resized, inputSize)
	plane := inputSize * inputSize

	for i := 0; i < plane; i += 997 {
		r, g, b := data[i], data[plane+i], data[2*plane+i]
		if r < 0 || r > 1 {
			t.Fatalf("index %d: channel %v out of [0, 1]", i, r)
		}
		if math.Abs(float64(r-g)) > 1e-3 || math.Abs(float64(r-b)) > 1e-3 {
			t.Fatalf("index %d: input is not neutral gray (%v, %v, %v)", i, r, g, b)
		}
	}

	// Spot-check one pixel against the reference computation.
	pr, pg, pb, _ := utils.Sample16(resized.Pix, 0, true)
	l, _, _ := utils.RgbToLab(float32(pr)/65535.0, float32(pg)/65535.0, float32(pb)/65535.0)
	wr, _, _ := utils.LabToRgb(l, 0, 0)
	if data[0] != wr {
		t.Fatalf("pixel 0: got %v, want %v", data[0], wr)
	}
}

// synthChroma builds deterministic, non-trivial ab planes at model resolution, spanning the negative and positive
// chroma range so the composition is exercised well away from the neutral axis.
func synthChroma(size int) (aPlane, bPlane []float32) {
	plane := size * size
	aPlane = make([]float32, plane)
	bPlane = make([]float32, plane)

	for i := range plane {
		x, y := i%size, i/size
		aPlane[i] = float32((x*13+y*7)%241) - 120
		bPlane[i] = float32((x*29+y*17)%241) - 120
	}

	return aPlane, bPlane
}

// TestComposeMatchesReference is the guarantee behind fusing the four full-resolution passes into one, and behind the
// lookup tables that replaced the per-pixel math.Pow calls: for every pixel of every shape tested, the fused compose
// must produce the exact same byte as extracting, upsampling and combining separately. Not "within a count" — equal.
func TestComposeMatchesReference(t *testing.T) {
	sizes := []struct{ srcSize, width, height int }{
		{16, 64, 48},  // the usual case: upsampling chroma to a larger image
		{16, 16, 16},  // identity, where the reference short-circuits to a copy
		{16, 7, 5},    // downsampling, and dimensions coprime with the source
		{32, 129, 31}, // extreme aspect ratio, odd dimensions
		{8, 1, 1},     // degenerate single pixel
	}

	for _, tc := range sizes {
		img := synth(tc.width, tc.height)
		aPlane, bPlane := synthChroma(tc.srcSize)

		got := compose(img, aPlane, bPlane, tc.srcSize).(*image.RGBA)
		want := composeReference(img, aPlane, bPlane, tc.srcSize).(*image.RGBA)

		for y := range tc.height {
			for x := range tc.width {
				g := y*got.Stride + x*4
				w := y*want.Stride + x*4

				for c := range 4 {
					if got.Pix[g+c] != want.Pix[w+c] {
						t.Fatalf("%dx%d from %d, pixel (%d, %d) channel %d: got %d, want %d",
							tc.width, tc.height, tc.srcSize, x, y, c, got.Pix[g+c], want.Pix[w+c])
					}
				}
			}
		}
	}
}

// TestComposeMatchesReferenceForImageTypes covers the three source-buffer shapes compose() distinguishes: the
// premultiplied fast path, the straight-alpha fast path including partially transparent pixels (where the byte lookup
// must give way to the general conversion), and the generic At() fallback.
func TestComposeMatchesReferenceForImageTypes(t *testing.T) {
	const size, width, height = 16, 40, 24

	aPlane, bPlane := synthChroma(size)
	nrgba := synth(width, height)

	// Vary alpha so the un-premultiplying branch is exercised alongside the opaque one.
	translucent := image.NewNRGBA(image.Rect(0, 0, width, height))
	copy(translucent.Pix, nrgba.Pix)
	for y := range height {
		for x := range width {
			translucent.Pix[y*translucent.Stride+x*4+3] = uint8((x*11 + y*5) % 256)
		}
	}

	rgba := image.NewRGBA(image.Rect(0, 0, width, height))
	copy(rgba.Pix, nrgba.Pix)

	cases := map[string]image.Image{
		"nrgba":       nrgba,
		"translucent": translucent,
		"rgba":        rgba,
		"generic":     genericImage{src: nrgba},
	}

	for name, img := range cases {
		t.Run(name, func(t *testing.T) {
			got := compose(img, aPlane, bPlane, size).(*image.RGBA)
			want := composeReference(img, aPlane, bPlane, size).(*image.RGBA)

			for i := range want.Pix {
				if got.Pix[i] != want.Pix[i] {
					t.Fatalf("byte %d: got %d, want %d", i, got.Pix[i], want.Pix[i])
				}
			}
		})
	}
}
