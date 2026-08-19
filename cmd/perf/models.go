package main

import (
	"context"
	"errors"
	"fmt"
	"slices"
	"strings"

	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/colorbalance/rio"
	"github.com/vegidio/open-photo-ai/models/denoise/gothenburg"
	"github.com/vegidio/open-photo-ai/models/denoise/malmo"
	"github.com/vegidio/open-photo-ai/models/denoise/stockholm"
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/models/detection/newyork"
	"github.com/vegidio/open-photo-ai/models/facerecovery/athens"
	"github.com/vegidio/open-photo-ai/models/facerecovery/santorini"
	"github.com/vegidio/open-photo-ai/models/lightadjustment/paris"
	"github.com/vegidio/open-photo-ai/models/sharpen/moscow"
	"github.com/vegidio/open-photo-ai/models/sharpen/novgorod"
	"github.com/vegidio/open-photo-ai/models/sharpen/petersburg"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/models/upscale/osaka"
	"github.com/vegidio/open-photo-ai/models/upscale/saitama"
	"github.com/vegidio/open-photo-ai/models/upscale/tokyo"
	"github.com/vegidio/open-photo-ai/types"
)

// outcome is what one iteration contributes to the report: the size of the image produced (zero for models that don't
// produce one) and a short note, e.g. how many faces a detector found.
type outcome struct {
	width  int
	height int
	note   string
}

// runner performs one benchmark iteration. It closes over an operation that is already fully built, so the timed
// section contains nothing but the library call, and it hides whether the model produces an image (opai.Process) or
// data (opai.Execute) so bench.go needs no per-model branch.
type runner func(ctx context.Context, input *types.ImageData) (outcome, error)

// entry is one benchmarkable model. Adding a model is one line in catalog below: the four Op shapes are normalized
// into a single build function by the adapters further down.
type entry struct {
	name string
	kind types.ModelType

	// build prepares everything that must NOT be timed: it constructs the operation from cfg and, for the face
	// recovery models, runs the detection pass that produces their input faces. It can therefore be slow and fail.
	build func(ctx context.Context, input *types.ImageData, cfg config) (runner, error)
}

// catalog is ordered; both the sweep and `list` follow it, so a full run reads top-to-bottom like the list output.
var catalog = []entry{
	// Upscale
	scaleEntry("tokyo", types.ModelTypeUpscale, tokyo.Op),
	scaleEntry("kyoto", types.ModelTypeUpscale, kyoto.Op),
	scaleEntry("saitama", types.ModelTypeUpscale, saitama.Op),

	// Osaka is the diffusion upscaler: the first run downloads 7.3 GB, and it is orders of magnitude slower than
	// the others on anything but a capable GPU.
	scaleEntry("osaka", types.ModelTypeUpscale, osaka.Op),

	// Denoise
	intensityEntry("stockholm", types.ModelTypeDenoise, stockholm.Op),
	intensityEntry("gothenburg", types.ModelTypeDenoise, gothenburg.Op),
	intensityEntry("malmo", types.ModelTypeDenoise, malmo.Op),

	// Sharpen
	intensityEntry("moscow", types.ModelTypeSharpen, moscow.Op),
	intensityEntry("petersburg", types.ModelTypeSharpen, petersburg.Op),
	intensityEntry("novgorod", types.ModelTypeSharpen, novgorod.Op),

	// Light Adjustment / Color Balance
	intensityEntry("paris", types.ModelTypeLightAdjustment, paris.Op),
	intensityEntry("rio", types.ModelTypeColorBalance, rio.Op),

	// Detection
	detectEntry("newyork", types.ModelTypeDetection, newyork.Op),

	// Face Recovery
	faceEntry("athens", types.ModelTypeFaceRecovery, athens.Op),
	faceEntry("santorini", types.ModelTypeFaceRecovery, santorini.Op),
}

// lookup finds a model by name. A linear scan over 14 entries needs no init() and no parallel map to keep in sync.
func lookup(name string) (entry, bool) {
	i := slices.IndexFunc(catalog, func(e entry) bool { return e.name == name })
	if i < 0 {
		return entry{}, false
	}

	return catalog[i], true
}

// resolveSelection turns the positional arguments into the models to benchmark. No arguments means the whole catalog.
// Duplicates are dropped, keeping the first occurrence, so "perftest kyoto kyoto" doesn't benchmark kyoto twice.
//
// It is called before opai.Initialize so a typo fails in milliseconds instead of after a multi-gigabyte download.
func resolveSelection(names []string) ([]entry, error) {
	if len(names) == 0 {
		return slices.Clone(catalog), nil
	}

	selection := make([]entry, 0, len(names))
	seen := make(map[string]bool, len(names))

	for _, name := range names {
		name = strings.ToLower(strings.TrimSpace(name))
		if seen[name] {
			continue
		}

		e, known := lookup(name)
		if !known {
			return nil, fmt.Errorf("unknown model %q; run \"perftest list\" to see the available models", name)
		}

		seen[name] = true
		selection = append(selection, e)
	}

	return selection, nil
}

