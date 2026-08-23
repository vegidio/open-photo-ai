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

func Process(ctx context.Context, session *utils.Session, img image.Image) (image.Image, error) {
	bounds := img.Bounds()
	fullW := bounds.Dx()
	fullH := bounds.Dy()

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Resize so neither side exceeds maxSize, padded to a multiple of 16. The alignment applies to an image that
	// already fits too - it is a requirement of the graph, not a side effect of shrinking - so this runs on both
	// branches rather than only on the one that downsamples. Anything already aligned and inside the ceiling is
	// resized to its own dimensions, which imaging returns unchanged.
	newW, newH := utils.FitWithinMaxSize(fullW, fullH, maxSize)

	resized := img
	if newW != fullW || newH != fullH {
		resized = imaging.Resize(img, newW, newH, imaging.Lanczos)
	}

	rb := resized.Bounds()
	rW, rH := rb.Dx(), rb.Dy()

	// Convert resized image to CHW [0,1] float32
	inputData := utils.ImageToCHW(resized, false, false)

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	shape := ort.NewShape(1, 3, int64(rH), int64(rW))

	outputData, err := utils.RunUnary(session, inputData, shape, shape)
	if err != nil {
		return nil, err
	}

	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	outLR := utils.CHWToImage(outputData, rW, rH, false)

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

	// imaging.Resize always returns an origin-based *image.NRGBA, so both upsampled sources are already on the fast
	// path; only img's concrete type is unknown.
	inUp := imaging.Resize(resized, fullW, fullH, imaging.Lanczos)
	outUp := imaging.Resize(outLR, fullW, fullH, imaging.Lanczos)

	out := image.NewRGBA(image.Rect(0, 0, fullW, fullH))

	// Fast path: this loop runs at full photo resolution over three sources, so the generic path costs four interface
	// dispatches per pixel (~96M on a 24MP image). Sample16 reproduces the exact 16-bit values At().RGBA() would
	// return, so the direct-Pix path is bit-identical to the fallback below.
	//
	// The generic path indexes img with absolute coordinates starting at 0, so it only agrees with a Pix-relative
	// fast path when img sits at the origin. Anything else falls through to At().
	fPix, fStride, fFast := utils.RgbPixBuffer(img)
	_, fIsNRGBA := img.(*image.NRGBA)

	if fFast && bounds.Min == (image.Point{}) {
		for y := range fullH {
			fRow := y * fStride
			iRow := y * inUp.Stride
			oRow := y * outUp.Stride
			dst := y * out.Stride

			for x := range fullW {
				off := x * 4
				fr, fg, fb, _ := utils.Sample16(fPix, fRow+off, fIsNRGBA)
				ir, ig, ib, _ := utils.Sample16(inUp.Pix, iRow+off, true)
				or, og, ob, _ := utils.Sample16(outUp.Pix, oRow+off, true)

				out.Pix[dst] = uint8(applyGain(float32(fr), float32(ir), float32(or)))
				out.Pix[dst+1] = uint8(applyGain(float32(fg), float32(ig), float32(og)))
				out.Pix[dst+2] = uint8(applyGain(float32(fb), float32(ib), float32(ob)))
				out.Pix[dst+3] = 255
				dst += 4
			}
		}

		return out
	}

	for y := range fullH {
		for x := range fullW {
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
