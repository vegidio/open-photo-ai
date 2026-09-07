package colorbalance

import (
	"image"
	"image/color"

	"github.com/vegidio/open-photo-ai/internal/utils"
)

// referenceBlend is the pre-fusion implementation of blendWeighted, kept so the fused one has something to be
// checked against. It does the same arithmetic in the same order, but the obvious way: it materialises every
// rendering at full resolution first, upsamples each weight plane into its own full-resolution buffer, and only then
// combines them, in a single-threaded loop with no precomputed sampling tables.
//
// It is deliberately slow and allocation-heavy - that is the whole point of the fused version - so it lives in a test
// file and is never built into the binary. Anything the two disagree on is a bug in the fused path, because fusing
// passes is a performance change and nothing else.
func referenceBlend(
	img image.Image,
	maps [][11][3]float32,
	weights []float32,
	canvasSize, cropW, cropH int,
) image.Image {
	bounds := img.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()
	plane := canvasSize * canvasSize

	// Every rendering, materialised at full resolution and clipped back into gamut.
	renderings := make([][]float32, len(maps))
	for i, m := range maps {
		buf := make([]float32, width*height*3)
		for y := range height {
			for x := range width {
				pr, pg, pb, _ := img.At(bounds.Min.X+x, bounds.Min.Y+y).RGBA()
				k := kernelP(float32(pr)/65535.0, float32(pg)/65535.0, float32(pb)/65535.0)

				var mr, mg, mb float32
				// Rounded explicitly for the reason blendWeighted documents: the two must not differ by whether
				// the compiler happened to contract a multiply-add here and not there.
				for j := range 11 {
					mr += float32(k[j] * m[j][0])
					mg += float32(k[j] * m[j][1])
					mb += float32(k[j] * m[j][2])
				}

				o := (y*width + x) * 3
				buf[o] = clamp01(mr)
				buf[o+1] = clamp01(mg)
				buf[o+2] = clamp01(mb)
			}
		}
		renderings[i] = buf
	}

	// Every weight plane, upsampled to full resolution on its own.
	upsampled := make([][]float32, len(maps)+1)
	for c := range upsampled {
		upsampled[c] = resizePlane(weights[c*plane:(c+1)*plane], canvasSize, cropW, cropH, width, height)
	}

	out := image.NewRGBA(image.Rect(0, 0, width, height))

	for y := range height {
		for x := range width {
			pr, pg, pb, _ := img.At(bounds.Min.X+x, bounds.Min.Y+y).RGBA()
			r := float32(pr) / 65535.0
			g := float32(pg) / 65535.0
			b := float32(pb) / 65535.0

			i := y*width + x
			w0 := upsampled[0][i]
			nr, ng, nb := w0*r, w0*g, w0*b

			for c, buf := range renderings {
				w := upsampled[c+1][i]
				o := i * 3
				nr += float32(w * buf[o])
				ng += float32(w * buf[o+1])
				nb += float32(w * buf[o+2])
			}

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

// resizePlane bilinearly upsamples the top-left cropW x cropH region of one canvas-sized plane to dstW x dstH,
// recomputing the sampling positions per pixel rather than looking them up.
func resizePlane(src []float32, canvasSize, cropW, cropH, dstW, dstH int) []float32 {
	out := make([]float32, dstW*dstH)

	axis := func(i, srcLen, dstLen int) (int, int, float32) {
		s := (float64(i)+0.5)*(float64(srcLen)/float64(dstLen)) - 0.5
		if s < 0 {
			s = 0
		}
		i0 := min(int(s), srcLen-1)

		return i0, min(i0+1, srcLen-1), float32(s - float64(i0))
	}

	for y := range dstH {
		y0, y1, fy := axis(y, cropH, dstH)
		for x := range dstW {
			x0, x1, fx := axis(x, cropW, dstW)

			top := float32(src[y0*canvasSize+x0]*(1-fx)) + float32(src[y0*canvasSize+x1]*fx)
			bottom := float32(src[y1*canvasSize+x0]*(1-fx)) + float32(src[y1*canvasSize+x1]*fx)
			out[y*dstW+x] = float32(top*(1-fy)) + float32(bottom*fy)
		}
	}

	return out
}
