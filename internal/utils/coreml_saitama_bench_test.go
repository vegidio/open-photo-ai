//go:build coremlbench

// Sweeps for saitama's CoreML settings, reusing the harness in coreml_kyoto_bench_test.go.
//
// The export-comparison rows need the candidate re-export on disk beside the shipping one, as
// up_saitama_4x_{fp16,fp32}_v2.onnx. Rows whose model is missing are skipped rather than failing the run, so the
// compute-unit sweeps still work with only the published weights present.
//
//	go test -tags coremlbench -run TestCoreMLSaitama -timeout 90m -v ./internal/utils/
package utils

import (
	"math"

	"testing"

	"github.com/vegidio/open-photo-ai/types"
)

const (
	saitamaFp32   = "up_saitama_4x_fp32"
	saitamaFp16   = "up_saitama_4x_fp16"
	saitamaFp32V2 = "up_saitama_4x_fp32_v2"
	saitamaFp16V2 = "up_saitama_4x_fp16_v2"
)

// TestCoreMLSaitamaComputeUnits asks saitama the question kyoto was asked - which of the Mac's engines each precision
// wants - against the weights as published.
func TestCoreMLSaitamaComputeUnits(t *testing.T) {
	bootstrap(t)

	rows := []sweepRow{
		{name: "ALL", profile: EPProfile{}},
		{name: "CPUAndGPU", profile: EPProfile{CoreMLComputeUnits: CoreMLComputeUnitsCPUAndGPU}},
		{name: "CPUAndNeuralEngine", profile: EPProfile{
			CoreMLComputeUnits: CoreMLComputeUnitsCPUAndNeuralEngine}},
	}

	for _, precision := range []string{"fp32", "fp16"} {
		t.Run(precision, func(t *testing.T) {
			profileSweep(t, "up_saitama_4x_"+precision, rows, 3, 2, 6, 3)
		})
	}
}

// TestCoreMLSaitamaExport interleaves the two exports against each other under every compute-unit setting, in one
// sweep, which is the only way to compare them on this machine: run as two sweeps the thermal drift is larger than
// the difference.
func TestCoreMLSaitamaExportFp16(t *testing.T) {
	bootstrap(t)

	rows := []sweepRow{
		{name: "shipping/ALL", model: saitamaFp16, profile: EPProfile{}},
		{name: "shipping/CPUAndGPU", model: saitamaFp16, profile: EPProfile{
			CoreMLComputeUnits: CoreMLComputeUnitsCPUAndGPU}},
		{name: "shipping/CPUAndANE", model: saitamaFp16, profile: EPProfile{
			CoreMLComputeUnits: CoreMLComputeUnitsCPUAndNeuralEngine}},
		{name: "v2/ALL", model: saitamaFp16V2, profile: EPProfile{}},
		{name: "v2/CPUAndGPU", model: saitamaFp16V2, profile: EPProfile{
			CoreMLComputeUnits: CoreMLComputeUnitsCPUAndGPU}},
		{name: "v2/CPUAndANE", model: saitamaFp16V2, profile: EPProfile{
			CoreMLComputeUnits: CoreMLComputeUnitsCPUAndNeuralEngine}},
	}

	profileSweep(t, saitamaFp16, rows, 3, 2, 6, 3)
}

// TestCoreMLSaitamaExportFp32 is the same for fp32, minus the Neural Engine rows: CoreML bars an fp32 MLProgram from
// that unit, and asking anyway measured 1675ms against 86ms - a 20x regression that is not worth re-running here.
func TestCoreMLSaitamaExportFp32(t *testing.T) {
	bootstrap(t)

	rows := []sweepRow{
		{name: "shipping/ALL", model: saitamaFp32, profile: EPProfile{}},
		{name: "shipping/CPUAndGPU", model: saitamaFp32, profile: EPProfile{
			CoreMLComputeUnits: CoreMLComputeUnitsCPUAndGPU}},
		{name: "v2/ALL", model: saitamaFp32V2, profile: EPProfile{}},
		{name: "v2/CPUAndGPU", model: saitamaFp32V2, profile: EPProfile{
			CoreMLComputeUnits: CoreMLComputeUnitsCPUAndGPU}},
	}

	profileSweep(t, saitamaFp32, rows, 3, 2, 4, 3)
}

