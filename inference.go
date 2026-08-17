package opai

import (
	"context"
	"image"
	"strings"
	"time"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/models/colorbalance/rio"
	"github.com/vegidio/open-photo-ai/models/denoise/gothenburg"
	"github.com/vegidio/open-photo-ai/models/denoise/malmo"
	"github.com/vegidio/open-photo-ai/models/denoise/stockholm"
	"github.com/vegidio/open-photo-ai/models/detection/newyork"
	"github.com/vegidio/open-photo-ai/models/facerecovery/athens"
	"github.com/vegidio/open-photo-ai/models/facerecovery/santorini"
	"github.com/vegidio/open-photo-ai/models/lightadjustment/paris"
	"github.com/vegidio/open-photo-ai/models/sharpen/moscow"
	"github.com/vegidio/open-photo-ai/models/sharpen/novgorod"
	"github.com/vegidio/open-photo-ai/models/sharpen/petersburg"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/models/upscale/saitama"
	"github.com/vegidio/open-photo-ai/models/upscale/tokyo"
	"github.com/vegidio/open-photo-ai/types"
)

// Process processes an image through a sequence of image operations.
//
// The function selects the appropriate AI model for each operation and runs its inference on the image. If multiple
// operations are provided, they are applied in the order specified. The output is the final processed image after all
// operations are applied.
//
// # Parameters:
//   - ctx: A context object that can be used to cancel the operation.
//   - input: The input image data to be processed.
//   - ep: The execution provider (CPU, CUDA, etc.) to use for inference.
//   - onProgress: A callback function called with the progress of the current operation (0-1).
//   - operations: A variable number of operations to apply sequentially.
//
// # Returns:
//   - *types.ImageData: The final processed image data after all operations
//   - error: An error if model selection fails, any operation fails, or no operations are provided
//
// # Example:
//
//	output, err := Process(ctx, inputImage, faceRecoveryOp, upscaleOp)
func Process(
	ctx context.Context,
	input *types.ImageData,
	ep types.ExecutionProvider,
	onProgress types.InferenceProgress,
	operations ...types.Operation,
) (*types.ImageData, error) {
	var err error

	// Held for the whole loop, not per operation, so the registry can't be drained between two operations of the same
	// image. See internal.InferenceMu.
	internal.InferenceMu.RLock()
	defer internal.InferenceMu.RUnlock()

	start := time.Now()
	internal.Log().Info("processing image", "op_count", len(operations), "hash", input.Hash)

	// Make a copy of the input img so the original input is not modified
	output := input.Pixels

	// Read once per call rather than per operation, so a concurrent SetImageCacheEnabled can't have this loop read
	// from the cache and then decline to write back to it.
	useCache := internal.ImageCacheEnabled()

	for i, op := range operations {
		if !useCache {
			internal.Log().Debug("cache disabled, running inference", "op", op.Id(), "index", i)
		} else if cachedImg, err := internal.ImageCache.GetImage(ctx, input.Hash, operations[:i+1]...); err == nil {
			// Check first if there's a cached image for this operation
			internal.Log().Debug("cache hit", "op", op.Id(), "index", i)
			output = cachedImg
			continue
		} else {
			internal.Log().Debug("cache miss, running inference", "op", op.Id(), "index", i)
		}

		output, err = runInference(ctx, output, ep, onProgress, op)
		if err != nil {
			return nil, errors.Wrap(err, "error running inference")
		}

		if !useCache {
			continue
		}

		if err = internal.ImageCache.SetImage(ctx, output, input.Hash, operations[:i+1]...); err != nil {
			return nil, errors.Wrap(err, "error caching image")
		}
	}

	internal.Log().Info("image processed",
		"op_count", len(operations), "hash", input.Hash, "duration", time.Since(start))

	return &types.ImageData{
		FilePath: input.FilePath,
		Pixels:   output,
		Hash:     input.Hash,
	}, nil
}

// Execute executes a single image operation and returns the result as a generic data type.
//
// The function selects the appropriate AI model for the operation and runs its inference on the image. The output is
// not an image, but the information data returned by the model.
//
// # Parameters:
//   - ctx: A context object that can be used to cancel the operation.
//   - input: The input image data to be processed.
//   - ep: The execution provider (CPU, CUDA, etc.) to use for inference.
//   - onProgress: A callback function called with the progress of the current operation (0-1).
//   - operation: The operation to apply to the image.
//
// # Returns:
//   - T: The result of the operation with the specified generic type
//   - error: An error if model selection fails, the operation fails, or the operation type is not supported
//
// # Example:
//
//	faces, err := Execute[[]types.Face](ctx, inputImage, progressCallback, faceDetectionOp)
func Execute[T any](
	ctx context.Context,
	input *types.ImageData,
	ep types.ExecutionProvider,
	onProgress types.InferenceProgress,
	operation types.Operation,
) (T, error) {
	// nil value for type T
	var genericNil T

	// Keeps the model alive from creation until Run returns. See internal.InferenceMu.
	internal.InferenceMu.RLock()
	defer internal.InferenceMu.RUnlock()

	model, err := selectModel(ctx, operation, ep, func(_, _ int64, percent float64) {
		if onProgress != nil {
			onProgress("dl", percent)
		}
	})

	if err != nil {
		return genericNil, errors.Wrap(err, "error selecting model")
	}

	dataModel, ok := model.(types.Model[T])
	if !ok {
		internal.Log().Warn("operation type not supported", "op", operation.Id())
		return genericNil, errors.Errorf("operation type not supported: %s", operation.Id())
	}

	start := time.Now()
	result, err := dataModel.Run(ctx, input.Pixels, paramsOf(operation), onProgress)
	logModelRun(operation, start)
	return result, err
}

