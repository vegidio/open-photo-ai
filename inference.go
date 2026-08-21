package opai

import (
	"context"
	"image"
	"strings"
	"sync/atomic"
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
	"github.com/vegidio/open-photo-ai/models/upscale/osaka"
	"github.com/vegidio/open-photo-ai/models/upscale/saitama"
	"github.com/vegidio/open-photo-ai/models/upscale/tokyo"
	"github.com/vegidio/open-photo-ai/types"
)

// Process runs the image through the given operations in sequence, each on the AI model that implements it.
//
// onProgress reports the progress of the whole chain in Progress.Total, so the bar fills once no matter how many
// operations there are: each operation owns an equal 1/n slice of it, and Progress.Operation says which one is
// currently running. Progress.Fraction separately carries how far the current phase has got on its own terms, which
// is what a "Downloading 34%" style label needs while the bar itself is still near the start of a slice.
//
// # Example:
//
//	output, err := Process(ctx, inputImage, ep, onProgress, faceRecoveryOp, upscaleOp)
func Process(
	ctx context.Context,
	input *types.ImageData,
	ep types.ExecutionProvider,
	onProgress types.ProgressHandler,
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

	// Tracks the latest image through the chain; the input's own Pixels are never written to, so the caller's
	// ImageData is left untouched.
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

		// The slice of the overall bar this operation owns. Whatever reports against this operation - a cache hit
		// below, or runInference - reports 0-1 for itself, so without this rescaling the bar would refill from zero
		// once per operation instead of once per call. Fraction is left alone: it is deliberately phase-local.
		base, frac := float64(i)/float64(len(operations)), 1/float64(len(operations))

		wrapped := onProgress
		if onProgress != nil {
			wrapped = func(progress types.Progress) {
				progress.Total = base + progress.Total*frac
				onProgress(progress)
			}
		}

		if useCache {
			if cachedImg, err := internal.ImageCache.GetImage(ctx, input.Hash, applied...); err == nil {
				internal.Log().Debug("cache hit", "op", op.Id(), "index", i)
				output = cachedImg

				// A skipped operation still has to hand its slice back, or the bar stalls for as long as the cached
				// operations last. It goes through wrapped like every other report, so the slice arithmetic above is
				// the only place that knows how an operation maps onto the overall bar.
				if wrapped != nil {
					wrapped(types.Progress{
						Operation: op.Id(),
						Phase:     types.PhaseInference,
						Total:     1,
						Fraction:  1,
					})
				}

				continue
			}

			internal.Log().Debug("cache miss, running inference", "op", op.Id(), "index", i)
		}

		output, err = runInference(ctx, leases, output, ep, wrapped, op)
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

// Execute runs a single operation whose output is data rather than an image - face detection, for instance. T must
// match what the operation's model produces, or it fails with "operation type not supported".
//
// onProgress reports the operation's progress in the 0-1 range, the model download and the run itself sharing that one
// range so the bar never goes backwards between them.
//
// # Example:
//
//	faces, err := Execute[[]detection.Face](ctx, inputImage, ep, onProgress, faceDetectionOp)
func Execute[T any](
	ctx context.Context,
	input *types.ImageData,
	ep types.ExecutionProvider,
	onProgress types.ProgressHandler,
	operation types.Operation,
) (T, error) {
	var genericNil T

	split := &progressSplitter{id: operation.Id(), onProgress: onProgress}

	lease, err := selectModel(ctx, operation, ep, split.download)
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
	result, err := dataModel.Run(ctx, input.Pixels, paramsOf(operation), split.run())
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

// downloadShare is the slice of one operation's progress range given to fetching its model, when the model isn't on
// disk yet. Downloading and running would otherwise both report 0-1 over the same range and the bar would visibly go
// backwards once the download finished.
const downloadShare = 0.2

// progressSplitter folds the two phases of carrying out one operation - fetching the model, then running it - into the
// single 0-1 range that operation is allotted, and turns what the model reports into what the caller of Process or
// Execute is promised.
//
// The two phases can't just each report 0-1: the caller sees one bar, and a download handing over to a run would send
// it back to the start. The download therefore takes the head of the range and the run takes the rest.
type progressSplitter struct {
	// id is the operation's full ID, resolved once at construction. Operation.Id() formats a string on every call,
	// and the run callback below fires once per tile - thousands of times on a large upscale - so calling it there
	// would put an avoidable allocation on the hot path.
	id         string
	onProgress types.ProgressHandler

	// downloaded records whether a download actually happened, and is atomic because the download callback runs on
	// whichever goroutine deps.Install reports from rather than this one.
	downloaded atomic.Bool
}

// download is the types.DownloadProgress to hand to selectModel.
func (s *progressSplitter) download(_, _ int64, percent float64) {
	s.downloaded.Store(true)

	if s.onProgress == nil {
		return
	}

	s.onProgress(types.Progress{
		Operation: s.id,
		Phase:     types.PhaseDownload,
		Total:     percent * downloadShare,
		Fraction:  percent,
	})
}

// run is the types.InferenceProgress to hand to Model.Run. It must be called only once selectModel has returned, which
// is what settles whether a download took the head of the range: a model already on disk never reports one, and
// shouldn't have to give up part of its range for it.
//
// It returns nil when the caller asked for no progress, which is the same thing the models check for.
func (s *progressSplitter) run() types.InferenceProgress {
	if s.onProgress == nil {
		return nil
	}

	base, span := 0.0, 1.0
	if s.downloaded.Load() {
		base, span = downloadShare, 1-downloadShare
	}

	// The name the model reports is dropped in favour of the operation's full ID: the models report a two-letter
	// family prefix, and the operation itself is both more specific and consistent across phases.
	return func(_ string, progress float64) {
		s.onProgress(types.Progress{
			Operation: s.id,
			Phase:     types.PhaseInference,
			Total:     base + progress*span,
			Fraction:  progress,
		})
	}
}

// runInference runs one operation of a Process chain.
//
// onProgress covers this one operation in the 0-1 range - Process is what maps that onto the operation's slice of the
// overall bar. Within it, progressSplitter divides the range between fetching the model and running it.
//
// The lease it takes is handed to leases rather than released here: the models of earlier operations must stay
// resident until the whole chain finishes, or a concurrent admission could evict operation 1's model while operation 2
// is still running.
func runInference(
	ctx context.Context,
	leases *leaseSet,
	img image.Image,
	ep types.ExecutionProvider,
	onProgress types.ProgressHandler,
	operation types.Operation,
) (image.Image, error) {
	split := &progressSplitter{id: operation.Id(), onProgress: onProgress}

	lease, err := selectModel(ctx, operation, ep, split.download)
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
	result, err := imageModel.Run(ctx, img, paramsOf(operation), split.run())
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
	case strings.HasPrefix(operation.Id(), "up_osaka"):
		model, err = osaka.New(ctx, operation, ep, onProgress)

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
