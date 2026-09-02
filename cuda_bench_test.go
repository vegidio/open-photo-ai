//go:build cudabench

// Benchmark harness for the CUDA execution provider's per-model options.
//
// It is behind a build tag because it needs a CUDA GPU, the downloaded weights and several minutes; it is not part
// of the normal test run. Run it with:
//
//	go test -tags cudabench -run TestCuda -timeout 90m -v .
package opai

import (
	"bytes"
	"context"
	"fmt"
	"image"
	_ "image/jpeg"
	"maps"
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
	iutils "github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/detection"
	pub "github.com/vegidio/open-photo-ai/utils"
	ort "github.com/yalue/onnxruntime_go"
)

// baseCuda mirrors what internal/utils.cudaOptions ships today.
var baseCuda = map[string]string{
	"cudnn_conv_algo_search":       "EXHAUSTIVE",
	"cudnn_conv_use_max_workspace": "1",
	"device_id":                    "0",
	"do_copy_in_default_stream":    "1",
	"enable_cuda_graph":            "0",
	"gpu_mem_limit":                "0",
	"prefer_nhwc":                  "0",
}

// graph is one model's session shape: what to open, and how to build the tensors a Run needs.
//
// It exists because the two graphs measured here do not agree on either end - newyork is one input and three
// outputs, athens is two inputs and one - so the timing loop cannot assume a shape.
type graph struct {
	id      string
	inputs  []string
	outputs []string

	// tensors builds a fresh set of input and output tensors for one session.
	tensors func() (ins, outs []*ort.Tensor[float32], err error)

	// probe indexes the output whose contents are compared between configs.
	probe int
}

type benchConfig struct {
	name string

	// cuda holds the overrides applied on top of baseCuda.
	cuda map[string]string

	// sequential selects ORT's sequential execution mode.
	sequential bool

	// session holds raw session config entries.
	session map[string]string

	// intraOp, when above zero, sets the intra-op thread count.
	intraOp int
}

func (c benchConfig) cudaMap() map[string]string {
	out := maps.Clone(baseCuda)
	maps.Copy(out, c.cuda)

	return out
}

type benchResult struct {
	name string

	// samples holds the per-run cost in nanoseconds, derived by timing a block of runs and dividing. Timing each
	// run on its own does not work here: Go's clock on Windows quantises to about a millisecond, which on a 4ms
	// graph rounds every config to the same two values and hides anything smaller than 25%.
	samples []float64

	// firstOut and lastOut are the probe output of the first and the last timed run, so a config that returns
	// stale or zeroed data on later runs cannot pass on the strength of its first one.
	firstOut []float32
	lastOut  []float32
}

func (r benchResult) sorted() []float64 {
	s := append([]float64(nil), r.samples...)
	sort.Float64s(s)

	return s
}

func (r benchResult) median() float64  { return r.sorted()[len(r.samples)/2] }
func (r benchResult) fastest() float64 { return r.sorted()[0] }

// benchSession is one built ORT session plus the tensors it runs over.
type benchSession struct {
	sess      *ort.DynamicAdvancedSession
	opts      *ort.SessionOptions
	ins       []*ort.Tensor[float32]
	outs      []*ort.Tensor[float32]
	probe     int
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

	for _, t := range append(append([]*ort.Tensor[float32]{}, b.ins...), b.outs...) {
		if t != nil {
			t.Destroy()
		}
	}
}

func (b *benchSession) run() error {
	ins := make([]ort.Value, len(b.ins))
	for i, t := range b.ins {
		ins[i] = t
	}

	outs := make([]ort.Value, len(b.outs))
	for i, t := range b.outs {
		outs[i] = t
	}

	return b.sess.Run(ins, outs)
}

func (b *benchSession) probeData() []float32 { return copyOf(b.outs[b.probe]) }