// CleanRegistry releases all resources held by registered models. It iterates through all models in the registry and
// calls their Destroy method to clean up memory and other resources.
//
// This function should be called when the application is shutting down or when all model instances are no longer needed
// to prevent resource leaks. It also forgets which execution provider was found to be unusable, so the next model
// creation gives the requested one another chance - which is what makes cleaning the registry the way to apply a change
// of execution provider.
//
// Destroying a model releases its underlying ONNX session, and freeing one while another goroutine is running inference
// on it is a use-after-free in native code that terminates the process (see
// https://github.com/vegidio/open-photo-ai/issues/34). This function therefore blocks until the inference in flight has
// finished; callers don't have to coordinate anything themselves.
func CleanRegistry() {
	internal.InferenceMu.Lock()
	defer internal.InferenceMu.Unlock()

	internal.ResetFallback()

	drained := internal.Registry.Drain()
	internal.Log().Debug("cleaning model registry", "count", len(drained))

	for _, model := range drained {
		if destroyable, ok := model.(types.Destroyable); ok {
			destroyable.Destroy()
		}
	}
}

// region - Private functions

func runInference(
	ctx context.Context,
	img image.Image,
	ep types.ExecutionProvider,
	onProgress types.InferenceProgress,
	operation types.Operation,
) (image.Image, error) {
	model, err := selectModel(ctx, operation, ep, func(_, _ int64, percent float64) {
		if onProgress != nil {
			onProgress("dl", percent)
		}
	})

	if err != nil {
		return nil, errors.Wrap(err, "error selecting model")
	}

	imageModel, ok := model.(types.Model[image.Image])
	if !ok {
		internal.Log().Warn("operation type not supported", "op", operation.Id())
		return nil, errors.Errorf("operation type not supported: %s", operation.Id())
	}

	start := time.Now()
	result, err := imageModel.Run(ctx, img, paramsOf(operation), onProgress)
	logModelRun(operation, start)
	return result, err
}

// paramsOf returns the operation's per-run inputs for Model.Run, or nil when the operation has none. It keeps the
// inference pipeline agnostic of any concrete operation/model type.
func paramsOf(operation types.Operation) map[string]any {
	if p, ok := operation.(types.Parameterized); ok {
		return p.Params()
	}

	return nil
}

// logModelRun emits the per-run timing Debug log shared by Execute and runInference.
func logModelRun(operation types.Operation, start time.Time) {
	internal.Log().Debug("model run complete", "op", operation.Id(), "duration", time.Since(start))
}

// selectModel returns the model that implements the given operation, building it on first use. The registry lookup and
// the fallback to the CPU live in internal.GetOrCreateModel, shared with the face detection used by
// SuggestEnhancements; all this adds is the switch that knows which model an operation maps to.
func selectModel(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (any, error) {
	return internal.GetOrCreateModel(operation.Id(), ep, func(ep types.ExecutionProvider) (any, error) {
		return newModel(ctx, operation, ep, onProgress)
	})
}

// newModel builds the model that implements the given operation, using the requested execution provider.
func newModel(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (any, error) {
	var model any
	var err error

	switch {
	// Face Detection
	case strings.HasPrefix(operation.Id(), "dt_newyork"):
		model, err = newyork.New(ctx, operation, ep, onProgress)

	// Face Recovery
	case strings.HasPrefix(operation.Id(), "fr_athens"):
		model, err = athens.New(ctx, operation, ep, onProgress)
	case strings.HasPrefix(operation.Id(), "fr_santorini"):
		model, err = santorini.New(ctx, operation, ep, onProgress)

	// Light Adjustment
	case strings.HasPrefix(operation.Id(), "la_paris"):
		model, err = paris.New(ctx, operation, ep, onProgress)

	// Color Balance
	case strings.HasPrefix(operation.Id(), "cb_rio"):
		model, err = rio.New(ctx, operation, ep, onProgress)

	// Upscale
	case strings.HasPrefix(operation.Id(), "up_tokyo"):
		model, err = tokyo.New(ctx, operation, ep, onProgress)
	case strings.HasPrefix(operation.Id(), "up_kyoto"):
		model, err = kyoto.New(ctx, operation, ep, onProgress)
	case strings.HasPrefix(operation.Id(), "up_saitama"):
		model, err = saitama.New(ctx, operation, ep, onProgress)

	// Denoise
	case strings.HasPrefix(operation.Id(), "dn_stockholm"):
		model, err = stockholm.New(ctx, operation, ep, onProgress)
	case strings.HasPrefix(operation.Id(), "dn_malmo"):
		model, err = malmo.New(ctx, operation, ep, onProgress)
	case strings.HasPrefix(operation.Id(), "dn_gothenburg"):
		model, err = gothenburg.New(ctx, operation, ep, onProgress)

	// Sharpen
	case strings.HasPrefix(operation.Id(), "sh_moscow"):
		model, err = moscow.New(ctx, operation, ep, onProgress)
	case strings.HasPrefix(operation.Id(), "sh_novgorod"):
		model, err = novgorod.New(ctx, operation, ep, onProgress)
	case strings.HasPrefix(operation.Id(), "sh_petersburg"):
		model, err = petersburg.New(ctx, operation, ep, onProgress)

	default:
		internal.Log().Warn("no model found for operation", "op", operation.Id())
		return nil, errors.Errorf("no model found with ID: %s", operation.Id())
	}

	return model, err
}

// endregion
