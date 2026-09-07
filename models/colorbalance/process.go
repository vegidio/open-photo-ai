package colorbalance

import (
	"context"
	"image"
	"image/color"
	"math"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal/utils"
	ort "github.com/yalue/onnxruntime_go"
)

// plan is the geometry one Process run uses: what the image is resized to, and how much reflection padding brings
// that to the graph's square canvas.
type plan struct {
	scaledW, scaledH int
	padW, padH       int
}

// planCanvas works out that geometry from the image's size and the variant's Canvas. It is separate from Process, and
// pure, so the geometry is something a test can check rather than something a reader has to take on trust.
//
// A fixed-shape graph accepts exactly one size, so the longest side always lands on Canvas.Size - enlarging a small
// image as readily as shrinking a large one - and the short side is padded out to the square.
func planCanvas(fullW, fullH int, c Canvas) plan {
	sw, sh := utils.FitLongSide(fullW, fullH, c.Size)

	return plan{
		scaledW: sw, scaledH: sh,
		padW: c.Size - sw, padH: c.Size - sh,
	}
}

func Process(ctx context.Context, session *utils.Session, img image.Image, canvas Canvas) (image.Image, error) {
	bounds := img.Bounds()
	origW := bounds.Dx()
	origH := bounds.Dy()

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// The graph accepts exactly one size, so the longest side is resampled onto the canvas and the rest of the square
	// is reflection-padded. Padding rather than stretching, for the same reason the light adjustment family pads: the
	// pixels the model sees must not change just because the image needed more columns to fill the canvas.
	p := planCanvas(origW, origH, canvas)

	resized := imaging.Resize(img, p.scaledW, p.scaledH, imaging.Lanczos)

	var padded image.Image = resized
	if p.padW > 0 || p.padH > 0 {
		padded = utils.ReflectionPad(resized, 0, 0, p.padW, p.padH)
	}

	// Convert the padded image to CHW [0,1] float32
	inputData := utils.ImageToCHW(padded, false, false)

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	shape := ort.NewShape(1, 3, int64(canvas.Size), int64(canvas.Size))

	outputData, err := utils.RunUnary(session, inputData, shape, shape)
	if err != nil {
		return nil, err
	}

	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Fit a polynomial color mapping from low-res input -> low-res output, then apply that mapping to the
	// full-resolution original.
	//
	// The padding is dropped here rather than carried into the fit, and that is worth more than it looks. The padded
	// columns are a mirror of the image's own edge, so leaving them in re-weights the border content against the rest
	// of the photo - and this is a global fit, so a re-weighting moves every pixel of the result. Over 72 photos
	// (three sources, six aspect ratios, four illuminant casts), scoring the final full-resolution image against what
	// the dynamic-shape graph rendered, cropping first takes the median from 47.9 dB to 52.7 dB and the worst case
	// from 38.8 dB to 41.0 dB. It costs nothing: the pad is only ever on the right and bottom, so the wanted region is
	// already contiguous from the origin.
	srcLR := chwToHWC(inputData, canvas.Size, canvas.Size, p.scaledW, p.scaledH)
	dstLR := chwToHWC(outputData, canvas.Size, canvas.Size, p.scaledW, p.scaledH)
	w, err := fitPolynomialMapping(srcLR, dstLR)
	if err != nil {
		return nil, err
	}

	return applyMapping(img, w), nil
}

// chwToHWC unpacks the top-left cropW x cropH region of a [1, 3, canvasH, canvasW] CHW float32 tensor into a flat HWC
// slice of [3]float32.
//
// The crop is a parameter rather than a separate function because every caller wants it: the tensor is always the
// square the graph accepts, and the region of interest is always the un-padded image inside it.
func chwToHWC(data []float32, canvasW, canvasH, cropW, cropH int) [][3]float32 {
	plane := canvasW * canvasH
	out := make([][3]float32, 0, cropW*cropH)

	for y := range cropH {
		row := y * canvasW
		for x := range cropW {
			i := row + x
			out = append(out, [3]float32{data[i], data[plane+i], data[2*plane+i]})
		}
	}

	return out
}