func buildBenchSession(modelPath string, g graph, cfg benchConfig) (*benchSession, error) {
	start := time.Now()

	opts, err := ort.NewSessionOptions()
	if err != nil {
		return nil, err
	}

	cudaOpts, err := ort.NewCUDAProviderOptions()
	if err != nil {
		_ = opts.Destroy()
		return nil, err
	}
	defer cudaOpts.Destroy()

	if err = cudaOpts.Update(cfg.cudaMap()); err != nil {
		_ = opts.Destroy()
		return nil, err
	}

	if err = opts.AppendExecutionProviderCUDA(cudaOpts); err != nil {
		_ = opts.Destroy()
		return nil, err
	}

	mode := ort.ExecutionMode(ort.ExecutionModeParallel)
	if cfg.sequential {
		mode = ort.ExecutionModeSequential
	}

	if err = opts.SetExecutionMode(mode); err != nil {
		_ = opts.Destroy()
		return nil, err
	}

	if cfg.intraOp > 0 {
		if err = opts.SetIntraOpNumThreads(cfg.intraOp); err != nil {
			_ = opts.Destroy()
			return nil, err
		}
	}

	for k, v := range cfg.session {
		if err = opts.AddSessionConfigEntry(k, v); err != nil {
			_ = opts.Destroy()
			return nil, err
		}
	}

	sess, err := ort.NewDynamicAdvancedSession(modelPath, g.inputs, g.outputs, opts)
	if err != nil {
		_ = opts.Destroy()
		return nil, err
	}

	b := &benchSession{sess: sess, opts: opts, probe: g.probe, buildTime: time.Since(start)}

	if b.ins, b.outs, err = g.tensors(); err != nil {
		b.destroy()
		return nil, err
	}

	return b, nil
}

// region - Graphs

func sampleImage(t *testing.T) image.Image {
	t.Helper()

	raw, err := os.ReadFile(filepath.Join("cmd", "perf", "test.dat"))
	if err != nil {
		t.Fatalf("failed to read the sample image: %v", err)
	}

	img, _, err := image.Decode(bytes.NewReader(raw))
	if err != nil {
		t.Fatalf("failed to decode the sample image: %v", err)
	}

	return img
}

func newyorkGraph(t *testing.T, precision string) graph {
	t.Helper()

	in, _, _ := detection.PreprocessImage(sampleImage(t), detection.TargetSize)
	n := int64(detection.AnchorCount())

	return graph{
		id:      "dt_newyork_" + precision,
		inputs:  []string{"input"},
		outputs: []string{"loc", "conf", "landmarks"},
		probe:   1, // conf
		tensors: func() (ins, outs []*ort.Tensor[float32], err error) {
			input, err := ort.NewTensor(ort.NewShape(1, 3, 640, 640), in)
			if err != nil {
				return nil, nil, err
			}

			for _, shape := range []ort.Shape{
				ort.NewShape(1, n, 4), ort.NewShape(1, n, 2), ort.NewShape(1, n, 10),
			} {
				out, oErr := ort.NewEmptyTensor[float32](shape)
				if oErr != nil {
					return nil, nil, oErr
				}

				outs = append(outs, out)
			}

			return []*ort.Tensor[float32]{input}, outs, nil
		},
	}
}

// athensGraph feeds the restorer a 512x512 crop of the sample rather than a properly aligned face. The alignment is
// not exported, and it does not matter here: convolution timing does not depend on the pixel values, and the output
// checks are all comparisons between two configs given the same input.
func athensGraph(t *testing.T, precision string) graph {
	t.Helper()

	const tile = 512

	face := imaging.Resize(sampleImage(t), tile, tile, imaging.Lanczos)
	in := iutils.ImageToCHW(face, false, true)

	return graph{
		id:      "fr_athens_" + precision,
		inputs:  []string{"input", "weight"},
		outputs: []string{"output"},
		probe:   0,
		tensors: func() (ins, outs []*ort.Tensor[float32], err error) {
			input, err := ort.NewTensor(ort.NewShape(1, 3, tile, tile), in)
			if err != nil {
				return nil, nil, err
			}

			// 1.0 is the Fidelity the athens variant binds to this input.
			weight, err := ort.NewTensor(ort.NewShape(1), []float32{1.0})
			if err != nil {
				return nil, nil, err
			}

			out, err := ort.NewEmptyTensor[float32](ort.NewShape(1, 3, tile, tile))
			if err != nil {
				return nil, nil, err
			}

			return []*ort.Tensor[float32]{input, weight}, []*ort.Tensor[float32]{out}, nil
		},
	}
}

// endregion

