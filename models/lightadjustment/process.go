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

	// The graph requires both sides to be a multiple of 16, and the model is only run on an image whose longest side
	// is at most maxSize. Those are two separate requirements and they are met two different ways.
	//
	// Downscaling is a resample, because that is the point of it. Alignment is reflection padding, because it is not:
	// the pixels the model was going to see must not change just because the image needed eight more columns.
	//
	// Resizing to the aligned size did change them, and visibly. Measured on a 1000x750 photo through the real paris
	// graph, the old path left the last column differing from its neighbour by 40.8 levels on average against an
	// interior column-to-column gradient of 3.9 - a hard one-pixel line down the right edge of every image inside the
	// ceiling whose width was not already a multiple of 16, which is most of them. Padding brings that to 2.9, in line
	// with the interior. It is also far cheaper: the resize made `resized != img`, which sent every such image through
	// buildResult's three further full-resolution passes for the sake of an eight-pixel adjustment.
	//
	// FitToMaxSize aligns its own result, so the downscale branch needs no padding; the pass-through branch is the
	// one padding exists for. FitToMaxSize is not used unconditionally because it also enlarges, and running a small
	// image at the ceiling costs inference time proportional to an area it never had.
	scaledW, scaledH := fullW, fullH

	resized := img
	if max(fullW, fullH) > maxSize {
		scaledW, scaledH = utils.FitToMaxSize(fullW, fullH, maxSize)
		resized = imaging.Resize(img, scaledW, scaledH, imaging.Lanczos)
	}

	// Pad after any downscale, so the alignment is of what the model actually receives.
	padded := resized
	padW, padH := utils.RoundUpTo16(scaledW)-scaledW, utils.RoundUpTo16(scaledH)-scaledH
	if padW > 0 || padH > 0 {
		padded = utils.ReflectionPad(resized, 0, 0, padW, padH)
	}

	rb := padded.Bounds()
	rW, rH := rb.Dx(), rb.Dy()

	// Convert the padded image to CHW [0,1] float32
	inputData := utils.ImageToCHW(padded, false, false)

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

	// Drop the alignment padding again: everything downstream works in the scaled image's dimensions.
	if padW > 0 || padH > 0 {
		outLR = imaging.Crop(outLR, image.Rect(0, 0, scaledW, scaledH))
	}

	// If we never downscaled, the output is already at full resolution and is the result. Padding alone does not
	// trigger the gain map - it did before, which is what made buildResult run for nearly every image.
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
