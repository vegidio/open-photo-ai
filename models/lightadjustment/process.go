package lightadjustment

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

// eps guards the per-channel gain division against near-black input pixels.
const eps = 1e-3

// plan is the geometry one Process run uses: what the image is resized to, how much reflection padding brings that to
// the graph's canvas, and whether a resize happened at all - which is what decides the gain-map path.
type plan struct {
	scaledW, scaledH int
	padW, padH       int
	resize           bool
}

// planCanvas works out that geometry from the image's size and the variant's Canvas. It is separate from Process, and
// pure, so the claim that paris's geometry is unchanged is something a test can check rather than something a reader
// has to take on trust.
func planCanvas(fullW, fullH int, c Canvas) plan {
	if c.Square {
		// A fixed-shape graph accepts exactly one size, so the longest side always lands on MaxSize - enlarging a
		// small image as readily as shrinking a large one - and the short side is padded out to the square.
		sw, sh := fitLongSide(fullW, fullH, c.MaxSize)
		return plan{
			scaledW: sw, scaledH: sh,
			padW: c.MaxSize - sw, padH: c.MaxSize - sh,
			resize: true,
		}
	}

	scaledW, scaledH := fullW, fullH
	resize := max(fullW, fullH) > c.MaxSize
	if resize {
		scaledW, scaledH = utils.FitToMaxSize(fullW, fullH, c.MaxSize)
	}

	return plan{
		scaledW: scaledW, scaledH: scaledH,
		padW:   alignUp(scaledW, c.Align) - scaledW,
		padH:   alignUp(scaledH, c.Align) - scaledH,
		resize: resize,
	}
}

// fitLongSide scales (w, h) so the longer side is exactly size, preserving the aspect ratio.
//
// utils.FitToMaxSize is close but not usable here: it rounds both sides up to a multiple of 16, which for a square
// canvas would overshoot the exact dimension the fixed-shape graph demands.
func fitLongSide(w, h, size int) (int, int) {
	ratio := float64(size) / float64(max(w, h))
	return max(1, int(math.Round(float64(w)*ratio))), max(1, int(math.Round(float64(h)*ratio)))
}

// alignUp rounds v up to the next multiple of n. It lives here rather than in internal/utils because the shared
// RoundUpTo16 has its own pinned tests and callers, and this family is the only one that needs a second alignment.
func alignUp(v, n int) int {
	if n <= 1 || v%n == 0 {
		return v
	}
	return v + (n - v%n)
}

func Process(ctx context.Context, session *utils.Session, img image.Image, canvas Canvas) (image.Image, error) {
	bounds := img.Bounds()
	fullW := bounds.Dx()
	fullH := bounds.Dy()

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// The graph requires both sides to be a multiple of Canvas.Align (or exactly the square, for a fixed-shape
	// variant), and it is only run on an image whose longest side is at most Canvas.MaxSize. Those are two separate
	// requirements and they are met two different ways.
	//
	// Downscaling is a resample, because that is the point of it. Alignment is reflection padding, because it is not:
	// the pixels the model was going to see must not change just because the image needed eight more columns.
	//
	// Resizing to the aligned size did change them, and visibly. Measured on a 1000x750 photo through the real paris
	// graph, the old path left the last column differing from its neighbour by 40.8 levels on average against an
	// interior column-to-column gradient of 3.9 - a hard one-pixel line down the right edge of every image inside the
	// ceiling whose width was not already a multiple of 16, which is most of them. Padding brings that to 2.9, in line
	// with the interior. It is also far cheaper: the resize made the image differ from the original, which sent every
	// such image through buildResult's three further full-resolution passes for the sake of an eight-pixel adjustment.
	p := planCanvas(fullW, fullH, canvas)

	resized := img
	if p.resize {
		resized = imaging.Resize(img, p.scaledW, p.scaledH, imaging.Lanczos)
	}

	// Pad after any downscale, so the alignment is of what the model actually receives.
	padded := resized
	if p.padW > 0 || p.padH > 0 {
		padded = utils.ReflectionPad(resized, 0, 0, p.padW, p.padH)
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
	if p.padW > 0 || p.padH > 0 {
		outLR = imaging.Crop(outLR, image.Rect(0, 0, p.scaledW, p.scaledH))
	}

	// If we never downscaled, the output is already at full resolution and is the result. Padding alone does not
	// trigger the gain map - it did before, which is what made buildResult run for nearly every image. A Square
	// variant always resizes, so it always takes the gain-map path.
	if !p.resize {
		return outLR, nil
	}

	return buildResult(img, resized, outLR), nil
}

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
