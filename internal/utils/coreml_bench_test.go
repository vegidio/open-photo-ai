//go:build coremlbench

// Benchmark harness for the session-level execution mode on the CoreML execution provider.
//
// It is behind a build tag because it needs a Mac with the weights already downloaded and several minutes; it is not
// part of the normal test run. Run it with:
//
//	go test -tags coremlbench -run TestCoreMLExecutionMode -timeout 90m -v ./internal/utils/
//
// It lives in this package rather than beside cuda_bench_test.go so that it can build its sessions through
// createOptions and applyProfile - the same two functions the app uses - and therefore measure the execution mode on
// top of each model's real shipping profile rather than on a hand-rebuilt approximation of it.
package utils

import (
	"fmt"
	"image"
	_ "image/jpeg"
	"math"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/disintegration/imaging"
	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

const samplePath = "../../cmd/perf/test.dat" // a 640x640 JPEG

// bootstrap brings up ONNX Runtime without going through the root package, which imports this one.
func bootstrap(t *testing.T) {
	t.Helper()

	internal.AppName = "open-photo-ai"

	modelData, err := LoadModelData()
	if err != nil {
		t.Fatalf("load model manifest: %v", err)
	}
	internal.ModelData = modelData

	dir, err := fs.MkUserConfigDir(internal.AppName, internal.RuntimeDir)
	if err != nil {
		t.Fatalf("runtime dir: %v", err)
	}

	pinned, found := internal.PinnedArchive("onnx")
	if !found {
		t.Fatalf("no ONNX Runtime is published for this platform")
	}

	ort.SetSharedLibraryPath(filepath.Join(dir, pinned.Lib))
	if err = ort.InitializeEnvironment(); err != nil {
		t.Fatalf("initialize ONNX Runtime: %v", err)
	}

	t.Cleanup(func() { _ = ort.DestroyEnvironment() })
}

func modelPathFor(t *testing.T, id string) string {
	t.Helper()

	dir, err := fs.MkUserConfigDir(internal.AppName, internal.ModelsDir)
	if err != nil {
		t.Fatalf("models dir: %v", err)
	}

	p := filepath.Join(dir, id+".onnx")
	if _, err = os.Stat(p); err != nil {
		t.Fatalf("model %s is not on disk: %v", id, err)
	}

	return p
}

// sampleInput returns the shared sample image resized to width x height as a CHW float32 tensor in [0,1].
//
// The content barely moves a timing, but using the same image every model is benchmarked against keeps this
// comparable with the numbers in the model profiles, which all come from this file.
func sampleInput(t *testing.T, width, height int) []float32 {
	t.Helper()

	f, err := os.Open(samplePath)
	if err != nil {
		t.Fatalf("open sample: %v", err)
	}
	defer f.Close()

	img, _, err := image.Decode(f)
	if err != nil {
		t.Fatalf("decode sample: %v", err)
	}

	resized := imaging.Resize(img, width, height, imaging.Lanczos)
	out := make([]float32, 3*width*height)
	plane := width * height

	for y := range height {
		for x := range width {
			r, g, b, _ := resized.At(x, y).RGBA()
			i := y*width + x
			out[i] = float32(r>>8) / 255
			out[plane+i] = float32(g>>8) / 255
			out[2*plane+i] = float32(b>>8) / 255
		}
	}

	return out
}

// scalarInputValue is what a non-image input is filled with. The only one in the catalogue is athens' fidelity
// weight, whose shipping value is 1.0 - and a graph is timed on the path its real input takes, so this is not an
// arbitrary filler.
const scalarInputValue float32 = 1.0

// inputData builds the data for one of a graph's inputs: the sample image for a 4-D NCHW image input, and
// scalarInputValue for anything else.
func inputData(t *testing.T, info ort.InputOutputInfo) ([]float32, error) {
	t.Helper()

	elements := int64(1)

	for _, d := range info.Dimensions {
		if d <= 0 {
			return nil, fmt.Errorf("input %q has a dynamic shape %v, which this harness does not cover",
				info.Name, info.Dimensions)
		}

		elements *= d
	}

	if len(info.Dimensions) != 4 {
		out := make([]float32, elements)
		for i := range out {
			out[i] = scalarInputValue
		}

		return out, nil
	}

	return sampleInput(t, int(info.Dimensions[3]), int(info.Dimensions[2])), nil
}

// benchSession is one built ORT session plus the tensors it runs over.
type benchSession struct {
	sess      *ort.DynamicAdvancedSession
	opts      *ort.SessionOptions
	in        []ort.Value
	out       []ort.Value
	buildTime time.Duration
}

func (b *benchSession) destroy() {
	if b == nil {
		return
	}

	if b.sess != nil {
		_ = b.sess.Destroy()
	}
	if b.opts != nil {
		_ = b.opts.Destroy()
	}

	for _, v := range append(append([]ort.Value{}, b.in...), b.out...) {
		if v != nil {
			_ = v.Destroy()
		}
	}
}

func (b *benchSession) run() error { return b.sess.Run(b.in, b.out) }

// firstOutput copies the first output tensor's data, which is what the two modes are compared on.
func (b *benchSession) firstOutput() []float32 {
	tensor, ok := b.out[0].(*ort.Tensor[float32])
	if !ok {
		return nil
	}

	d := tensor.GetData()
	out := make([]float32, len(d))
	copy(out, d)

	return out
}

// buildBenchSession builds one CoreML session for modelPath under the given profile, allocating its input and output
// tensors from the shapes the graph itself declares.
func buildBenchSession(t *testing.T, modelPath string, p EPProfile) (*benchSession, error) {
	t.Helper()

	inputInfo, outputInfo, err := ort.GetInputOutputInfo(modelPath)
	if err != nil {
		return nil, err
	}

	start := time.Now()

	cachePath, err := fs.MkUserConfigDir(internal.AppName, internal.ModelsDir)
	if err != nil {
		return nil, err
	}

	opts, err := createOptions("darwin", cachePath, types.ExecutionProviderCoreML, p)
	if err != nil {
		return nil, err
	}

	if err = applyProfile(opts, p); err != nil {
		_ = opts.Destroy()
		return nil, err
	}

	inNames := make([]string, len(inputInfo))
	outNames := make([]string, len(outputInfo))

	b := &benchSession{opts: opts}

	for i, info := range inputInfo {
		inNames[i] = info.Name

		data, derr := inputData(t, info)
		if derr != nil {
			b.destroy()
			return nil, derr
		}

		tensor, terr := ort.NewTensor(info.Dimensions, data)
		if terr != nil {
			b.destroy()
			return nil, terr
		}

		b.in = append(b.in, tensor)
	}

	// The outputs are left nil so ORT allocates them on the first Run. Declaring them here would mean hard-coding
	// each graph's output shape, which is the one thing a harness driven by GetInputOutputInfo does not need to know.
	for i, info := range outputInfo {
		outNames[i] = info.Name
	}

	b.out = make([]ort.Value, len(outNames))

	sess, err := ort.NewDynamicAdvancedSession(modelPath, inNames, outNames, opts)
	if err != nil {
		b.destroy()
		return nil, err
	}

	b.sess = sess
	b.buildTime = time.Since(start)

	return b, nil
}

// benchConfig is one row of a sweep: an execution mode and the label it is reported under.
type benchConfig struct {
	name string
	mode ExecutionMode
}

func modeName(m ExecutionMode) string {
	if m == ExecutionModeSequential {
		return "sequential"
	}

	return "parallel"
}

type benchResult struct {
	name    string
	samples []float64
	first   []float32
	last    []float32
}

func (r benchResult) sorted() []float64 {
	s := append([]float64(nil), r.samples...)
	sort.Float64s(s)

	return s
}

func (r benchResult) median() float64  { return r.sorted()[len(r.samples)/2] }
func (r benchResult) fastest() float64 { return r.sorted()[0] }

func maxAbsDiff(a, b []float32) float64 {
	if len(a) != len(b) {
		return math.Inf(1)
	}

	var worst float64

	for i := range a {
		if d := math.Abs(float64(a[i]) - float64(b[i])); d > worst {
			worst = d
		}
	}

	return worst
}

// modeSweep times the parallel and the sequential mode against each other on one model, interleaving rounds so that a
// machine that drifts in clock or temperature moves both rows rather than whichever one happens to run late.
//
// That interleaving is not a formality here. Measuring the two modes in separate processes, one after the other, put
// tokyo's 12.2-second end-to-end run at 31 seconds two rounds later on a thermally loaded Mac - a drift far larger
// than the effect being measured.
func modeSweep(t *testing.T, modelID string, profile EPProfile, blockSize, blocks, rounds, warmup int) {
	t.Helper()

	modeSweepOrdered(t, modelID, profile, []ExecutionMode{ExecutionModeParallel, ExecutionModeSequential},
		blockSize, blocks, rounds, warmup)
}

// modeSweepOrdered is modeSweep with the build order made explicit, so that the order the two sessions are created in
// can itself be controlled for.
//
// It has to be: CoreML caches the model it compiles, and the second session of a pair builds in a fraction of the
// first one's time (40ms against 525ms on newyork). That is a cold-start effect rather than a steady-state one, but
// nothing in the timings themselves would show it if it were not.
func modeSweepOrdered(t *testing.T, modelID string, profile EPProfile, modes []ExecutionMode,
	blockSize, blocks, rounds, warmup int,
) {
	t.Helper()

	path := modelPathFor(t, modelID)

	configs := make([]benchConfig, 0, len(modes))

	for _, m := range modes {
		configs = append(configs, benchConfig{name: modeName(m), mode: m})
	}

	sessions := make([]*benchSession, 0, len(configs))
	results := make([]*benchResult, 0, len(configs))

	for _, cfg := range configs {
		p := profile
		p.ExecutionMode = cfg.mode

		s, err := buildBenchSession(t, path, p)
		if err != nil {
			t.Fatalf("%s/%s: build failed: %v", modelID, cfg.name, err)
		}

		for range warmup {
			if err = s.run(); err != nil {
				t.Fatalf("%s/%s: warm-up run failed: %v", modelID, cfg.name, err)
			}
		}

		t.Logf("built %-12s in %v", cfg.name, s.buildTime.Round(time.Millisecond))
		sessions = append(sessions, s)
		results = append(results, &benchResult{name: cfg.name})
	}

	t.Cleanup(func() {
		for _, s := range sessions {
			s.destroy()
		}
	})

	// The session order alternates every round. Interleaving at round granularity alone is not enough: within a
	// round the second session always runs after the first, so any warming inside a round is charged entirely to
	// whichever session is second, which is a systematic bias of about the size of the effect being measured.
	// Alternating makes each session run first in half the rounds, so that bias cancels instead of accumulating - which
	// is why every caller passes an EVEN round count. An odd one leaves the extra round's position uncompensated.
	for round := range rounds {
		for pos := range sessions {
			i := pos
			if round%2 == 1 {
				i = len(sessions) - 1 - pos
			}

			s := sessions[i]

			for b := range blocks {
				start := time.Now()

				for range blockSize {
					if err := s.run(); err != nil {
						t.Fatalf("%s/%s: run failed: %v", modelID, results[i].name, err)
					}
				}

				results[i].samples = append(results[i].samples, float64(time.Since(start))/float64(blockSize))

				if round == 0 && b == 0 {
					results[i].first = s.firstOutput()
				}

				if round == rounds-1 && b == blocks-1 {
					results[i].last = s.firstOutput()
				}
			}
		}
	}

	baseline := results[0]
	base := baseline.median()

	var sb strings.Builder

	fmt.Fprintf(&sb, "\n=== %s (CoreML) - median of %d blocks of %d runs ===\n", modelID, len(baseline.samples),
		blockSize)
	fmt.Fprintf(&sb, "%-12s %11s %11s %9s %10s %10s\n", "mode", "median", "min", "vs base", "maxdiff", "drift")

	for _, r := range results {
		fmt.Fprintf(&sb, "%-12s %10.3fms %10.3fms %+8.1f%% %10.2e %10.2e\n",
			r.name,
			r.median()/1e6,
			r.fastest()/1e6,
			(r.median()-base)/base*100,
			maxAbsDiff(r.last, baseline.last),
			maxAbsDiff(r.first, r.last))
	}

	t.Log(sb.String())
}

// region - Shipping profiles
//
// These mirror what each model's variant declares today, minus the ExecutionMode the sweep is varying. They are
// copied rather than imported because the model packages import this one.

func tokyoProfile() EPProfile {
	return EPProfile{CoreMLComputeUnits: CoreMLComputeUnitsCPUAndGPU}
}

func santoriniProfile() EPProfile {
	return EPProfile{CoreMLSpecialization: CoreMLSpecializationFastPrediction}
}

func newyorkProfile() EPProfile { return EPProfile{} }

func athensProfile() EPProfile {
	return EPProfile{CoreMLComputeUnits: CoreMLComputeUnitsCPUAndGPU}
}

// endregion

func TestCoreMLExecutionModeTokyo(t *testing.T) {
	bootstrap(t)

	for _, precision := range []string{"fp32", "fp16"} {
		t.Run(precision, func(t *testing.T) {
			modeSweep(t, "up_tokyo_4x_"+precision, tokyoProfile(), 3, 2, 4, 2)
		})
	}
}

func TestCoreMLExecutionModeSantorini(t *testing.T) {
	bootstrap(t)

	for _, precision := range []string{"fp32", "fp16"} {
		t.Run(precision, func(t *testing.T) {
			modeSweep(t, "fr_santorini_"+precision, santoriniProfile(), 10, 3, 4, 5)
		})
	}
}

func TestCoreMLExecutionModeNewYork(t *testing.T) {
	bootstrap(t)

	for _, precision := range []string{"fp32", "fp16"} {
		t.Run(precision, func(t *testing.T) {
			modeSweep(t, "dt_newyork_"+precision, newyorkProfile(), 20, 5, 4, 20)
		})
	}
}

// TestCoreMLExecutionModeAthens sweeps athens in BOTH build orders, which the other models do not need.
//
// Athens is the one model here whose setting is still open: the other three already ship sequential on the strength
// of what it is worth on CUDA, so their CoreML sweeps only have to show it costs nothing. Athens ships parallel, so
// a CoreML number is the only evidence there is, and a margin of a percent is exactly the size of the compile-cache
// bias described on ExecutionMode. Running both orders makes that bias cancel: a real effect keeps its sign when the
// build order is reversed, and an artifact does not.
func TestCoreMLExecutionModeAthens(t *testing.T) {
	bootstrap(t)

	orders := map[string][]ExecutionMode{
		"parallel_first":   {ExecutionModeParallel, ExecutionModeSequential},
		"sequential_first": {ExecutionModeSequential, ExecutionModeParallel},
	}

	for _, precision := range []string{"fp32", "fp16"} {
		for _, name := range []string{"parallel_first", "sequential_first"} {
			t.Run(precision+"/"+name, func(t *testing.T) {
				modeSweepOrdered(t, "fr_athens_"+precision, athensProfile(), orders[name], 10, 3, 4, 5)
			})
		}
	}
}

// TestCoreMLExecutionModeBuildOrder controls for the build order on the one row where sequential measured as a small
// but repeatable loss. If the ordering were what produced it, reversing it would reverse the sign.
func TestCoreMLExecutionModeBuildOrder(t *testing.T) {
	bootstrap(t)

	modeSweepOrdered(t, "dt_newyork_fp32", newyorkProfile(),
		[]ExecutionMode{ExecutionModeSequential, ExecutionModeParallel}, 20, 5, 4, 20)
}