// region - Op adapters

// Each model's Op returns its own concrete operation type, so the adapters below are generic over that type: Go will
// not convert `func(float64, types.Precision) tokyo.OpUpTokyo` into `func(float64, types.Precision) types.Operation`
// implicitly. cmd/gui/utils/operations.go solves the same problem the same way, but it lives in module `gui` and so
// can't be shared with this one.

// scaleEntry adapts the upscale constructors. Op itself clamps the scale to 1..8; --scale's validator mirrors that so
// the report can't claim a factor the model didn't use.
func scaleEntry[T types.Operation](name string, kind types.ModelType, op func(float64, types.Precision) T) entry {
	return entry{
		name: name,
		kind: kind,
		build: func(_ context.Context, _ *types.ImageData, cfg config) (runner, error) {
			return processRunner(cfg, op(cfg.scale, cfg.precision), ""), nil
		},
	}
}

// intensityEntry adapts the denoise / sharpen / light-adjustment / color-balance constructors. The intensity is a
// post-inference blend factor (see internal/utils.BlendWithIntensity), so it changes the output and the cache key but
// barely the timing; it is configurable for completeness.
func intensityEntry[T types.Operation](name string, kind types.ModelType, op func(float32, types.Precision) T) entry {
	return entry{
		name: name,
		kind: kind,
		build: func(_ context.Context, _ *types.ImageData, cfg config) (runner, error) {
			return processRunner(cfg, op(cfg.intensity, cfg.precision), ""), nil
		},
	}
}

// detectEntry adapts a model whose result is data rather than an image, and which therefore must go through
// opai.Execute. The type parameter is the operation type; the result type is fixed to []detection.Face, which is what
// the detection models return.
func detectEntry[T types.Operation](name string, kind types.ModelType, op func(types.Precision) T) entry {
	return entry{
		name: name,
		kind: kind,
		build: func(_ context.Context, _ *types.ImageData, cfg config) (runner, error) {
			operation := op(cfg.precision)

			return func(ctx context.Context, input *types.ImageData) (outcome, error) {
				faces, err := opai.Execute[[]detection.Face](ctx, input, cfg.provider, nil, operation)
				if err != nil {
					return outcome{}, err
				}

				return outcome{note: facesNote(len(faces))}, nil
			}, nil
		},
	}
}

// faceEntry adapts the face-recovery constructors, which only have work to do on faces someone else located: without a
// detection pass they would recover nothing and the benchmark would time a no-op.
//
// The faces come from cfg, where detectFacesOnce put them before the sweep started, so the detection is never charged
// to the model under test and is never repeated per model.
func faceEntry[T types.Operation](
	name string,
	kind types.ModelType,
	op func(types.Precision, []detection.Face) T,
) entry {
	return entry{
		name: name,
		kind: kind,
		build: func(_ context.Context, _ *types.ImageData, cfg config) (runner, error) {
			if cfg.faces.err != nil {
				return nil, cfg.faces.err
			}

			faces := cfg.faces.faces

			return processRunner(cfg, op(cfg.precision, faces), facesNote(len(faces))), nil
		},
	}
}

// detectFaces runs the auxiliary detection pass that the face-recovery models need as input.
//
// The precision is pinned to fp32 rather than taken from cfg: this mirrors the library's own auxiliary detection
// (models/facerecovery.GetDtModel pins fp32), and an fp16 detection model may not be published. Taking cfg.precision
// here would break face recovery under --precision fp16 for a reason that has nothing to do with the model being
// benchmarked.
func detectFaces(ctx context.Context, input *types.ImageData, cfg config) ([]detection.Face, error) {
	faces, err := opai.Execute[[]detection.Face](ctx, input, cfg.provider, nil, newyork.Op(types.PrecisionFp32))
	if err != nil {
		return nil, fmt.Errorf("face detection failed: %w", err)
	}

	if len(faces) == 0 {
		return nil, errors.New("no faces detected in the input image, so there would be nothing to recover")
	}

	return faces, nil
}

// processRunner is the single iteration shared by every model that produces an image.
func processRunner(cfg config, operation types.Operation, note string) runner {
	return func(ctx context.Context, input *types.ImageData) (outcome, error) {
		out, err := opai.Process(ctx, input, cfg.provider, cfg.onProgress, operation)
		if err != nil {
			return outcome{}, err
		}

		bounds := out.Pixels.Bounds()
		return outcome{width: bounds.Dx(), height: bounds.Dy(), note: note}, nil
	}
}

func facesNote(count int) string {
	if count == 1 {
		return "1 face"
	}

	return fmt.Sprintf("%d faces", count)
}

// endregion
