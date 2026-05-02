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

func Process(ctx context.Context, session *ort.DynamicAdvancedSession, img image.Image) (image.Image, error) {
	bounds := img.Bounds()
	origW := bounds.Dx()
	origH := bounds.Dy()

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Resize so the longest side equals maxSize, padded to a multiple of 16
	newW, newH := targetSize(origW, origH)
	resized := imaging.Resize(img, newW, newH, imaging.Lanczos)

	// Convert resized image to CHW [0,1] float32
	inputData := utils.ImageToCHW(resized, false, false)

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	inputShape := ort.NewShape(1, 3, int64(newH), int64(newW))
	inputTensor, err := ort.NewTensor(inputShape, inputData)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create input tensor")
	}
	defer inputTensor.Destroy()

	outputShape := ort.NewShape(1, 3, int64(newH), int64(newW))
	outputTensor, err := ort.NewEmptyTensor[float32](outputShape)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create output tensor")
	}
	defer outputTensor.Destroy()

	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	err = session.Run([]ort.Value{inputTensor}, []ort.Value{outputTensor})
	if err != nil {
		return nil, errors.Wrap(err, "failed to run inference")
	}

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	outputData := outputTensor.GetData()

	// Fit a polynomial color mapping from low-res input -> low-res output, then
	// apply that mapping to the full-resolution original.
	srcLR := chwToHWC(inputData, newW, newH)
	dstLR := chwToHWC(outputData, newW, newH)
	w := fitPolynomialMapping(srcLR, dstLR)

	return applyMapping(img, w), nil
}

// targetSize returns (newW, newH) such that the longest side equals maxSize
// and both dimensions are rounded up to the next multiple of 16.
func targetSize(w, h int) (int, int) {
	longest := w
	if h > longest {
		longest = h
	}
	ratio := float64(maxSize) / float64(longest)
	nw := int(math.Round(float64(w) * ratio))
	nh := int(math.Round(float64(h) * ratio))
	return roundUpTo16(nw), roundUpTo16(nh)
}

func roundUpTo16(v int) int {
	if v%16 == 0 {
		return v
	}
	return v + (16 - v%16)
}

// chwToHWC unpacks a [1, 3, H, W] CHW float32 tensor into a flat HWC slice of [3]float32.
func chwToHWC(data []float32, width, height int) [][3]float32 {
	plane := width * height
	out := make([][3]float32, plane)
	for i := 0; i < plane; i++ {
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
func fitPolynomialMapping(src, dst [][3]float32) [11][3]float32 {
	var xtx [11][11]float64
	var xty [11][3]float64

	for i := range src {
		k := kernelP(src[i][0], src[i][1], src[i][2])
		var k64 [11]float64
		for a := 0; a < 11; a++ {
			k64[a] = float64(k[a])
		}
		for a := 0; a < 11; a++ {
			ka := k64[a]
			for b := 0; b < 11; b++ {
				xtx[a][b] += ka * k64[b]
			}
			xty[a][0] += ka * float64(dst[i][0])
			xty[a][1] += ka * float64(dst[i][1])
			xty[a][2] += ka * float64(dst[i][2])
		}
	}

	for a := 0; a < 11; a++ {
		xtx[a][a] += 1e-8
	}

	// Augmented matrix [XtX | XtY] -> Gauss-Jordan elimination with partial pivoting.
	var aug [11][14]float64
	for i := 0; i < 11; i++ {
		for j := 0; j < 11; j++ {
			aug[i][j] = xtx[i][j]
		}
		aug[i][11] = xty[i][0]
		aug[i][12] = xty[i][1]
		aug[i][13] = xty[i][2]
	}

	for col := 0; col < 11; col++ {
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

		piv := aug[col][col]
		for j := col; j < 14; j++ {
			aug[col][j] /= piv
		}
		for r := 0; r < 11; r++ {
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
	for i := 0; i < 11; i++ {
		w[i][0] = float32(aug[i][11])
		w[i][1] = float32(aug[i][12])
		w[i][2] = float32(aug[i][13])
	}
	return w
}

// applyMapping renders a new full-resolution image by mapping each pixel of img
// through the fitted polynomial weights w.
func applyMapping(img image.Image, w [11][3]float32) image.Image {
	bounds := img.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()

	out := image.NewRGBA(image.Rect(0, 0, width, height))

	for y := 0; y < height; y++ {
		for x := 0; x < width; x++ {
			pr, pg, pb, _ := img.At(bounds.Min.X+x, bounds.Min.Y+y).RGBA()
			r := float32(pr) / 65535.0
			g := float32(pg) / 65535.0
			b := float32(pb) / 65535.0

			k := kernelP(r, g, b)
			var nr, ng, nb float32
			for i := 0; i < 11; i++ {
				nr += k[i] * w[i][0]
				ng += k[i] * w[i][1]
				nb += k[i] * w[i][2]
			}

			out.Set(x, y, color.RGBA{
				R: uint8(utils.ClampFloat32(nr * 255.0)),
				G: uint8(utils.ClampFloat32(ng * 255.0)),
				B: uint8(utils.ClampFloat32(nb * 255.0)),
				A: 255,
			})
		}
	}

	return out
}