// TestCoreMLSaitamaExecutionMode measures the session-level mode on top of the compute units each precision is going
// to ship with - measuring it on the default ALL would measure CoreML's unstable placement instead - and on the CPU
// provider, which is where the graph really is a few hundred separate nodes rather than the one partition CoreML
// fuses it into.
func TestCoreMLSaitamaExecutionMode(t *testing.T) {
	bootstrap(t)

	cpu := types.ExecutionProviderCPU

	for _, tc := range []struct {
		model string
		units CoreMLComputeUnits
	}{
		{saitamaFp32V2, CoreMLComputeUnitsAll},
		{saitamaFp16V2, CoreMLComputeUnitsCPUAndNeuralEngine},
	} {
		t.Run(tc.model, func(t *testing.T) {
			rows := []sweepRow{
				{name: "coreml/parallel", profile: EPProfile{CoreMLComputeUnits: tc.units}},
				{name: "coreml/sequential", profile: EPProfile{
					CoreMLComputeUnits: tc.units, ExecutionMode: ExecutionModeSequential}},
				{name: "cpu/parallel", ep: cpu, profile: EPProfile{}},
				{name: "cpu/sequential", ep: cpu, profile: EPProfile{ExecutionMode: ExecutionModeSequential}},
			}

			profileSweep(t, tc.model, rows, 3, 2, 4, 2)
		})
	}
}

// TestCoreMLSaitamaOtherSettings covers the CoreML options saitama was measured against but does not set, so that
// "measured, and it is noise" stays reproducible rather than being only a claim in a comment.
func TestCoreMLSaitamaOtherSettings(t *testing.T) {
	bootstrap(t)

	units := map[string]CoreMLComputeUnits{saitamaFp32V2: CoreMLComputeUnitsAll, saitamaFp16V2: CoreMLComputeUnitsCPUAndNeuralEngine}

	for _, model := range []string{saitamaFp32V2, saitamaFp16V2} {
		t.Run(model, func(t *testing.T) {
			rows := []sweepRow{
				{name: "base", profile: EPProfile{CoreMLComputeUnits: units[model]}},
				{name: "base/fastprediction", profile: EPProfile{
					CoreMLComputeUnits: units[model], CoreMLSpecialization: CoreMLSpecializationFastPrediction}},
				{name: "base/lowprecaccum", profile: EPProfile{CoreMLComputeUnits: units[model]},
					extra: coreMLExtra{"AllowLowPrecisionAccumulationOnGPU": "1"}},
				{name: "base/subgraphs", profile: EPProfile{CoreMLComputeUnits: units[model]},
					extra: coreMLExtra{"EnableOnSubgraphs": "1"}},
				{name: "base/neuralnetwork", profile: EPProfile{CoreMLComputeUnits: units[model]},
					extra: coreMLExtra{"ModelFormat": "NeuralNetwork"}, skip: model == saitamaFp16V2},
			}

			profileSweep(t, model, rows, 3, 2, 5, 3)
		})
	}
}

// TestSaitamaOutputQuality scores every configuration saitama could ship against an fp32 CPU-provider reference,
// which is the closest thing to ground truth this harness can build without leaving Go.
//
// It reports rather than asserts, for the reason kyoto's equivalent does: a threshold here would either pass anything
// or fail on the next Mac, since how far CoreML's own reduced precision moves a result is a property of the hardware.
// What the numbers are for is ranking the candidates against each other.
func TestSaitamaOutputQuality(t *testing.T) {
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

	ref := out(t, saitamaFp32, sweepRow{ep: types.ExecutionProviderCPU})

	ane := EPProfile{CoreMLComputeUnits: CoreMLComputeUnitsCPUAndNeuralEngine}
	gpu := EPProfile{CoreMLComputeUnits: CoreMLComputeUnitsCPUAndGPU}

	for _, c := range []struct {
		name string
		id   string
		row  sweepRow
	}{
		{"fp32 shipping / coreml", saitamaFp32, sweepRow{}},
		{"fp32 v2 / cpu", saitamaFp32V2, sweepRow{ep: types.ExecutionProviderCPU}},
		{"fp32 v2 / coreml (ships)", saitamaFp32V2, sweepRow{}},
		{"fp16 shipping / cpu", saitamaFp16, sweepRow{ep: types.ExecutionProviderCPU}},
		{"fp16 shipping / coreml ALL", saitamaFp16, sweepRow{}},
		{"fp16 v2 / cpu", saitamaFp16V2, sweepRow{ep: types.ExecutionProviderCPU}},
		{"fp16 v2 / coreml ALL", saitamaFp16V2, sweepRow{}},
		{"fp16 v2 / coreml CPUAndGPU", saitamaFp16V2, sweepRow{profile: gpu}},
		{"fp16 v2 / coreml CPUAndANE (ships)", saitamaFp16V2, sweepRow{profile: ane}},
	} {
		got := out(t, c.id, c.row)

		if len(got) != len(ref) {
			t.Fatalf("%s: output has %d elements, want %d", c.name, len(got), len(ref))
		}

		var sse float64
		for i := range ref {
			d := float64(got[i]) - float64(ref[i])
			sse += d * d
		}

		t.Logf("%-36s maxdiff %.3e (%.3f/255)  PSNR %.1f dB", c.name,
			maxAbsDiff(ref, got), maxAbsDiff(ref, got)*255,
			10*math.Log10(1/(sse/float64(len(ref)))))
	}
}
