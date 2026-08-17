package main

import (
	"context"
	"errors"
	"fmt"
	"image"
	"math"
	"slices"
	"sync"
	"time"

	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/types"
)

// config is the fully resolved run configuration, built once from the CLI flags.
type config struct {
	provider  types.ExecutionProvider
	precision types.Precision
	scale     float64
	intensity float32
	runs      int
	warmup    int
	cache     bool

	// onProgress is handed to opai.Process on every run, including the timed ones, so it must be cheap enough to be
	// invisible in the measurement: the live UI's implementation does a single atomic store per tile and nothing else.
	// It is nil when there is no live view to feed.
	onProgress types.InferenceProgress
}

// stats summarizes the timed runs.
type stats struct {
	min    time.Duration
	max    time.Duration
	mean   time.Duration
	median time.Duration
	stdDev time.Duration
}

// result is everything the report needs about one model. A failed model carries err and is rendered as a FAILED row
// rather than aborting the sweep.
type result struct {
	entry    entry
	cold     time.Duration
	runs     []time.Duration // kept in run order, which is what --verbose prints
	stats    stats
	outcome  outcome
	fallback *fallbackEvent
	err      error
}

func (r result) ok() bool { return r.err == nil }

// interrupted reports whether this model stopped because the user cancelled the run rather than because anything is
// wrong with it. Reporting a Ctrl-C as a model failure would send someone debugging a model that is perfectly fine.
func (r result) interrupted() bool {
	return errors.Is(r.err, context.Canceled)
}

// fallbackEvent records that the library downgraded a model to the CPU because the requested execution provider
// couldn't build a session.
type fallbackEvent struct {
	provider types.ExecutionProvider
	err      error
}

// fallbackRecorder captures the library's report that the requested execution provider couldn't build a session and
// the model was rebuilt on the CPU. Without it a silent GPU->CPU downgrade would just look like a slow GPU.
//
// The handler runs on the goroutine performing the inference, but it is guarded anyway so it can't race the sweep
// goroutine reading the result.
type fallbackRecorder struct {
	mu    sync.Mutex
	event *fallbackEvent
}

func (f *fallbackRecorder) record(ep types.ExecutionProvider, err error) {
	f.mu.Lock()
	defer f.mu.Unlock()

	f.event = &fallbackEvent{provider: ep, err: err}
}

func (f *fallbackRecorder) reset() {
	f.mu.Lock()
	defer f.mu.Unlock()

	f.event = nil
}

func (f *fallbackRecorder) take() *fallbackEvent {
	f.mu.Lock()
	defer f.mu.Unlock()

	return f.event
}

