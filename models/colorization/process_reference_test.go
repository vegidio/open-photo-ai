package colorization

import (
	"image"

	"github.com/vegidio/open-photo-ai/internal/utils"
)

// This file holds the colorization compose path as it was written before it was fused into a single pass: extract the
// luminance plane, upsample each chroma plane, then combine the three. It is kept, verbatim in behaviour, as the
// reference that TestComposeMatchesReference checks the fused implementation against — the fusion is a performance
// change and is only correct if it is byte-for-byte indistinguishable from this.

// lPlane extracts the CIELab L channel of every pixel at full resolution.
func lPlane(img image.Image) []float32 {
	bounds := img.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()

	out := make([]float32, width*height)

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

// resizePlane bilinearly resizes a single float32 plane, aligning pixel centers and clamping at the edges.
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
		y0 := min(int(sy), srcH-1)
		y1 := min(y0+1, srcH-1)
		fy := float32(sy - float64(y0))

		for x := range dstW {
			sx := (float64(x)+0.5)*scaleX - 0.5
			if sx < 0 {
				sx = 0
			}
			x0 := min(int(sx), srcW-1)
			x1 := min(x0+1, srcW-1)
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

			out.Pix[dst] = uint8(utils.Clamp255(r*255.0 + 0.5))
			out.Pix[dst+1] = uint8(utils.Clamp255(g*255.0 + 0.5))
			out.Pix[dst+2] = uint8(utils.Clamp255(b*255.0 + 0.5))
			out.Pix[dst+3] = 255
			dst += 4
		}
	}

	return out
}

// composeReference is the whole pre-fusion tail: the three passes compose() replaced with one.
func composeReference(img image.Image, aPlane, bPlane []float32, srcSize int) image.Image {
	bounds := img.Bounds()
	width := bounds.Dx()
	height := bounds.Dy()

	return composeLab(
		lPlane(img),
		resizePlane(aPlane, srcSize, srcSize, width, height),
		resizePlane(bPlane, srcSize, srcSize, width, height),
		width,
		height,
	)
}
