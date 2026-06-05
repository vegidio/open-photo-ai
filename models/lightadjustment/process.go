package lightadjustment

import (
	"context"
	"image"
	"image/color"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal/utils"
	ort "github.com/yalue/onnxruntime_go"
)

// maxSize caps the longest side fed to the model. Light adjustment is a low-frequency tonal effect, so inference at a
// reduced resolution is visually faithful while keeping the conv activations small enough to fit in memory.
//
// The full-resolution detail is preserved by applying the result as a gain map (see buildResult). Running the whole
// image natively could request multi-GB buffers and fail to allocate on constrained machines.
const maxSize = 1024

// eps guards the per-channel gain division against near-black input pixels.
const eps = 1e-3

func Process(ctx context.Context, session *ort.DynamicAdvancedSession, img image.Image) (image.Image, error) {
	bounds := img.Bounds()
	fullW := bounds.Dx()
	fullH := bounds.Dy()

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Resize so the longest side is at most maxSize (padded to a multiple of 16);
	// if the image already fits, run it natively.
	resized := img
	if fullW > maxSize || fullH > maxSize {
		newW, newH := utils.FitToMaxSize(fullW, fullH, maxSize)
		resized = imaging.Resize(img, newW, newH, imaging.Lanczos)
	}

	rb := resized.Bounds()
	rW, rH := rb.Dx(), rb.Dy()

	// Convert resized image to CHW [0,1] float32
	inputData := utils.ImageToCHW(resized, false, false)

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Create input tensor with dynamic shape
	inputShape := ort.NewShape(1, 3, int64(rH), int64(rW))
	inputTensor, err := ort.NewTensor(inputShape, inputData)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create input tensor")
	}
	defer inputTensor.Destroy()

	// Create output tensor
	outputShape := ort.NewShape(1, 3, int64(rH), int64(rW))
	outputTensor, err := ort.NewEmptyTensor[float32](outputShape)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create output tensor")
	}
	defer outputTensor.Destroy()

	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Run inference on the (down)scaled image
	if err = session.Run([]ort.Value{inputTensor}, []ort.Value{outputTensor}); err != nil {
		return nil, errors.Wrap(err, "failed to run inference")
	}

	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Convert the low-res model output back to an image
	outLR := utils.CHWToImage(outputTensor.GetData(), rW, rH, false)

	// If we never downscaled, the output already matches the full resolution.
	if resized == img {
		return outLR, nil
	}

	return buildResult(img, resized, outLR), nil
}

// buildResult applies the low-res relighting as a per-pixel multiplicative gain map on the full-resolution original.
// Both the smoothed input the model saw (inUp) and its relit output (outUp) are upsampled to full size; the ratio
// outUp/inUp is the low-frequency gain, applied to the full-res detail in img.
func buildResult(img, resized, outLR image.Image) image.Image {
	bounds := img.Bounds()
	fullW, fullH := bounds.Dx(), bounds.Dy()

	inUp := imaging.Resize(resized, fullW, fullH, imaging.Lanczos)
	outUp := imaging.Resize(outLR, fullW, fullH, imaging.Lanczos)

	out := image.NewRGBA(image.Rect(0, 0, fullW, fullH))

	for y := 0; y < fullH; y++ {
		for x := 0; x < fullW; x++ {
			fr, fg, fb, _ := img.At(x, y).RGBA()
			ir, ig, ib, _ := inUp.At(x, y).RGBA()
			or, og, ob, _ := outUp.At(x, y).RGBA()

			r := applyGain(float32(fr), float32(ir), float32(or))
			g := applyGain(float32(fg), float32(ig), float32(og))
			b := applyGain(float32(fb), float32(ib), float32(ob))

			out.Set(x, y, color.RGBA{R: uint8(r), G: uint8(g), B: uint8(b), A: 255})
		}
	}

	return out
}

// applyGain returns full * (out/in) on the [0,255] scale, clamped. The 16-bit channel values from RGBA() cancel in the
// out/in ratio, so only full needs rescaling to [0,255].
func applyGain(full, in, out float32) float32 {
	gain := out / (in + eps*65535.0)
	return utils.Clamp255(full / 257.0 * gain)
}
