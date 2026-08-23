package colorization

import (
	"context"
	"image"
	"runtime"
	"sync"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal/utils"
	ort "github.com/yalue/onnxruntime_go"
)

// Spec is everything that differs between the colorization graphs. Both families reduce to the same shape - render
// the image square and gray, run it, take chroma from the result and compose it onto the original luminance - so the
// pipeline itself is written once in Process() and the differences are stated as data in specs.go rather than buried
// in a second copy.
type Spec struct {
	// Size is the square resolution the graph is exported at.
	Size int

	// Filter resamples the image up to that size. The two families were trained against different resamplers and are
	// sensitive to the difference.
	Filter imaging.ResampleFilter

	// BuildInput renders the resized image into the graph's CHW input tensor.
	BuildInput func(img *image.NRGBA, size int) []float32

	// OutChannels is the channel count of the output tensor: DDColor emits ab, DeOldify emits RGB.
	OutChannels int

	// Chroma pulls the Lab a/b planes, at model resolution, out of the raw output tensor.
	Chroma func(data []float32, size int) (a, b []float32)
}

// Process colorizes img with session, which must be a graph matching sp.
//
// Every colorization family shares this one pipeline: the image is rendered square and gray the way the graph was
// trained, run, and the predicted chroma is composed onto the original image's full-resolution luminance. Only chroma
// comes from the model, so the graph's fixed input size is not a cap on output detail.
func Process(ctx context.Context, session *utils.Session, img image.Image, sp Spec) (image.Image, error) {
	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	resized := imaging.Resize(img, sp.Size, sp.Size, sp.Filter)
	inputData := sp.BuildInput(resized, sp.Size)

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	inputShape := ort.NewShape(1, 3, int64(sp.Size), int64(sp.Size))
	outputShape := ort.NewShape(1, int64(sp.OutChannels), int64(sp.Size), int64(sp.Size))

	outputData, err := utils.RunUnary(session, inputData, inputShape, outputShape)
	if err != nil {
		return nil, err
	}

	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	aPlane, bPlane := sp.Chroma(outputData, sp.Size)

	return compose(img, aPlane, bPlane, sp.Size), nil
}

// compose renders the final image directly from the source pixels and the model-resolution chroma planes.
//
// This deliberately fuses what would otherwise be four full-resolution passes - extract luminance, upsample a,
// upsample b, combine - into one. The intermediate planes it avoids are float32 at full photo resolution, so on a
// 12 MP image they would total ~144 MB of heap that is written once, read once and thrown away. Fusing also keeps
// each source pixel in cache for the whole of its own computation.
//
// The bilinear weights, the clamping and the arithmetic order are exactly those of the separate resize-then-compose
// path, which survives as the reference implementation the tests check this against.
func compose(img image.Image, aPlane, bPlane []float32, srcSize int) image.Image {
	bounds := img.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()

	out := image.NewRGBA(image.Rect(0, 0, width, height))

	// Neither mapping depends on the pixel data, so both are computed once here instead of per pixel.
	x0s, x1s, fxs := sampleAxis(srcSize, width)
	y0s, y1s, fys := sampleAxis(srcSize, height)

	pix, stride, fast := utils.RgbPixBuffer(img)
	_, isNRGBA := img.(*image.NRGBA)

	rows := func(yStart, yEnd int) {
		for y := yStart; y < yEnd; y++ {
			y0, y1, fy := y0s[y], y1s[y], fys[y]

			rowTop := y0 * srcSize
			rowBottom := y1 * srcSize
			src := y * stride
			dst := y * out.Stride

			for x := range width {
				x0, x1, fx := x0s[x], x1s[x], fxs[x]

				aTop := aPlane[rowTop+x0]*(1-fx) + aPlane[rowTop+x1]*fx
				aBottom := aPlane[rowBottom+x0]*(1-fx) + aPlane[rowBottom+x1]*fx
				bTop := bPlane[rowTop+x0]*(1-fx) + bPlane[rowTop+x1]*fx
				bBottom := bPlane[rowBottom+x0]*(1-fx) + bPlane[rowBottom+x1]*fx

				var l float32
				if fast {
					off := src + x*4
					// Sample16 leaves the channel bytes untouched unless it has to un-premultiply, so the table-driven
					// conversion is exact for everything but a partially transparent straight-alpha pixel.
					if !isNRGBA || pix[off+3] == 0xff {
						l = utils.RgbToLabLBytes(pix[off], pix[off+1], pix[off+2])
					} else {
						pr, pg, pb, _ := utils.Sample16(pix, off, true)
						l = utils.RgbToLabL(float32(pr)/65535.0, float32(pg)/65535.0, float32(pb)/65535.0)
					}
				} else {
					pr, pg, pb, _ := img.At(bounds.Min.X+x, bounds.Min.Y+y).RGBA()
					l = utils.RgbToLabL(float32(pr)/65535.0, float32(pg)/65535.0, float32(pb)/65535.0)
				}

				r, g, b := utils.LabToLinearRgb(l, aTop*(1-fy)+aBottom*fy, bTop*(1-fy)+bBottom*fy)

				// Round rather than truncate, matching the reference pipeline's np.round(): truncation would also turn
				// the conversion's ~1e-5 neutral-axis error into visible one-count channel splits on gray pixels.
				out.Pix[dst] = utils.SrgbByte(r)
				out.Pix[dst+1] = utils.SrgbByte(g)
				out.Pix[dst+2] = utils.SrgbByte(b)
				out.Pix[dst+3] = 255
				dst += 4
			}
		}
	}

	// The loop is strictly row-independent, so it splits across cores with no coordination beyond the join.
	bands := min(runtime.NumCPU(), height)
	if bands <= 1 {
		rows(0, height)
		return out
	}

	var wg sync.WaitGroup
	band := (height + bands - 1) / bands

	for start := 0; start < height; start += band {
		wg.Add(1)

		go func(yStart int) {
			defer wg.Done()
			rows(yStart, min(yStart+band, height))
		}(start)
	}

	wg.Wait()

	return out
}

// sampleAxis precomputes the bilinear source columns and weights for every destination column. Sampling aligns pixel
// centers (the cv2.INTER_LINEAR convention) and clamps at the edges.
func sampleAxis(srcLen, dstLen int) (lo, hi []int, frac []float32) {
	lo = make([]int, dstLen)
	hi = make([]int, dstLen)
	frac = make([]float32, dstLen)

	scale := float64(srcLen) / float64(dstLen)

	for i := range dstLen {
		s := (float64(i)+0.5)*scale - 0.5
		if s < 0 {
			s = 0
		}

		i0 := min(int(s), srcLen-1)
		i1 := min(i0+1, srcLen-1)

		lo[i] = i0
		hi[i] = i1
		frac[i] = float32(s - float64(i0))
	}

	return lo, hi, frac
}