// kernelP builds the 11-feature polynomial vector used by Deep_White_Balance.
// Order is significant: [r, g, b, r*g, r*b, g*b, r*r, g*g, b*b, r*g*b, 1]
func kernelP(r, g, b float32) [11]float32 {
	return [11]float32{r, g, b, r * g, r * b, g * b, r * r, g * g, b * b, r * g * b, 1}
}

// fitPolynomialMapping solves the 11x3 normal equations W = (X^T X)^-1 X^T Y
// where each row of X is kernelP(src[i]) and each row of Y is dst[i].
// A small ridge term is added to the diagonal to keep degenerate inputs stable.
func fitPolynomialMapping(src, dst [][3]float32) ([11][3]float32, error) {
	w, err := fitPolynomialMappings(src, dst)
	if err != nil {
		return [11][3]float32{}, err
	}

	return w[0], nil
}

// fitPolynomialMappings fits one mapping per destination from a shared source, solving them together. Every dst must
// have the same length as src.
//
// The multi-destination form is what the mixed-illuminant pipeline needs: it fits the same low-resolution source to
// two renderings at once. X^T X depends only on the source, so it is the same matrix for every destination, and the
// pass that builds it is the expensive half - O(N*121) over a few hundred thousand samples, against a fixed 11x11
// elimination afterwards. Accumulating it once rather than once per destination is therefore very nearly the whole
// saving, and it is why this is one function taking several destinations rather than a loop around the single one.
//
// The single-destination case is arithmetically unchanged by this: the accumulation order, the ridge, the pivoting
// and the elimination all run exactly as they did, so rio's fit is bit-for-bit what it was.
//
// BenchmarkFitPolynomialMappings measures the saving at the colour balance canvas: 32ms for two destinations together
// against 53ms for the same two fitted separately.
func fitPolynomialMappings(src [][3]float32, dsts ...[][3]float32) ([][11][3]float32, error) {
	if len(dsts) == 0 {
		return nil, errors.New("no destination to fit the colour balance mapping to")
	}

	for i, dst := range dsts {
		if len(dst) != len(src) {
			return nil, errors.Errorf("destination %d has %d samples but the source has %d", i, len(dst), len(src))
		}
	}

	// One block of three columns per destination, laid out end to end so a single elimination solves all of them.
	cols := 3 * len(dsts)

	var xtx [11][11]float64
	xty := make([][]float64, 11)
	for a := range 11 {
		xty[a] = make([]float64, cols)
	}

	for i := range src {
		k := kernelP(src[i][0], src[i][1], src[i][2])
		var k64 [11]float64
		for a := range 11 {
			k64[a] = float64(k[a])
		}
		for a := range 11 {
			ka := k64[a]
			for b := range 11 {
				xtx[a][b] += ka * k64[b]
			}
			for d, dst := range dsts {
				xty[a][3*d] += ka * float64(dst[i][0])
				xty[a][3*d+1] += ka * float64(dst[i][1])
				xty[a][3*d+2] += ka * float64(dst[i][2])
			}
		}
	}

	for a := range 11 {
		xtx[a][a] += 1e-8
	}

	// Augmented matrix [XtX | XtY] -> Gauss-Jordan elimination with partial pivoting.
	width := 11 + cols
	aug := make([][]float64, 11)
	for i := range 11 {
		aug[i] = make([]float64, width)
		for j := range 11 {
			aug[i][j] = xtx[i][j]
		}
		copy(aug[i][11:], xty[i])
	}

	for col := range 11 {
		pivot := col
		maxVal := math.Abs(aug[col][col])
		for r := col + 1; r < 11; r++ {
			if v := math.Abs(aug[r][col]); v > maxVal {
				maxVal = v
				pivot = r
			}
		}
		if pivot != col {
			aug[col], aug[pivot] = aug[pivot], aug[col]
		}

		// The ridge above makes XtX positive-definite, so after partial pivoting this should never be zero. Should is
		// not a guarantee in float64, and dividing by it would fill w with NaN - which applyMapping renders as a
		// destroyed image with nothing anywhere to say why. Fail loudly instead.
		piv := aug[col][col]
		if piv == 0 {
			return nil, errors.Errorf("singular normal equations at column %d; cannot fit the colour "+
				"balance mapping", col)
		}

		for j := col; j < width; j++ {
			aug[col][j] /= piv
		}
		for r := range 11 {
			if r == col {
				continue
			}
			f := aug[r][col]
			if f == 0 {
				continue
			}
			for j := col; j < width; j++ {
				aug[r][j] -= f * aug[col][j]
			}
		}
	}

	out := make([][11][3]float32, len(dsts))
	for d := range dsts {
		base := 11 + 3*d
		for i := range 11 {
			out[d][i][0] = float32(aug[i][base])
			out[d][i][1] = float32(aug[i][base+1])
			out[d][i][2] = float32(aug[i][base+2])
		}
	}

	return out, nil
}

