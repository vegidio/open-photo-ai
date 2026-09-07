package colorbalance

import (
	"context"
	"image"
	"image/color"
	"runtime"
	"sync"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal/utils"
	ort "github.com/yalue/onnxruntime_go"
)

// ProcessMixed runs the mixed-illuminant pipeline.
//
// Where rio's Process corrects an image, this one blends several. The graph returns, at the canvas, a per-pixel weight
// map over the white-balance renderings plus the renderings themselves; this fits a polynomial colour mapping to each
// rendering and evaluates the weighted blend at full photo resolution.
//
// The division of labour between the graph and this function is the same one Process uses, and it falls where it does
// for the same reason. Everything at a fixed resolution is in the graph - the editing network that synthesizes the
// renderings, the weight predictor, and the predictor's internal multi-scale ensemble. What is left here is the part
// that runs at the photo's own size, which a fixed-shape graph cannot express: fitting the mappings, and applying
// them. The fit exists only to carry a canvas-resolution colour transform up to full resolution, which is why the
// graph has no need of it internally - at the canvas it simply has the renderings.
func ProcessMixed(
	ctx context.Context,
	session *utils.Session,
	img image.Image,
	canvas Canvas,
	spec *MixedSpec,
) (image.Image, error) {
	bounds := img.Bounds()

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	p := planCanvas(bounds.Dx(), bounds.Dy(), canvas)

	resized := imaging.Resize(img, p.scaledW, p.scaledH, imaging.Lanczos)

	var padded image.Image = resized
	if p.padW > 0 || p.padH > 0 {
		padded = utils.ReflectionPad(resized, 0, 0, p.padW, p.padH)
	}

	inputData := utils.ImageToCHW(padded, false, false)

	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	inShape := ort.NewShape(1, 3, int64(canvas.Size), int64(canvas.Size))
	outShape := ort.NewShape(1, int64(spec.Channels()), int64(canvas.Size), int64(canvas.Size))

	outputData, err := utils.RunUnary(session, inputData, inShape, outShape)
	if err != nil {
		return nil, err
	}

	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// The padding is dropped before the fit for the reason Process spells out: the padded columns mirror the image's
	// own edge, and this is a global fit, so leaving them in re-weights the border content against the rest of the
	// photo and moves every pixel of the result. The pad is only ever on the right and bottom, so the wanted region
	// is contiguous from the origin and the crop costs nothing.
	plane := canvas.Size * canvas.Size
	srcLR := chwToHWC(inputData, canvas.Size, canvas.Size, p.scaledW, p.scaledH)

	dsts := make([][][3]float32, spec.Renderings())
	for i := range dsts {
		dsts[i] = chwToHWC(outputData[spec.RenderingOffset(i)*plane:], canvas.Size, canvas.Size, p.scaledW, p.scaledH)
	}

	// One fit, not one per rendering: X^T X depends only on the source, and building it is the expensive half.
	maps, err := fitPolynomialMappings(srcLR, dsts...)
	if err != nil {
		return nil, err
	}

	return blendWeighted(img, maps, outputData[:spec.Settings*plane], canvas.Size, p.scaledW, p.scaledH), nil
}

