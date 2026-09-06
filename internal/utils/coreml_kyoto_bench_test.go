//go:build coremlbench

// Sweeps for kyoto's CoreML settings, and the general profile-sweep harness they use.
//
// coreml_bench_test.go varies one thing - the execution mode - on top of a model's shipping profile. Kyoto's
// question was which compute units its graphs want, so this file generalises that into a sweep over whole
// EPProfiles, over raw CoreML provider keys that EPProfile has no field for, over providers other than CoreML, and
// over two model files at once. The anti-bias machinery is the same idea as modeSweepOrdered's and is there for the
// same reason: see ExecutionMode in ep_profile.go for what thermal drift on this hardware does to a naive sweep.
//
// It needs a Mac with the weights already on disk. Run one of:
//
//	go test -tags coremlbench -run TestCoreMLKyotoANE -timeout 90m -v ./internal/utils/
//	go test -tags coremlbench -run TestCoreMLKyotoComputeUnits -timeout 90m -v ./internal/utils/
//	go test -tags coremlbench -run TestCPUKyotoExecutionMode -timeout 90m -v ./internal/utils/
//	go test -tags coremlbench -run TestKyotoOutputQuality -timeout 90m -v ./internal/utils/
package utils

import (
	"fmt"
	"math"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/vegidio/go-sak/fs"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

// coreMLExtra lets a sweep row set raw CoreML provider keys that EPProfile has no typed field for.
type coreMLExtra map[string]string

// sweepRow is one configuration under test.
type sweepRow struct {
	name    string
	profile EPProfile
	extra   coreMLExtra

	// ep overrides the provider the row is built on. The zero value is CoreML, which is what this file is for;
	// the CPU provider matters because the execution mode is a session setting rather than a CoreML one.
	ep types.ExecutionProvider

	// model overrides the sweep's model id, so two exports of the same graph can be interleaved against each
	// other in one sweep rather than compared across two - which on this machine measures the thermal drift
	// rather than the export.
	model string
}

// buildRowSession is buildBenchSession with raw CoreML keys layered on top of the profile's own.
func buildRowSession(t *testing.T, modelPath string, row sweepRow) (*benchSession, error) {
	t.Helper()

	if row.ep != "" && row.ep != types.ExecutionProviderCoreML {
		return buildProviderSession(t, modelPath, row)
	}

	if len(row.extra) == 0 {
		return buildBenchSession(t, modelPath, row.profile)
	}

	inputInfo, outputInfo, err := ort.GetInputOutputInfo(modelPath)
	if err != nil {
		return nil, err
	}

	start := time.Now()

	cachePath, err := fs.MkUserConfigDir(internal.AppName, internal.ModelsDir)
	if err != nil {
		return nil, err
	}

	opts, err := ort.NewSessionOptions()
	if err != nil {
		return nil, err
	}

	b := &benchSession{opts: opts}

	coreml := coreMLOptions(cachePath, row.profile)
	for k, v := range row.extra {
		coreml[k] = v
	}

	if err = opts.AppendExecutionProviderCoreMLV2(coreml); err != nil {
		b.destroy()
		return nil, err
	}

	if err = applyProfile(opts, row.profile); err != nil {
		b.destroy()
		return nil, err
	}

	inNames := make([]string, len(inputInfo))
	outNames := make([]string, len(outputInfo))

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

// buildProviderSession builds a row on a provider other than CoreML, so the session-level settings can be measured
// where they are not hidden behind a single fused CoreML node.
func buildProviderSession(t *testing.T, modelPath string, row sweepRow) (*benchSession, error) {
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

	opts, err := createOptions("darwin", cachePath, row.ep, row.profile)
	if err != nil {
		return nil, err
	}

	b := &benchSession{opts: opts}

	if err = applyProfile(opts, row.profile); err != nil {
		b.destroy()
		return nil, err
	}

	inNames := make([]string, len(inputInfo))
	outNames := make([]string, len(outputInfo))

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

// profileSweep times several CoreML configurations against each other on one model, rotating the run order every
// round so a drifting machine moves every row rather than whichever happens to run late.
func profileSweep(t *testing.T, modelID string, rows []sweepRow, blockSize, blocks, rounds, warmup int) {
	t.Helper()

	sessions := make([]*benchSession, 0, len(rows))
	results := make([]*benchResult, 0, len(rows))

	for _, row := range rows {
		id := modelID
		if row.model != "" {
			id = row.model
		}

		s, err := buildRowSession(t, modelPathFor(t, id), row)
		if err != nil {
			t.Logf("SKIP %-28s build failed: %v", row.name, err)
			continue
		}

		ok := true
		for range warmup {
			if err = s.run(); err != nil {
				t.Logf("SKIP %-28s warm-up failed: %v", row.name, err)
				s.destroy()
				ok = false
				break
			}
		}
		if !ok {
			continue
		}

		t.Logf("built %-28s in %v", row.name, s.buildTime.Round(time.Millisecond))
		sessions = append(sessions, s)
		results = append(results, &benchResult{name: row.name})
	}

	if len(sessions) == 0 {
		t.Fatalf("%s: no configuration built", modelID)
	}

	t.Cleanup(func() {
		for _, s := range sessions {
			s.destroy()
		}
	})

	// Rotate the starting position each round so every row runs first an equal share of the time; with n rows this
	// wants a round count that is a multiple of n for the position bias to cancel exactly.
	for round := range rounds {
		for pos := range sessions {
			i := (pos + round) % len(sessions)
			s := sessions[i]

			for b := range blocks {
				start := time.Now()

				for range blockSize {
					if err := s.run(); err != nil {
						t.Fatalf("%s/%s: run failed: %v", modelID, results[i].name, err)
					}
				}

				results[i].samples = append(results[i].samples, float64(time.Since(start))/float64(blockSize))

				if results[i].first == nil && b == 0 {
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

	fmt.Fprintf(&sb, "\n=== %s (CoreML) - median of %d blocks of %d runs ===\n", modelID,
		len(baseline.samples), blockSize)
	fmt.Fprintf(&sb, "%-28s %11s %11s %9s %10s %10s\n", "config", "median", "min", "vs base", "maxdiff", "drift")

	ranked := append([]*benchResult(nil), results...)
	sort.SliceStable(ranked, func(a, b int) bool { return ranked[a].median() < ranked[b].median() })

	for _, r := range ranked {
		fmt.Fprintf(&sb, "%-28s %10.3fms %10.3fms %+8.1f%% %10.2e %10.2e\n",
			r.name, r.median()/1e6, r.fastest()/1e6, (r.median()-base)/base*100,
			maxAbsDiff(r.last, baseline.last), maxAbsDiff(r.first, r.last))
	}
	fmt.Fprintf(&sb, "(baseline = %s)\n", baseline.name)

	t.Log(sb.String())
}

// TestCoreMLKyotoANE is the measurement kyoto's profile is built on: which engine each fp16 pass wants, run long
// enough to separate the rows from the machine.
func TestCoreMLKyotoANE(t *testing.T) {
	bootstrap(t)

	rows := []sweepRow{
		{name: "ALL", profile: EPProfile{}},
		{name: "CPUAndGPU", profile: EPProfile{CoreMLComputeUnits: CoreMLComputeUnitsCPUAndGPU}},
		{name: "CPUAndNeuralEngine", profile: EPProfile{
			CoreMLComputeUnits: CoreMLComputeUnitsCPUAndNeuralEngine}},
	}

	for _, scale := range []string{"4x", "2x"} {
		t.Run(scale, func(t *testing.T) {
			profileSweep(t, "up_kyoto_"+scale+"_fp16", rows, 5, 4, 9, 5)
		})
	}
}

// TestCoreMLKyotoComputeUnits is the same sweep across both precisions and both scales, shorter. Its fp32 rows are
// the ones that show why the profile is conditional on the precision: CoreML bars an fp32 MLProgram from the Neural
// Engine, so asking for it there drops the graph onto the CPU instead.
func TestCoreMLKyotoComputeUnits(t *testing.T) {
	bootstrap(t)

	rows := []sweepRow{
		{name: "ALL", profile: EPProfile{}},
		{name: "CPUAndGPU", profile: EPProfile{CoreMLComputeUnits: CoreMLComputeUnitsCPUAndGPU}},
		{name: "CPUAndNeuralEngine", profile: EPProfile{
			CoreMLComputeUnits: CoreMLComputeUnitsCPUAndNeuralEngine}},
	}

	for _, scale := range []string{"2x", "4x"} {
		for _, precision := range []string{"fp16", "fp32"} {
			t.Run(scale+"/"+precision, func(t *testing.T) {
				profileSweep(t, "up_kyoto_"+scale+"_"+precision, rows, 3, 2, 6, 2)
			})
		}
	}
}

// TestCoreMLKyotoOtherSettings covers the CoreML options kyoto was measured against and does not set, so that
// "measured, and it is noise" stays reproducible rather than being only a claim in a comment.
func TestCoreMLKyotoOtherSettings(t *testing.T) {
	bootstrap(t)

	ane := CoreMLComputeUnitsCPUAndNeuralEngine

	rows := []sweepRow{
		{name: "ane", profile: EPProfile{CoreMLComputeUnits: ane}},
		{name: "ane/fastprediction", profile: EPProfile{
			CoreMLComputeUnits: ane, CoreMLSpecialization: CoreMLSpecializationFastPrediction}},
		{name: "ane/sequential", profile: EPProfile{
			CoreMLComputeUnits: ane, ExecutionMode: ExecutionModeSequential}},
		{name: "ane/lowprecaccum", profile: EPProfile{CoreMLComputeUnits: ane},
			extra: coreMLExtra{"AllowLowPrecisionAccumulationOnGPU": "1"}},
	}

	profileSweep(t, "up_kyoto_4x_fp16", rows, 3, 2, 8, 2)
}

// TestCPUKyotoExecutionMode measures the execution mode on the CPU provider, where the graph is a thousand separate
// nodes rather than the one fused node CoreML turns it into - which is the only place the inter-op pool has
// anything to schedule at all, and so the only place the setting tokyo carries could pay for itself here.
func TestCPUKyotoExecutionMode(t *testing.T) {
	bootstrap(t)

	cpu := types.ExecutionProviderCPU

	rows := []sweepRow{
		{name: "cpu/parallel", ep: cpu, profile: EPProfile{}},
		{name: "cpu/sequential", ep: cpu, profile: EPProfile{ExecutionMode: ExecutionModeSequential}},
	}

	for _, precision := range []string{"fp32", "fp16"} {
		t.Run(precision, func(t *testing.T) {
			profileSweep(t, "up_kyoto_4x_"+precision, rows, 2, 2, 4, 1)
		})
	}
}

// TestKyotoOutputQuality scores every configuration kyoto could ship against an fp32 CPU-provider reference, which
// is the closest thing to ground truth this harness can build without leaving Go.
//
// It reports rather than asserts. A threshold here would either be loose enough to pass anything or would fail on
// the next Mac, since how far CoreML's own reduced precision moves a result is a property of the hardware; what the
// numbers are for is comparing the candidates against each other, where a configuration that is much worse than its
// siblings is the signal worth having.
func TestKyotoOutputQuality(t *testing.T) {
	bootstrap(t)

	out := func(t *testing.T, id string, row sweepRow) []float32 {
		t.Helper()

		s, err := buildRowSession(t, modelPathFor(t, id), row)
		if err != nil {
			t.Fatalf("%s: %v", id, err)
		}
		defer s.destroy()

		if err = s.run(); err != nil {
			t.Fatalf("%s: %v", id, err)
		}

		return s.firstOutput()
	}

	for _, scale := range []string{"2x", "4x"} {
		t.Run(scale, func(t *testing.T) {
			ref := out(t, "up_kyoto_"+scale+"_fp32", sweepRow{ep: types.ExecutionProviderCPU})

			fp16 := "up_kyoto_" + scale + "_fp16"

			cands := []struct {
				name string
				id   string
				row  sweepRow
			}{
				{"fp32 / coreml", "up_kyoto_" + scale + "_fp32", sweepRow{}},
				{"fp16 / cpu", fp16, sweepRow{ep: types.ExecutionProviderCPU}},
				{"fp16 / coreml ALL", fp16, sweepRow{}},
				{"fp16 / coreml CPUAndGPU", fp16, sweepRow{
					profile: EPProfile{CoreMLComputeUnits: CoreMLComputeUnitsCPUAndGPU}}},
				{"fp16 / coreml CPUAndANE (ships)", fp16, sweepRow{
					profile: EPProfile{CoreMLComputeUnits: CoreMLComputeUnitsCPUAndNeuralEngine}}},
			}

			for _, c := range cands {
				got := out(t, c.id, c.row)

				if len(got) != len(ref) {
					t.Fatalf("%s: output has %d elements, want %d", c.name, len(got), len(ref))
				}

				var sse float64
				for i := range ref {
					d := float64(got[i]) - float64(ref[i])
					sse += d * d
				}

				t.Logf("%-32s maxdiff %.3e (%.2f/255)  PSNR %.1f dB", c.name,
					maxAbsDiff(ref, got), maxAbsDiff(ref, got)*255,
					10*math.Log10(1/(sse/float64(len(ref)))))
			}
		})
	}
}
