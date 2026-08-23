package osaka

import (
	"context"
	"image"
	"math"

	"github.com/cockroachdb/errors"
	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

// noiseSeed fixes the noise field across runs. A diffusion model with a fresh seed each time would hand the user a
// different image every run, which the image cache would then memoize - so the cached result and a re-run would
// disagree, and no A/B comparison of settings would mean anything.
const noiseSeed uint64 = 0x5EEDBEEF

// runPipeline resamples the image to its target size and then restores detail at that size.
//
// The resampling is not a fallback for a model that cannot upscale - it is how SeedVR2 is meant to be driven. The
// reference implementation resizes to the target resolution before inference with the comment that the model was only
// trained at high resolution, and the network is resolution-preserving throughout: the VAE compresses 8x and expands
// 8x, and the transformer between them does not change the token grid.
func runPipeline(
	ctx context.Context,
	m *upscale.Model,
	img image.Image,
	scale float64,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	if onProgress != nil {
		onProgress(0)
	}

	bounds := img.Bounds()
	targetW := int(math.Round(float64(bounds.Dx()) * scale))
	targetH := int(math.Round(float64(bounds.Dy()) * scale))

	// Lanczos both upsamples and, at scale 1, leaves the image alone. Either way this is the image the model is
	// conditioned on and the reference the colour fix corrects towards.
	base := imaging.Resize(img, targetW, targetH, imaging.Lanczos)

	// Two constraints on the size handed to the model. Every dimension must be a multiple of 16, because the VAE
	// compresses 8x and the DiT patchifies 2x on top of that. And neither dimension may be below one DiT region,
	// since that graph accepts exactly one region size - an image smaller than a region has to be padded up rather
	// than run at its own size.
	//
	// Padding rather than cropping keeps the edge pixels the user asked for; it is trimmed off at the end.
	paddedW := max(ditRegionEdge, utils.RoundUpTo16(targetW))
	paddedH := max(ditRegionEdge, utils.RoundUpTo16(targetH))
	padded := utils.ReflectionPad(base, 0, 0, paddedW-targetW, paddedH-targetH)

	// standardize=true is the [-1,1] range the reference pipeline normalizes to.
	basePixels := utils.ImageToCHW(padded, false, true)

	warnIfMemoryTight(m.EP())

	internal.Log().Debug("osaka pipeline",
		"scale", scale, "target", []int{targetW, targetH}, "padded", []int{paddedW, paddedH},
		"region", ditRegionEdge)

	restored, err := restore(ctx, m, basePixels, paddedW, paddedH, onProgress)
	if err != nil {
		return nil, err
	}

	// Once, on the assembled image, against the resampled input. Per-tile correction would fix each tile to its own
	// crop and bake the tile-to-tile drift in as a step at every boundary.
	restored = waveletColorFix(restored, basePixels, paddedW, paddedH, defaultColorFixLevels)

	out := utils.CHWToImage(restored, paddedW, paddedH, true)

	if onProgress != nil {
		onProgress(1)
	}

	if paddedW != targetW || paddedH != targetH {
		return imaging.Crop(out, image.Rect(0, 0, targetW, targetH)), nil
	}

	return out, nil
}

// restore runs the model over the image region by region. There is no whole-image path: the DiT accepts one size,
// so even an image that would comfortably fit in memory is processed in fixed-size regions.
//
// This is a custom driver rather than utils.RunTiledInference, which is the path every other model takes and the one
// to prefer. Four of that driver's assumptions are hardwired, and all four are wrong here: it drives a single session
// (Osaka runs a VAE encode, a DiT step and a VAE decode); it feeds [1,3,H,W] in [0,1] (Osaka needs [-1,1]); it takes
// an integer output scale (SeedVR2 does not change resolution - the image is resampled first and restored at that
// size); and it pads blindly to the tile size (Osaka must pad to a multiple of 16, for the 8x VAE and the 2x patchify
// on top of it). What can be shared is shared: the partitioning rule via utils.TileGrid and the padding via
// utils.ReflectionPad. The blending is deliberately its own - see canvas in blend.go.
func restore(
	ctx context.Context,
	m *upscale.Model,
	pixels []float32,
	width, height int,
	onProgress types.InferenceProgress,
) ([]float32, error) {
	grid := utils.TileGrid{Size: ditRegionEdge, Overlap: tileOverlap, Width: width, Height: height}
	tiles := grid.Tiles()
	canvas := newCanvas(width, height)

	for i, rect := range tiles {
		if err := ctx.Err(); err != nil {
			return nil, errors.Wrap(err, "context cancelled")
		}

		region := cropCHW(pixels, width, height, rect.Min.X, rect.Min.Y, rect.Dx(), rect.Dy(), 3)

		out, err := restoreRegion(ctx, m, region, rect.Dx(), rect.Dy(), rect.Min.X, rect.Min.Y)
		if err != nil {
			return nil, errors.Wrapf(err, "failed to restore tile %d of %d", i+1, len(tiles))
		}

		canvas.add(out, rect, tileOverlap)

		if onProgress != nil {
			onProgress(utils.ClampProgress(float64(i+1) / float64(len(tiles))))
		}
	}

	return canvas.resolve(), nil
}

// restoreRegion is the model itself: encode to latent space, take one diffusion step, decode back.
//
// The region's dimensions must already be multiples of 16, which the caller guarantees - the image is padded to that
// multiple, and every tile geometry is itself a multiple of 16, so no tile can be misaligned.
func restoreRegion(
	ctx context.Context,
	m *upscale.Model,
	pixels []float32,
	width, height, originX, originY int,
) ([]float32, error) {
	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// The DiT accepts one region size and nothing else. Checking here turns what would otherwise surface as an
	// opaque reshape failure from inside the runtime into a statement of the actual constraint.
	if width != ditRegionEdge || height != ditRegionEdge {
		return nil, errors.Errorf("region is %dx%d, but the model accepts only %dx%d",
			width, height, ditRegionEdge, ditRegionEdge)
	}

	latentW, latentH := width/vaeStride, height/vaeStride
	latentPlane := latentW * latentH

	cond, err := utils.RunUnary(m.Graph(roleEncoder),
		pixels,
		ort.NewShape(1, 3, int64(height), int64(width)),
		ort.NewShape(1, latentChannels, int64(latentH), int64(latentW)))
	if err != nil {
		return nil, errors.Wrap(err, "failed to encode the region")
	}

	noise := gaussianNoise(latentChannels*latentPlane, originX, originY, noiseSeed)
	vidInput := packVidInput(cond, noise, latentPlane)

	prediction, err := runStep(m.Graph(roleDiT),
		vidInput, ditTimestep,
		ort.NewShape(1, ditChannels, int64(latentH), int64(latentW)),
		ort.NewShape(1, latentChannels, int64(latentH), int64(latentW)))
	if err != nil {
		return nil, errors.Wrap(err, "failed to run the diffusion step")
	}

	denoised := schedulerStep(prediction, noise)

	out, err := utils.RunUnary(m.Graph(roleDecoder),
		denoised,
		ort.NewShape(1, latentChannels, int64(latentH), int64(latentW)),
		ort.NewShape(1, 3, int64(height), int64(width)))
	if err != nil {
		return nil, errors.Wrap(err, "failed to decode the region")
	}

	return out, nil
}

// schedulerStep turns the transformer's prediction into the clean latent.
//
// The graph's output is named denoised_latent, but it is the flow-matching velocity, not the latent itself - the
// reference implementation calls the same tensor "noise" and hands it to a scheduler. For a single step the schedule
// runs from t=1000 to t=0, so the normalized time is 1 and the whole update collapses to
//
//	x0 = sample - t_norm * prediction = noise - prediction
//
// Skipping it is not a subtle error: the decoded result is the image buried under the velocity field, which scores
// worse than the input it was given.
func schedulerStep(prediction, noise []float32) []float32 {
	out := make([]float32, len(prediction))
	for i := range prediction {
		out[i] = noise[i] - prediction[i]
	}

	return out
}

// runStep runs the diffusion transformer, which takes the packed latent and the timestep.
func runStep(session *utils.Session, vidInput []float32, timestep float32, inShape, outShape ort.Shape) ([]float32, error) {
	vidTensor, err := ort.NewTensor(inShape, vidInput)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create the vid_input tensor")
	}
	defer vidTensor.Destroy()

	stepTensor, err := ort.NewTensor(ort.NewShape(1), []float32{timestep})
	if err != nil {
		return nil, errors.Wrap(err, "failed to create the timestep tensor")
	}
	defer stepTensor.Destroy()

	return utils.RunSession(session, []ort.Value{vidTensor, stepTensor}, outShape)
}
