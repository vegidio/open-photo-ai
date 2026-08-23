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

const maxSize = 656

func Process(ctx context.Context, session *utils.Session, img image.Image) (image.Image, error) {
	bounds := img.Bounds()
	origW := bounds.Dx()
	origH := bounds.Dy()

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Resize so neither side exceeds maxSize, padded to a multiple of 16. An image already inside the ceiling is only
	// aligned, never stretched up to it: the model has no detail to add, so the enlargement bought nothing and cost
	// inference time proportional to the area.
	newW, newH := utils.FitWithinMaxSize(origW, origH, maxSize)
	resized := imaging.Resize(img, newW, newH, imaging.Lanczos)

	// Convert resized image to CHW [0,1] float32
	inputData := utils.ImageToCHW(resized, false, false)

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	shape := ort.NewShape(1, 3, int64(newH), int64(newW))

	outputData, err := utils.RunUnary(session, inputData, shape, shape)
	if err != nil {
		return nil, err
	}

	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Fit a polynomial color mapping from low-res input -> low-res output, then
	// apply that mapping to the full-resolution original.
	srcLR := chwToHWC(inputData, newW, newH)
	dstLR := chwToHWC(outputData, newW, newH)
	w, err := fitPolynomialMapping(srcLR, dstLR)
	if err != nil {
		return nil, err
	}

	return applyMapping(img, w), nil
}

// chwToHWC unpacks a [1, 3, H, W] CHW float32 tensor into a flat HWC slice of [3]float32.
func chwToHWC(data []float32, width, height int) [][3]float32 {
	plane := width * height
	out := make([][3]float32, plane)
	for i := range plane {
		out[i] = [3]float32{data[i], data[plane+i], data[2*plane+i]}
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
	var xtx [11][11]float64
	var xty [11][3]float64

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
			xty[a][0] += ka * float64(dst[i][0])
			xty[a][1] += ka * float64(dst[i][1])
			xty[a][2] += ka * float64(dst[i][2])
		}
	}

	for a := range 11 {
		xtx[a][a] += 1e-8
	}

	// Augmented matrix [XtX | XtY] -> Gauss-Jordan elimination with partial pivoting.
	var aug [11][14]float64
	for i := range 11 {
		for j := range 11 {
			aug[i][j] = xtx[i][j]
		}
		aug[i][11] = xty[i][0]
		aug[i][12] = xty[i][1]
		aug[i][13] = xty[i][2]
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
			return [11][3]float32{}, errors.Errorf("singular normal equations at column %d; cannot fit the colour "+
				"balance mapping", col)
		}

		for j := col; j < 14; j++ {
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
			for j := col; j < 14; j++ {
				aug[r][j] -= f * aug[col][j]
			}
		}
	}

	var w [11][3]float32
	for i := range 11 {
		w[i][0] = float32(aug[i][11])
		w[i][1] = float32(aug[i][12])
		w[i][2] = float32(aug[i][13])
	}
	return w, nil
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