func setupRuntime(t *testing.T) {
	t.Helper()

	ctx := context.Background()
	if err := Initialize(ctx, "open-photo-ai", nil); err != nil {
		t.Fatalf("failed to initialize: %v", err)
	}

	t.Cleanup(Destroy)

	for _, lib := range []string{"cuda", "cudnn"} {
		if err := pub.InitializeNvidiaLib(ctx, lib, nil); err != nil {
			t.Fatalf("failed to initialize %s: %v", lib, err)
		}
	}
}

func modelPathFor(t *testing.T, id string) string {
	t.Helper()

	dir, err := fs.MkUserConfigDir(internal.AppName, internal.ModelsDir)
	if err != nil {
		t.Fatalf("failed to resolve the models dir: %v", err)
	}

	p := filepath.Join(dir, id+".onnx")
	if _, err = os.Stat(p); err != nil {
		t.Fatalf("model %s is not on disk: %v", id, err)
	}

	return p
}

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

// firstLine trims an ORT error down to something a table can sit next to: the CUDA provider dumps the whole cuDNN
// frontend graph as JSON into the message, which is several thousand characters on one line.
func firstLine(s string) string {
	if i := strings.IndexAny(s, "\r\n"); i >= 0 {
		s = s[:i]
	}

	if len(s) > 160 {
		s = s[:160] + "..."
	}

	return s
}

func copyOf(t *ort.Tensor[float32]) []float32 {
	d := t.GetData()
	out := make([]float32, len(d))
	copy(out, d)

	return out
}

// sweep builds every config, then interleaves rounds over them so a GPU that drifts in clock or temperature moves
// every row rather than the ones that happen to run late.
//
// Each sample is a block of blockSize runs divided by blockSize, for the clock-resolution reason on benchResult.
func sweep(t *testing.T, g graph, configs []benchConfig, blockSize, blocks, rounds, warmup int) {
	t.Helper()

	path := modelPathFor(t, g.id)

	sessions := make([]*benchSession, 0, len(configs))
	kept := make([]benchConfig, 0, len(configs))

	for _, cfg := range configs {
		// ORT logs its provider diagnostics to stderr from C++, with nothing in the line saying which session
		// produced it. This marker is the only thing that makes the "running in Fallback mode" warnings below
		// attributable to a config.
		fmt.Fprintf(os.Stderr, "\n##### building %s / %s #####\n", g.id, cfg.name)

		s, err := buildBenchSession(path, g, cfg)
		if err != nil {
			t.Logf("SKIP %-26s build failed: %v", cfg.name, firstLine(err.Error()))
			continue
		}

		// The fallback warnings are emitted on the first Run, when cuDNN picks its algorithm, not at build - so
		// the warm-up has to happen under the marker too. A config that cannot run at all is dropped rather than
		// failing the sweep: that is a result about that option, not a broken harness.
		var runErr error

		for range warmup {
			if runErr = s.run(); runErr != nil {
				break
			}
		}

		if runErr != nil {
			t.Logf("SKIP %-26s first run failed: %v", cfg.name, firstLine(runErr.Error()))
			s.destroy()

			continue
		}

		t.Logf("built %-26s in %v", cfg.name, s.buildTime.Round(time.Millisecond))
		sessions = append(sessions, s)
		kept = append(kept, cfg)
	}

	fmt.Fprintf(os.Stderr, "\n##### timing %s #####\n", g.id)

	t.Cleanup(func() {
		for _, s := range sessions {
			s.destroy()
		}
	})

	results := make([]*benchResult, len(kept))
	for i := range kept {
		results[i] = &benchResult{name: kept[i].name}
	}

	for round := range rounds {
		for i, s := range sessions {
			for b := range blocks {
				start := time.Now()

				for range blockSize {
					if err := s.run(); err != nil {
						t.Fatalf("%s: run failed: %v", kept[i].name, firstLine(err.Error()))
					}
				}

				results[i].samples = append(results[i].samples, float64(time.Since(start))/float64(blockSize))

				if round == 0 && b == 0 {
					results[i].firstOut = s.probeData()
				}

				if round == rounds-1 && b == blocks-1 {
					results[i].lastOut = s.probeData()
				}
			}
		}
	}

	report(t, g.id, results)
}