// applyMapping renders a new full-resolution image by mapping each pixel of img
// through the fitted polynomial weights w.
func applyMapping(img image.Image, w [11][3]float32) image.Image {
	bounds := img.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()

	out := image.NewRGBA(image.Rect(0, 0, width, height))

	// Fast path: the polynomial is evaluated at full photo resolution, so the generic path spends an interface
	// dispatch and a color.Color boxing per pixel on top of the arithmetic. RgbPixBuffer offsets are already relative
	// to Bounds().Min, which is exactly the (Min.X+x, Min.Y+y) indexing the fallback uses, so a non-origin source
	// stays on the fast path. Sample16 reproduces At().RGBA() bit-for-bit, making both paths output-identical.
	pix, stride, fast := utils.RgbPixBuffer(img)
	_, isNRGBA := img.(*image.NRGBA)

	if fast {
		for y := range height {
			row := y * stride
			dst := y * out.Stride

			for x := range width {
				pr, pg, pb, _ := utils.Sample16(pix, row+x*4, isNRGBA)
				nr, ng, nb := mapPixel(pr, pg, pb, w)

				out.Pix[dst] = uint8(utils.Clamp255(nr * 255.0))
				out.Pix[dst+1] = uint8(utils.Clamp255(ng * 255.0))
				out.Pix[dst+2] = uint8(utils.Clamp255(nb * 255.0))
				out.Pix[dst+3] = 255
				dst += 4
			}
		}

		return out
	}

	for y := range height {
		for x := range width {
			pr, pg, pb, _ := img.At(bounds.Min.X+x, bounds.Min.Y+y).RGBA()
			nr, ng, nb := mapPixel(pr, pg, pb, w)

			out.Set(x, y, color.RGBA{
				R: uint8(utils.Clamp255(nr * 255.0)),
				G: uint8(utils.Clamp255(ng * 255.0)),
				B: uint8(utils.Clamp255(nb * 255.0)),
				A: 255,
			})
		}
	}

	return out
}

// mapPixel evaluates the fitted polynomial for one pixel, taking the 16-bit channel values that both the fast and the
// generic path produce so the two cannot drift apart.
func mapPixel(pr, pg, pb uint32, w [11][3]float32) (nr, ng, nb float32) {
	r := float32(pr) / 65535.0
	g := float32(pg) / 65535.0
	b := float32(pb) / 65535.0

	k := kernelP(r, g, b)
	for i := range 11 {
		nr += k[i] * w[i][0]
		ng += k[i] * w[i][1]
		nb += k[i] * w[i][2]
	}

	return nr, ng, nb
}
