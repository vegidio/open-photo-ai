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

	// Every model this chain touches is pinned until the whole chain finishes, not just until its own operation
	// returns: releasing per operation would let a concurrent admission evict operation 1's model while operation 2 is
	// still running, and the rebuild would be charged to this call.
	leases := &leaseSet{}
	defer leases.releaseAll()

	start := time.Now()
	internal.Log().Info("processing image", "op_count", len(operations), "hash", input.Hash)

	// Make a copy of the input img so the original input is not modified
	output := input.Pixels

	// Read once per call rather than per operation, so a concurrent SetImageCacheEnabled can't have this loop read
	// from the cache and then decline to write back to it.
	useCache := internal.ImageCacheEnabled()
	if !useCache {
		internal.Log().Debug("image cache disabled for this call")
	}

	for i, op := range operations {
		// The cache key is the input image plus every operation applied so far, so the same operation lands in a
		// different slot depending on what preceded it.
		applied := operations[:i+1]

		if useCache {
			if cachedImg, err := internal.ImageCache.GetImage(ctx, input.Hash, applied...); err == nil {
				internal.Log().Debug("cache hit", "op", op.Id(), "index", i)
				output = cachedImg

				continue
			}

			internal.Log().Debug("cache miss, running inference", "op", op.Id(), "index", i)
		}

		output, err = runInference(ctx, leases, output, ep, onProgress, op)
		if err != nil {
			return nil, errors.Wrap(err, "error running inference")
		}

		if !useCache {
			continue
		}

		if err = internal.ImageCache.SetImage(ctx, output, input.Hash, applied...); err != nil {
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

	lease, err := selectModel(ctx, operation, ep, func(_, _ int64, percent float64) {
		if onProgress != nil {
			onProgress("dl", percent)
		}
	})

	if err != nil {
		return genericNil, errors.Wrap(err, "error selecting model")
	}

	// Keeps the model alive from here until Run returns. Deferred before the type assertion below, which has its own
	// early return.
	defer lease.Release()

	dataModel, ok := lease.Model().(types.Model[T])
	if !ok {
		internal.Log().Warn("operation type not supported", "op", operation.Id())
		return genericNil, errors.Errorf("operation type not supported: %s", operation.Id())
	}

	start := time.Now()
	result, err := dataModel.Run(ctx, input.Pixels, paramsOf(operation), onProgress)
	logModelRun(operation, start)
	return result, err
}

// cleanRegistryTimeout bounds how long CleanRegistry waits for models that are still in use. Reaching it means a run
// has been in flight for half a minute, which is possible for a large upscale, so the wait is generous.
const cleanRegistryTimeout = 30 * time.Second

// CleanRegistry unloads every model currently held in memory, destroying the ONNX session behind each one. It also
// forgets which execution provider was found to be unusable, so the next model creation gives the requested one
// another chance.
//
// Models that are still in use are removed from the registry immediately but destroyed only once the work using them
// finishes - freeing a session under a live inference is a use-after-free in native code that terminates the process
// (see https://github.com/vegidio/open-photo-ai/issues/34). This function waits for that to happen, so when it returns
// the memory really has been released.
//
// The waiting is load-bearing beyond tidiness: cmd/perf drains between benchmark runs precisely so the next run pays
// full session construction, and a drain that returned before the sessions were gone would silently understate every
// cold-start number the tool reports.
//
// Unlike the process-wide lock this used to take, waiting here only blocks on the models being torn down. Inference
// that doesn't touch them runs straight through.
func CleanRegistry() {
	internal.ResetFallback()

	drained := internal.Registry.DrainAll()
	internal.Log().Debug("cleaning model registry", "destroyed_now", len(drained))

	internal.DestroyEntries(drained)

	if !internal.Registry.WaitDrained(cleanRegistryTimeout) {
		internal.Log().Warn("timed out waiting for in-use models to be released; they are destroyed when their work finishes",
			"timeout", cleanRegistryTimeout)
	}
}

// region - Private functions

// leaseSet collects the model leases taken during one Process call so they can all be released together when the
// operation chain finishes.
//
// It is not safe for concurrent use, and does not need to be: a Process call runs its operations sequentially on one
// goroutine. An operation repeated in the same chain takes a second lease on the same model, which is correct - the
// refcount goes up twice and comes back down twice.
type leaseSet struct {
	leases []*internal.Lease
}

func (s *leaseSet) add(lease *internal.Lease) {
	s.leases = append(s.leases, lease)
}

func (s *leaseSet) releaseAll() {
	for _, lease := range s.leases {
		lease.Release()
	}

	s.leases = nil
}

// runInference runs one operation of a Process chain.
//
// The lease it takes is handed to leases rather than released here: the models of earlier operations must stay
// resident until the whole chain finishes, or a concurrent admission could evict operation 1's model while operation 2
// is still running.
func runInference(
	ctx context.Context,
	leases *leaseSet,
	img image.Image,
	ep types.ExecutionProvider,
	onProgress types.InferenceProgress,
	operation types.Operation,
) (image.Image, error) {
	lease, err := selectModel(ctx, operation, ep, func(_, _ int64, percent float64) {
		if onProgress != nil {
			onProgress("dl", percent)
		}
	})

	if err != nil {
		return nil, errors.Wrap(err, "error selecting model")
	}

	leases.add(lease)

	imageModel, ok := lease.Model().(types.Model[image.Image])
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

// selectModel returns a lease on the model that implements the given operation, building it on first use. The registry
// lookup and the fallback to the CPU live in internal.AcquireModel, shared with the face detection used by
// SuggestEnhancements; all this adds is the switch that knows which model an operation maps to.
//
// The caller owns the returned lease and must release it.
func selectModel(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*internal.Lease, error) {
	return internal.AcquireModel(operation.Id(), ep, func(ep types.ExecutionProvider) (any, error) {
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