// blendWeighted renders the final image in one fused pass over the photo.
//
// Per pixel it takes the source as the daylight rendering, evaluates the other renderings in place through their
// fitted mappings, and combines them with the weights sampled from the canvas-resolution map. The renderings are
// never materialised: at 12 MP each would be a ~144 MB float buffer, and there is nothing to do with them afterwards
// but multiply and add, which is what this loop already does.
//
// BenchmarkBlendWeighted and its reference measure what that is worth. On an M2 Max at 4000x3000, fusing the passes
// takes 1008ms and 672 MB down to 56ms and 48 MB - and the 48 MB is the output image, so the working set is now the
// result rather than three full-resolution intermediates plus it.
//
// The polynomial features are built once per pixel and reused across every mapping. That is most of the arithmetic -
// eleven terms including three multiplies for the cross terms - so evaluating it per rendering instead would very
// nearly double the cost of the loop for nothing.
//
// weights holds one plane per setting, in the graph's channel order, at canvasSize x canvasSize with only the top-left
// cropW x cropH region meaning anything. maps holds one mapping per rendering, in the same order minus daylight, so
// weights plane 0 multiplies the source and plane i+1 multiplies maps[i].
//
// Every multiply-add below rounds its product explicitly, and that is load-bearing rather than noise. Go permits an
// implementation to contract `x + y*z` into one fused operation with a single rounding, and whether it actually does
// is a property of the surrounding code rather than of the expression: the same arithmetic fuses when a compiler
// writes it out inline and does not when it sits inside this closure. The difference is one unit in the last place,
// which would be invisible if the result were not then truncated to 8 bits - where it falls on either side of an
// exact integer and changes the byte. An explicit conversion rounds, which forbids the contraction, and makes a photo
// render identically on every architecture and every compiler version instead of merely usually identically.
func blendWeighted(
	img image.Image,
	maps [][11][3]float32,
	weights []float32,
	canvasSize, cropW, cropH int,
) image.Image {
	bounds := img.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()

	out := image.NewRGBA(image.Rect(0, 0, width, height))

	// Neither axis mapping depends on the pixel data, so both are computed once rather than per pixel.
	//
	// Sampling aligns pixel centers rather than corners. The weight map is the un-padded region of a canvas built by
	// imaging.Resize, which is itself pixel-center aligned, so this is the geometrically consistent inverse of the
	// resize that produced it; align_corners would re-map corner to corner and shift by up to half a pixel against it.
	// Inside the graph the convention is the opposite, and has to be - there it must match how the checkpoint was
	// trained - but nothing downstream of the graph is trained, so the resize that made the canvas is what governs.
	x0s, x1s, fxs := sampleAxis(cropW, width)
	y0s, y1s, fys := sampleAxis(cropH, height)

	plane := canvasSize * canvasSize

	pix, stride, fast := utils.RgbPixBuffer(img)
	_, isNRGBA := img.(*image.NRGBA)

	rows := func(yStart, yEnd int) {
		// Scratch reused across the band; the weight count is fixed and small.
		w := make([]float32, len(maps)+1)

		for y := yStart; y < yEnd; y++ {
			y0, y1, fy := y0s[y], y1s[y], fys[y]

			rowTop := y0 * canvasSize
			rowBottom := y1 * canvasSize
			src := y * stride
			dst := y * out.Stride

			for x := range width {
				x0, x1, fx := x0s[x], x1s[x], fxs[x]

				// Bilinear interpolation is a convex combination of four samples, so it commutes with the sum over
				// channels: a map that is a partition of unity stays one, and the blend below needs no renormalising.
				for c := range w {
					base := c * plane
					top := float32(weights[base+rowTop+x0]*(1-fx)) + float32(weights[base+rowTop+x1]*fx)
					bottom := float32(weights[base+rowBottom+x0]*(1-fx)) + float32(weights[base+rowBottom+x1]*fx)
					w[c] = float32(top*(1-fy)) + float32(bottom*fy)
				}

				var pr, pg, pb uint32
				if fast {
					pr, pg, pb, _ = utils.Sample16(pix, src+x*4, isNRGBA)
				} else {
					pr, pg, pb, _ = img.At(bounds.Min.X+x, bounds.Min.Y+y).RGBA()
				}

				r := float32(pr) / 65535.0
				g := float32(pg) / 65535.0
				b := float32(pb) / 65535.0

				k := kernelP(r, g, b)

				// Daylight is the source itself.
				nr, ng, nb := w[0]*r, w[0]*g, w[0]*b

				for i, m := range maps {
					var mr, mg, mb float32
					for j := range 11 {
						mr += float32(k[j] * m[j][0])
						mg += float32(k[j] * m[j][1])
						mb += float32(k[j] * m[j][2])
					}

					// Clamp each rendering before it is weighted, not the blend afterwards. The upstream pipeline
					// materialises the renderings and clips them, and the eleven-term polynomial extrapolates well
					// outside [0,1] on saturated pixels - so clamping only at the end lets a blown highlight pull the
					// blend somewhere the reference never goes, by however much its own weight allows.
					weight := w[i+1]
					nr += float32(weight * clamp01(mr))
					ng += float32(weight * clamp01(mg))
					nb += float32(weight * clamp01(mb))
				}

				if fast {
					out.Pix[dst] = uint8(utils.Clamp255(nr * 255.0))
					out.Pix[dst+1] = uint8(utils.Clamp255(ng * 255.0))
					out.Pix[dst+2] = uint8(utils.Clamp255(nb * 255.0))
					out.Pix[dst+3] = 255
					dst += 4
					continue
				}

				out.Set(x, y, color.RGBA{
					R: uint8(utils.Clamp255(nr * 255.0)),
					G: uint8(utils.Clamp255(ng * 255.0)),
					B: uint8(utils.Clamp255(nb * 255.0)),
					A: 255,
				})
			}
		}
	}

	// The loop is strictly row-independent, so it splits across cores with no coordination beyond the join. It is
	// worth splitting where applyMapping is not: this evaluates the polynomial once per rendering per pixel on top of
	// the same per-pixel overheads.
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

// clamp01 clips a synthesized rendering back into gamut, matching the reference pipeline's outOfGamutClipping.
func clamp01(v float32) float32 {
	if v < 0 {
		return 0
	}
	if v > 1 {
		return 1
	}

	return v
}

// sampleAxis precomputes the bilinear source columns and weights for every destination column. Sampling aligns pixel
// centers and clamps at the edges.
//
// This is a copy of the colorization package's helper rather than a shared one in internal/utils. Promoting it would
// mean touching that package's reference test and benchmark, which pin it, for the sake of a model that is not
// changing; if a third caller ever appears, promote it then.
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