func report(t *testing.T, modelID string, results []*benchResult) {
	t.Helper()

	baseline := results[0]
	base := baseline.median()

	var sb strings.Builder

	fmt.Fprintf(&sb, "\n=== %s (CUDA) - median of %d blocks ===\n", modelID, len(baseline.samples))
	fmt.Fprintf(&sb, "%-26s %11s %11s %9s %10s %10s\n",
		"config", "median", "min", "vs base", "maxdiff", "drift")

	for _, r := range results {
		fmt.Fprintf(&sb, "%-26s %9.3fms %9.3fms %+8.1f%% %10.2e %10.2e\n",
			r.name,
			r.median()/1e6,
			r.fastest()/1e6,
			(r.median()-base)/base*100,
			maxAbsDiff(r.lastOut, baseline.lastOut),
			maxAbsDiff(r.firstOut, r.lastOut))
	}

	t.Log(sb.String())
}

// region - New York

func nhwc(extra ...map[string]string) map[string]string {
	out := map[string]string{"prefer_nhwc": "1"}
	for _, e := range extra {
		maps.Copy(out, e)
	}

	return out
}

func newyorkConfigs() []benchConfig {
	return []benchConfig{
		{name: "baseline (shipping)"},
		{name: "nhwc (parallel)", cuda: nhwc()},
		{name: "sequential", sequential: true},
		{name: "seq+nhwc", sequential: true, cuda: nhwc()},
		{name: "seq+nhwc+heuristic", sequential: true,
			cuda: nhwc(map[string]string{"cudnn_conv_algo_search": "HEURISTIC"})},
		{name: "seq+nhwc+no_max_ws", sequential: true,
			cuda: nhwc(map[string]string{"cudnn_conv_use_max_workspace": "0"})},
		{name: "seq+nhwc+copy_off_def", sequential: true,
			cuda: nhwc(map[string]string{"do_copy_in_default_stream": "0"})},
		{name: "seq+nhwc+arena_same", sequential: true,
			cuda: nhwc(map[string]string{"arena_extend_strategy": "kSameAsRequested"})},
		{name: "seq+nhwc+unified_stream", sequential: true,
			cuda: nhwc(map[string]string{"use_ep_level_unified_stream": "1"})},
		{name: "seq+algo_heuristic", sequential: true, cuda: map[string]string{"cudnn_conv_algo_search": "HEURISTIC"}},
		{name: "seq+algo_default", sequential: true, cuda: map[string]string{"cudnn_conv_algo_search": "DEFAULT"}},
		{name: "seq+copy_off_default", sequential: true, cuda: map[string]string{"do_copy_in_default_stream": "0"}},
		{name: "seq+fuse_conv_bias", sequential: true, cuda: map[string]string{"fuse_conv_bias": "1"}},
		{name: "seq+tf32_off", sequential: true, cuda: map[string]string{"use_tf32": "0"}},
		{name: "seq+tunable", sequential: true, cuda: map[string]string{
			"tunable_op_enable": "1", "tunable_op_tuning_enable": "1"}},
		{name: "seq+intraop1", sequential: true, intraOp: 1},
		{name: "seq+no_spinning", sequential: true, session: map[string]string{
			"session.intra_op.allow_spinning": "0"}},
	}
}

func TestCudaSweepNewYorkFp32(t *testing.T) {
	setupRuntime(t)
	sweep(t, newyorkGraph(t, "fp32"), newyorkConfigs(), 20, 10, 3, 20)
}

func TestCudaSweepNewYorkFp16(t *testing.T) {
	setupRuntime(t)
	sweep(t, newyorkGraph(t, "fp16"), newyorkConfigs(), 20, 10, 3, 20)
}

