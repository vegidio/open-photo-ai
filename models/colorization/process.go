package colorization

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal/utils"
	ort "github.com/yalue/onnxruntime_go"
)

// inputSize is the fixed spatial size the colorization graphs are exported at. The model only predicts the ab chroma
// planes at this resolution; the output keeps the original image's full-resolution luminance, so this is not a cap on
// output detail.
const inputSize = 512

// Process colorizes img with a DDColor-style session: the graph takes a gray RGB rendering of the image's luminance
// (CHW, [0,1], 512x512) and returns the predicted Lab ab planes at the same size. The result is composed from the
// original-resolution L channel plus the upsampled ab planes, so luminance detail is preserved exactly.
func Process(ctx context.Context, session *utils.Session, img image.Image) (image.Image, error) {
	bounds := img.Bounds()
	origW := bounds.Dx()
	origH := bounds.Dy()

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	origL := lPlane(img)

	resized := imaging.Resize(img, inputSize, inputSize, imaging.Lanczos)
	inputData := grayLabInput(resized)

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	inputShape := ort.NewShape(1, 3, inputSize, inputSize)
	inputTensor, err := ort.NewTensor(inputShape, inputData)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create input tensor")
	}
	defer inputTensor.Destroy()

	outputShape := ort.NewShape(1, 2, inputSize, inputSize)
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
	plane := inputSize * inputSize
	aPlane := resizePlane(outputData[:plane], inputSize, inputSize, origW, origH)
	bPlane := resizePlane(outputData[plane:2*plane], inputSize, inputSize, origW, origH)

	return composeLab(origL, aPlane, bPlane, origW, origH), nil
}

// lPlane extracts the CIELab L channel of every pixel at full resolution.
func lPlane(img image.Image) []float32 {
	bounds := img.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()

	out := make([]float32, width*height)

	// Fast path: this runs at full photo resolution, so skipping the At() dispatch and color.Color boxing per pixel
	// matters. Sample16 reproduces At().RGBA() bit-for-bit, so both paths are output-identical.
	pix, stride, fast := utils.RgbPixBuffer(img)
	_, isNRGBA := img.(*image.NRGBA)

	if fast {
		for y := range height {
			row := y * stride
			dst := y * width

			for x := range width {
				pr, pg, pb, _ := utils.Sample16(pix, row+x*4, isNRGBA)
				l, _, _ := utils.RgbToLab(float32(pr)/65535.0, float32(pg)/65535.0, float32(pb)/65535.0)
				out[dst+x] = l
			}
		}

		return out
	}

	for y := range height {
		for x := range width {
			pr, pg, pb, _ := img.At(bounds.Min.X+x, bounds.Min.Y+y).RGBA()
			l, _, _ := utils.RgbToLab(float32(pr)/65535.0, float32(pg)/65535.0, float32(pb)/65535.0)
			out[y*width+x] = l
		}
	}

	return out
}

// grayLabInput builds the model's CHW input tensor from the resized image: each pixel is reduced to its luminance
// (Lab L with zero chroma) and rendered back to RGB, which is the gray image DDColor was trained on.
func grayLabInput(img *image.NRGBA) []float32 {
	plane := inputSize * inputSize
	data := make([]float32, 3*plane)

	for y := range inputSize {
		row := y * img.Stride
		dst := y * inputSize

		for x := range inputSize {
			off := row + x*4
			pr, pg, pb, _ := utils.Sample16(img.Pix, off, true)

			l, _, _ := utils.RgbToLab(float32(pr)/65535.0, float32(pg)/65535.0, float32(pb)/65535.0)
			r, g, b := utils.LabToRgb(l, 0, 0)

			i := dst + x
			data[i] = r
			data[plane+i] = g
			data[2*plane+i] = b
		}
	}

	return data
}

// resizePlane bilinearly resizes a single float32 plane. The imaging package only resizes 8-bit images, and the ab
// planes must stay in float Lab units until they are combined with the luminance, so this is done by hand. Sampling
// aligns pixel centers (the cv2.INTER_LINEAR convention) and clamps at the edges.
func resizePlane(src []float32, srcW, srcH, dstW, dstH int) []float32 {
	if srcW == dstW && srcH == dstH {
		out := make([]float32, len(src))
		copy(out, src)
		return out
	}

	out := make([]float32, dstW*dstH)
	scaleX := float64(srcW) / float64(dstW)
	scaleY := float64(srcH) / float64(dstH)

	for y := range dstH {
		sy := (float64(y)+0.5)*scaleY - 0.5
		if sy < 0 {
			sy = 0
		}
		y0 := int(sy)
		if y0 > srcH-1 {
			y0 = srcH - 1
		}
		y1 := y0 + 1
		if y1 > srcH-1 {
			y1 = srcH - 1
		}
		fy := float32(sy - float64(y0))

		for x := range dstW {
			sx := (float64(x)+0.5)*scaleX - 0.5
			if sx < 0 {
				sx = 0
			}
			x0 := int(sx)
			if x0 > srcW-1 {
				x0 = srcW - 1
			}
			x1 := x0 + 1
			if x1 > srcW-1 {
				x1 = srcW - 1
			}
			fx := float32(sx - float64(x0))

			top := src[y0*srcW+x0]*(1-fx) + src[y0*srcW+x1]*fx
			bottom := src[y1*srcW+x0]*(1-fx) + src[y1*srcW+x1]*fx
			out[y*dstW+x] = top*(1-fy) + bottom*fy
		}
	}

	return out
}

// composeLab renders the final image from the original luminance plane and the upsampled chroma planes.
func composeLab(lPlane, aPlane, bPlane []float32, width, height int) image.Image {
	out := image.NewRGBA(image.Rect(0, 0, width, height))

	for y := range height {
		dst := y * out.Stride
		row := y * width

		for x := range width {
			i := row + x
			r, g, b := utils.LabToRgb(lPlane[i], aPlane[i], bPlane[i])

			// Round rather than truncate, matching the reference pipeline's np.round(): truncation would also turn
			// the conversion's ~1e-5 neutral-axis error into visible one-count channel splits on gray pixels.
			out.Pix[dst] = uint8(utils.Clamp255(r*255.0 + 0.5))
			out.Pix[dst+1] = uint8(utils.Clamp255(g*255.0 + 0.5))
			out.Pix[dst+2] = uint8(utils.Clamp255(b*255.0 + 0.5))
			out.Pix[dst+3] = 255
			dst += 4
		}
	}

	return out
}
