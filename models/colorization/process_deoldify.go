package colorization

import (
	"context"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal/utils"
	ort "github.com/yalue/onnxruntime_go"
)

// deoldifySize is the fixed spatial size the DeOldify-based graphs are exported at: the reference stable colorizer's
// default render_factor (35) times its render base (16). Like the DDColor pipeline, only chroma comes from the model,
// so this does not cap output detail.
const deoldifySize = 560

// ProcessDeOldify colorizes img with a DeOldify-style session: the graph takes the image's ITU-601 luma rendered as
// gray RGB (CHW, [0,1], 560x560 — ImageNet normalization is baked into the exported graph) and returns a full RGB
// colorization at the same size. The result keeps the original image's full-resolution luminance and takes only the
// model's chroma, upsampled — the reference implementation does this transfer in YUV; here it is done in Lab to reuse
// the category's tested conversion and composition helpers, which is perceptually equivalent.
func ProcessDeOldify(ctx context.Context, session *utils.Session, img image.Image) (image.Image, error) {
	bounds := img.Bounds()
	origW := bounds.Dx()
	origH := bounds.Dy()

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	origL := lPlane(img)

	// The reference stretches to a square with bilinear resampling; Linear is imaging's equivalent.
	resized := imaging.Resize(img, deoldifySize, deoldifySize, imaging.Linear)
	inputData := grayLumaInput(resized)

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	inputShape := ort.NewShape(1, 3, deoldifySize, deoldifySize)
	inputTensor, err := ort.NewTensor(inputShape, inputData)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create input tensor")
	}
	defer inputTensor.Destroy()

	outputShape := ort.NewShape(1, 3, deoldifySize, deoldifySize)
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

	aPlane, bPlane := abFromRgb(outputTensor.GetData(), deoldifySize, deoldifySize)
	aPlane = resizePlane(aPlane, deoldifySize, deoldifySize, origW, origH)
	bPlane = resizePlane(bPlane, deoldifySize, deoldifySize, origW, origH)

	return composeLab(origL, aPlane, bPlane, origW, origH), nil
}

// grayLumaInput builds the model's CHW input tensor: each pixel reduced to its ITU-601 luma (the grayscale DeOldify
// was trained on — PIL's 'LA' conversion), replicated across the three channels, in [0, 1].
func grayLumaInput(img *image.NRGBA) []float32 {
	plane := deoldifySize * deoldifySize
	data := make([]float32, 3*plane)

	for y := range deoldifySize {
		row := y * img.Stride
		dst := y * deoldifySize

		for x := range deoldifySize {
			pr, pg, pb, _ := utils.Sample16(img.Pix, row+x*4, true)
			luma := (0.299*float32(pr) + 0.587*float32(pg) + 0.114*float32(pb)) / 65535.0

			i := dst + x
			data[i] = luma
			data[plane+i] = luma
			data[2*plane+i] = luma
		}
	}

	return data
}

// abFromRgb extracts the Lab a/b planes from a CHW RGB tensor in [0, 1].
func abFromRgb(data []float32, width, height int) (aPlane, bPlane []float32) {
	plane := width * height
	aPlane = make([]float32, plane)
	bPlane = make([]float32, plane)

	for i := 0; i < plane; i++ {
		_, a, b := utils.RgbToLab(clamp01(data[i]), clamp01(data[plane+i]), clamp01(data[2*plane+i]))
		aPlane[i] = a
		bPlane[i] = b
	}

	return aPlane, bPlane
}

func clamp01(v float32) float32 {
	if v < 0 {
		return 0
	}
	if v > 1 {
		return 1
	}
	return v
}