// TestCudaNewYorkNHWCOutput checks the candidate against the shipping config through the decode the app actually
// uses, over repeated runs on one session.
//
// The sweep's maxdiff is on the raw conf tensor, which says nothing about whether a box moved; and a single-run
// comparison cannot see a session that goes wrong only from its second Run on - which is exactly how CUDA graph
// capture failed here before.
func TestCudaNewYorkNHWCOutput(t *testing.T) {
	setupRuntime(t)

	for _, precision := range []string{"fp32", "fp16"} {
		t.Run(precision, func(t *testing.T) {
			g := newyorkGraph(t, precision)
			path := modelPathFor(t, g.id)

			base, err := buildBenchSession(path, g, benchConfig{name: "baseline"})
			if err != nil {
				t.Fatalf("failed to build the baseline session: %v", err)
			}
			defer base.destroy()

			cand, err := buildBenchSession(path, g, benchConfig{name: "candidate", sequential: true, cuda: nhwc()})
			if err != nil {
				t.Fatalf("failed to build the candidate session: %v", err)
			}
			defer cand.destroy()

			var wantFaces, gotFaces []detection.Face

			for run := range 20 {
				if err = base.run(); err != nil {
					t.Fatalf("baseline run %d failed: %v", run, err)
				}
				if err = cand.run(); err != nil {
					t.Fatalf("candidate run %d failed: %v", run, err)
				}

				want := decodeFaces(base)
				got := decodeFaces(cand)

				if run == 0 {
					wantFaces, gotFaces = want, got
				}

				// Both sessions must also be stable in themselves: a config that returns a different answer on
				// every Run is unusable no matter how close it lands to the baseline on average.
				if !facesEqual(want, wantFaces, 0) {
					t.Fatalf("the baseline is not deterministic: run %d differs from run 0", run)
				}
				if !facesEqual(got, gotFaces, 0) {
					t.Fatalf("the candidate is not deterministic: run %d differs from run 0", run)
				}
			}

			if len(gotFaces) != len(wantFaces) {
				t.Fatalf("face count: got %d, want %d", len(gotFaces), len(wantFaces))
			}

			// A tenth of a pixel at 640x640, which is far below what the alignment downstream can resolve.
			if !facesEqual(gotFaces, wantFaces, 0.1) {
				t.Fatalf("faces differ by more than 0.1px\n got: %+v\nwant: %+v", gotFaces, wantFaces)
			}

			t.Logf("%s: %d faces, identical within 0.1px over 20 runs", precision, len(gotFaces))
		})
	}
}

func decodeFaces(s *benchSession) []detection.Face {
	return detection.PostProcessDetections(
		copyOf(s.outs[0]), copyOf(s.outs[1]), copyOf(s.outs[2]), 1024, 1024, 0.5)
}

func facesEqual(a, b []detection.Face, tol float32) bool {
	if len(a) != len(b) {
		return false
	}

	near := func(x, y float32) bool { return math.Abs(float64(x-y)) <= float64(tol) }

	for i := range a {
		if !near(a[i].BoundingBox.Min.X, b[i].BoundingBox.Min.X) ||
			!near(a[i].BoundingBox.Min.Y, b[i].BoundingBox.Min.Y) ||
			!near(a[i].BoundingBox.Max.X, b[i].BoundingBox.Max.X) ||
			!near(a[i].BoundingBox.Max.Y, b[i].BoundingBox.Max.Y) {
			return false
		}

		for j := range a[i].Landmarks {
			if !near(a[i].Landmarks[j].X, b[i].Landmarks[j].X) ||
				!near(a[i].Landmarks[j].Y, b[i].Landmarks[j].Y) {
				return false
			}
		}
	}

	return true
}

// endregion

// region - Athens

// athensConfigs isolates the execution mode against the layout athens actually ships at each precision, which is
// the only comparison that answers "should the variant set ExecutionMode": measuring sequential against an NCHW
// fp16 session would be measuring it against a config that does not exist.
func athensConfigs(shipsNHWC bool) []benchConfig {
	shipping := map[string]string{}
	if shipsNHWC {
		shipping = nhwc()
	}

	return []benchConfig{
		{name: "shipping (parallel)", cuda: shipping},
		{name: "shipping+sequential", cuda: shipping, sequential: true},
		{name: "other layout, parallel", cuda: flipNHWC(shipsNHWC)},
		{name: "other layout, sequential", cuda: flipNHWC(shipsNHWC), sequential: true},
	}
}

func flipNHWC(shipsNHWC bool) map[string]string {
	if shipsNHWC {
		return map[string]string{"prefer_nhwc": "0"}
	}

	return nhwc()
}

func TestCudaSweepAthensFp32(t *testing.T) {
	setupRuntime(t)
	// athens ships NCHW at fp32.
	sweep(t, athensGraph(t, "fp32"), athensConfigs(false), 10, 8, 3, 10)
}

func TestCudaSweepAthensFp16(t *testing.T) {
	setupRuntime(t)
	// athens ships NHWC at fp16.
	sweep(t, athensGraph(t, "fp16"), athensConfigs(true), 10, 8, 3, 10)
}

// endregion
