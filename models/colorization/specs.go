package colorization

import (
	"image"

	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal/utils"
)

// The colorization graphs this package can drive. Each is a Spec - pure data - so adding a family is an entry here
// plus its two adapters, not another copy of the pipeline in process.go.

// inputSize is the fixed spatial size the ab-output graphs are exported at. The model only predicts the ab chroma
// planes at this resolution; the output keeps the original image's full-resolution luminance, so this is not a cap on
// output detail.
const inputSize = 512

// rgbGraphSize is the fixed spatial size the RGB-output graphs are exported at. The value comes from the DeOldify
// reference these graphs were exported from: its default render_factor (35) times its render base (16). As with the
// ab-output graphs, only chroma comes from the model, so this does not cap output detail.
//
// That reference's stable and artistic colorizers happen to share the default, so this constant did not move when
// jaipur was re-exported from the artistic generator - which is worth stating, because it is the one number a
// backbone swap would otherwise be expected to change.
const rgbGraphSize = 560

// AbGraph drives the graphs that return chroma directly (delhi, mumbai): the graph takes a gray RGB rendering of the
// image's luminance (CHW, [0,1], 512x512) and returns the predicted Lab ab planes at the same size. The result is
// composed from the original-resolution L channel plus the upsampled ab planes, so luminance detail is preserved
// exactly.
var AbGraph = Spec{
	Size:        inputSize,
	Filter:      imaging.Lanczos,
	BuildInput:  grayLabInput,
	OutChannels: 2,
	Chroma:      abPlanes,
}

// RgbGraph drives the graphs that return a full colorization (jaipur): the graph takes the image's ITU-601 luma
// rendered as gray RGB (CHW, [0,1], 560x560 — ImageNet normalization is baked into the exported graph) and returns
// three RGB channels at the same size, clamped to [0,1].
//
// The result keeps the original image's full-resolution luminance and takes only the model's chroma, upsampled — the
// DeOldify reference these graphs come from does this transfer in YUV; here it is done in Lab to reuse the category's
// tested conversion and composition helpers, which is perceptually equivalent.
//
// That contract is why this spec survived jaipur being re-exported from a different generator of that reference: the
// backbone changed from ResNet101 to ResNet34 and the graph lost two thirds of its weights, but the normalization and
// the SigmoidRange denormalization stayed baked in at the same two ends, so nothing here had to move.
var RgbGraph = Spec{
	Size: rgbGraphSize,
	// The reference stretches to a square with bilinear resampling; Linear is imaging's equivalent.
	Filter:      imaging.Linear,
	BuildInput:  grayLumaInput,
	OutChannels: 3,
	Chroma:      abFromRgb,
}

// grayLabInput builds the model's CHW input tensor from the resized image: each pixel is reduced to its luminance
// (Lab L with zero chroma) and rendered back to RGB, which is the gray image the ab-output graphs' reference
// implementation, DDColor, was trained on.
//
// GrayFromRgbBytes is that reduction with the Lab round-trip collapsed out - at zero chroma it is the identity on the
// luminance, so going through L cost a cube root and three math.Pow per pixel to arrive back where it started. See the
// helper's comment, and TestGrayFromRgbBytesMatchesLabRoundTrip for the bound on the difference.
func grayLabInput(img *image.NRGBA, size int) []float32 {
	plane := size * size
	data := make([]float32, 3*plane)

	for y := range size {
		row := y * img.Stride
		dst := y * size

		for x := range size {
			off := row + x*4
			gray := utils.GrayFromRgbBytes(img.Pix[off], img.Pix[off+1], img.Pix[off+2])

			i := dst + x
			data[i] = gray
			data[plane+i] = gray
			data[2*plane+i] = gray
		}
	}

	return data
}

// abPlanes takes the ab planes straight out of a 2-channel CHW tensor that already holds them.
func abPlanes(data []float32, size int) (a, b []float32) {
	plane := size * size
	return data[:plane], data[plane : 2*plane]
}

// grayLumaInput builds the model's CHW input tensor: each pixel reduced to its ITU-601 luma (the grayscale the
// RGB-output graphs' reference implementation, DeOldify, was trained on — PIL's 'LA' conversion), replicated across
// the three channels, in [0, 1].
func grayLumaInput(img *image.NRGBA, size int) []float32 {
	plane := size * size
	data := make([]float32, 3*plane)

	for y := range size {
		row := y * img.Stride
		dst := y * size

		for x := range size {
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
func abFromRgb(data []float32, size int) (aPlane, bPlane []float32) {
	plane := size * size
	aPlane = make([]float32, plane)
	bPlane = make([]float32, plane)

	for i := range plane {
		_, a, b := utils.RgbToLab(
			min(1, max(0, data[i])),
			min(1, max(0, data[plane+i])),
			min(1, max(0, data[2*plane+i])),
		)
		aPlane[i] = a
		bPlane[i] = b
	}

	return aPlane, bPlane
}