// benchmark measures one model. It never terminates the process: a failure is returned in result.err so the sweep can
// carry on with the remaining models.
//
// observer is notified as the run progresses so a live view can show which phase the model is in; it may be nil.
func benchmark(
	ctx context.Context,
	e entry,
	input *types.ImageData,
	cfg config,
	rec *fallbackRecorder,
	observer runObserver,
) result {
	res := result{entry: e}

	// A fresh registry per model: every resident session is destroyed, so this model's first run really is a cold
	// start, GPU memory doesn't accumulate over a 14-model sweep, and the latched "this provider failed" flag is
	// cleared so a downgrade is attributed to the model that hit it. CleanRegistry takes the write side of
	// internal.InferenceMu, so it blocks until any in-flight inference has finished.
	//
	// The recorder must be reset at this exact point. Reset it earlier and the flag the library latched is still set,
	// so no downgrade is reported; don't reset it at all and the first downgrade is reported against every model that
	// follows.
	opai.CleanRegistry()
	rec.reset()

	// opai.Process short-circuits on the disk image cache, which is keyed by input.Hash plus the operations and
	// outlives the process (500 entries / 1 GB / 24 h). Reusing one hash would turn every run after the first into a
	// cache read, so each iteration gets a hash unique to this invocation. The pixels are untouched: every run does
	// identical work, only the cache key differs.
	//
	// With --cache off the library cache is disabled outright and this is moot, but it costs three lines and it is
	// what makes --cache measure five inferences instead of one inference and four cache reads.
	//
	// The restore is deferred rather than done at the end: the same *ImageData is threaded through every model that
	// follows, so an early return that leaked a mutated hash would silently change their cache keys too.
	base := input.Hash
	defer func() { input.Hash = base }()
	prefix := fmt.Sprintf("%s-perf-%s-%d", base, e.name, time.Now().UnixNano())

	observer.phase(phaseBuild, 0, 0)

	run, err := e.build(ctx, input, cfg)
	if err != nil {
		res.err = err
		return res
	}

	// Warm-up. This is also what downloads the model when it isn't on disk yet - which is exactly why the cold start
	// is measured after the warm-up instead of on the very first run: a multi-hundred-megabyte download would
	// otherwise dominate the number that is supposed to describe session construction.
	for i := range cfg.warmup {
		observer.phase(phaseWarmup, i+1, cfg.warmup)

		input.Hash = fmt.Sprintf("%s-w%d", prefix, i)
		if _, err = run(ctx, input); err != nil {
			res.err = fmt.Errorf("warm-up run %d: %w", i+1, err)
			return res
		}
	}

	// Drop the session the warm-up built so the next run pays the full construction cost: graph optimization plus the
	// provider's own compilation (the cuDNN algo search on CUDA, an engine build on TensorRT, an MLProgram compile on
	// CoreML). The gap between this and the steady state below is what makes keeping models resident worth it.
	opai.CleanRegistry()
	observer.phase(phaseCold, 0, 0)

	input.Hash = prefix + "-cold"
	start := time.Now()
	if _, err = run(ctx, input); err != nil {
		res.err = fmt.Errorf("cold-start run: %w", err)
		return res
	}
	res.cold = time.Since(start)

	res.runs = make([]time.Duration, 0, cfg.runs)

	for i := range cfg.runs {
		observer.phase(phaseRun, i+1, cfg.runs)

		input.Hash = fmt.Sprintf("%s-%d", prefix, i)

		start = time.Now()
		out, runErr := run(ctx, input)
		elapsed := time.Since(start)

		if runErr != nil {
			res.err = fmt.Errorf("run %d: %w", i+1, runErr)
			return res
		}

		res.runs = append(res.runs, elapsed)
		res.outcome = out

		observer.runDone(elapsed)
	}

	res.stats = computeStats(res.runs)
	res.fallback = rec.take()

	return res
}

// computeStats summarizes the timed runs.
//
// The standard deviation is the SAMPLE deviation (Bessel-corrected, n-1): the runs are a sample of the runs the
// machine could have produced, and at the default n=5 the population formula would understate the spread by about
// 11%. With a single run it is zero.
func computeStats(durations []time.Duration) stats {
	if len(durations) == 0 {
		return stats{}
	}

	// Sorted copy: the caller's slice is printed in run order by --verbose, so it must not be reordered.
	sorted := slices.Clone(durations)
	slices.Sort(sorted)

	// Accumulated as float64 nanoseconds and rounded once at the end, so the mean doesn't inherit the truncation of
	// an integer division.
	var sum float64
	for _, d := range sorted {
		sum += float64(d)
	}
	mean := sum / float64(len(sorted))

	var variance float64
	if len(sorted) > 1 {
		var squares float64
		for _, d := range sorted {
			diff := float64(d) - mean
			squares += diff * diff
		}
		variance = squares / float64(len(sorted)-1)
	}

	mid := len(sorted) / 2
	median := float64(sorted[mid])
	if len(sorted)%2 == 0 {
		median = (float64(sorted[mid-1]) + float64(sorted[mid])) / 2
	}

	return stats{
		min:    sorted[0],
		max:    sorted[len(sorted)-1],
		mean:   time.Duration(math.Round(mean)),
		median: time.Duration(math.Round(median)),
		stdDev: time.Duration(math.Round(math.Sqrt(variance))),
	}
}

// megapixelsPerSecond reports throughput against the INPUT megapixels and the MEDIAN run. Input, because it is the one
// thing every model has in common - a 4x upscaler's output is 16x its input, a denoiser's is 1x, and a detector has no
// image output at all. Median, because it isn't moved by a single scheduling hiccup.
func megapixelsPerSecond(bounds image.Rectangle, median time.Duration) float64 {
	if median <= 0 {
		return 0
	}

	megapixels := float64(bounds.Dx()*bounds.Dy()) / 1_000_000
	return megapixels / median.Seconds()
}
