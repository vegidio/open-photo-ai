package utils

import (
	"reflect"
	"testing"

	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
)

func TestCoreMLOptionsFollowTheProfile(t *testing.T) {
	tests := []struct {
		name    string
		profile EPProfile
		want    string
	}{
		{"the zero value keeps the shipped behaviour", EPProfile{}, "1"},
		{"a dynamic-shape model relaxes it", EPProfile{DynamicShapes: true}, "0"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := coreMLOptions(testPaths, tt.profile)

			if got["RequireStaticInputShapes"] != tt.want {
				t.Fatalf("RequireStaticInputShapes = %q, want %q", got["RequireStaticInputShapes"], tt.want)
			}
			if got["ModelCacheDirectory"] != "/cache" {
				t.Fatalf("ModelCacheDirectory = %q", got["ModelCacheDirectory"])
			}
			if got["ModelFormat"] != "MLProgram" {
				t.Fatalf("ModelFormat = %q", got["ModelFormat"])
			}
		})
	}
}

func TestCoreMLComputeUnitsFollowTheProfile(t *testing.T) {
	tests := []struct {
		name    string
		profile EPProfile
		want    string
	}{
		{"the zero value keeps the shipped behaviour", EPProfile{}, "ALL"},
		{"a model the Neural Engine handles badly opts out",
			EPProfile{CoreMLComputeUnits: CoreMLComputeUnitsCPUAndGPU}, "CPUAndGPU"},
		{"the GPU can be left for other work",
			EPProfile{CoreMLComputeUnits: CoreMLComputeUnitsCPUAndNeuralEngine}, "CPUAndNeuralEngine"},
		{"the diagnostic setting pins the partition to the CPU",
			EPProfile{CoreMLComputeUnits: CoreMLComputeUnitsCPUOnly}, "CPUOnly"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := coreMLOptions(testPaths, tt.profile)["MLComputeUnits"]; got != tt.want {
				t.Fatalf("MLComputeUnits = %q, want %q", got, tt.want)
			}
		})
	}
}

func TestCoreMLSpecializationFollowsTheProfile(t *testing.T) {
	tests := []struct {
		name    string
		profile EPProfile
		want    string
	}{
		{"the zero value keeps the shipped behaviour", EPProfile{}, "Default"},
		{"a fixed-shape model can specialise for latency",
			EPProfile{CoreMLSpecialization: CoreMLSpecializationFastPrediction}, "FastPrediction"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := coreMLOptions(testPaths, tt.profile)["SpecializationStrategy"]; got != tt.want {
				t.Fatalf("SpecializationStrategy = %q, want %q", got, tt.want)
			}
		})
	}
}

// testPaths stands in for the two directories createSessionInner resolves. They differ from one another on purpose:
// the engine directory is per model and the timing one is shared, and a test that passed the same string for both
// could not catch them being swapped.
var testPaths = cachePaths{engine: "/cache", timing: "/timing"}

func TestTensorRTOptionsFollowTheProfile(t *testing.T) {
	zero := tensorRTOptions(testPaths, EPProfile{})

	if zero["trt_max_workspace_size"] != "4294967296" {
		t.Fatalf("default workspace = %q", zero["trt_max_workspace_size"])
	}
	if zero["trt_fp16_enable"] != "0" {
		t.Fatalf("fp16 must be opt-in, got %q", zero["trt_fp16_enable"])
	}

	// The timing cache has to point at the shared directory rather than this model's engine one. Pointed at the
	// engine directory it would still work, and still be cleared by everything that clears the engine beside it -
	// which is the whole of what internal.TimingCacheDir exists to avoid, and it fails silently as a slow build.
	if zero["trt_timing_cache_enable"] != "1" {
		t.Fatalf("timing cache must be on, got %q", zero["trt_timing_cache_enable"])
	}
	if zero["trt_timing_cache_path"] != testPaths.timing {
		t.Fatalf("timing cache path = %q, want the shared directory %q",
			zero["trt_timing_cache_path"], testPaths.timing)
	}
	if zero["trt_engine_cache_path"] != testPaths.engine {
		t.Fatalf("engine cache path = %q, want the per-model directory %q",
			zero["trt_engine_cache_path"], testPaths.engine)
	}

	tuned := tensorRTOptions(testPaths, EPProfile{
		Fp16:              true,
		TrtWorkspaceBytes: 1 << 30,
		TrtShapes:         map[string]string{"trt_profile_min_shapes": "vid_input:1x33x32x32"},
	})

	if tuned["trt_fp16_enable"] != "1" {
		t.Fatalf("fp16 not applied: %q", tuned["trt_fp16_enable"])
	}
	if tuned["trt_max_workspace_size"] != "1073741824" {
		t.Fatalf("workspace override not applied: %q", tuned["trt_max_workspace_size"])
	}
	if tuned["trt_profile_min_shapes"] != "vid_input:1x33x32x32" {
		t.Fatalf("shape profile not merged: %q", tuned["trt_profile_min_shapes"])
	}
}

// TrtOptions is applied last so that a model can override a default, not merely add to it. Both halves are checked:
// overriding trt_engine_hw_compatible is what osaka is there for, and a key with no default has to survive too.
func TestTensorRTOptionsOverlayOverridesTheDefaults(t *testing.T) {
	options := tensorRTOptions(testPaths, EPProfile{
		TrtOptions: map[string]string{
			"trt_engine_hw_compatible": "0",
			"trt_auxiliary_streams":    "1",
		},
	})

	if options["trt_engine_hw_compatible"] != "0" {
		t.Fatalf("overlay did not override the default: %q", options["trt_engine_hw_compatible"])
	}
	if options["trt_auxiliary_streams"] != "1" {
		t.Fatalf("overlay key not merged: %q", options["trt_auxiliary_streams"])
	}

	// The cache path is not the overlay's to lose: it is derived per model by the session loader.
	if options["trt_engine_cache_path"] != "/cache" {
		t.Fatalf("overlay clobbered the cache path: %q", options["trt_engine_cache_path"])
	}
}

func TestCudaOptionsFollowTheProfile(t *testing.T) {
	zero := cudaOptions(EPProfile{})

	// NCHW is ORT's default, and a model nobody has measured must get it: NHWC is a loss on an fp32 graph and
	// fails outright on a graph whose Conv weights are computed rather than stored.
	if zero["prefer_nhwc"] != "0" {
		t.Fatalf("NHWC must be opt-in, got %q", zero["prefer_nhwc"])
	}

	// The capture run is the only correct one on TensorRT, and it dies outright on this provider. See the comment
	// above tensorRTOptions.
	if zero["enable_cuda_graph"] != "0" {
		t.Fatalf("CUDA graph capture must stay off, got %q", zero["enable_cuda_graph"])
	}

	if tuned := cudaOptions(EPProfile{CudaPreferNHWC: true}); tuned["prefer_nhwc"] != "1" {
		t.Fatalf("NHWC not applied: %q", tuned["prefer_nhwc"])
	}
}

// CudaOptions is applied last, so a model can override a default rather than only add to it. Both halves are checked,
// as they are for the TensorRT overlay: overriding a default, and a key with no default surviving.
func TestCudaOptionsOverlayOverridesTheDefaults(t *testing.T) {
	options := cudaOptions(EPProfile{
		CudaPreferNHWC: true,
		CudaOptions: map[string]string{
			"cudnn_conv_algo_search": "HEURISTIC",
			"arena_extend_strategy":  "kSameAsRequested",
			"prefer_nhwc":            "0",
		},
	})

	if options["cudnn_conv_algo_search"] != "HEURISTIC" {
		t.Fatalf("overlay did not override the default: %q", options["cudnn_conv_algo_search"])
	}
	if options["arena_extend_strategy"] != "kSameAsRequested" {
		t.Fatalf("overlay key not merged: %q", options["arena_extend_strategy"])
	}

	// The overlay is applied after the typed field, so it wins. That ordering is what makes it an escape hatch rather
	// than a second way to say the same thing.
	if options["prefer_nhwc"] != "0" {
		t.Fatalf("overlay lost to the typed field: %q", options["prefer_nhwc"])
	}

	// A graph capture must not become reachable through the overlay by accident - it is off for the reasons in the
	// comment above tensorRTOptions, and nothing here should have disturbed it.
	if options["enable_cuda_graph"] != "0" {
		t.Fatalf("overlay disturbed the graph capture default: %q", options["enable_cuda_graph"])
	}
}

// A profile that ignores the precision it was handed would silently apply one export's tuning to the other, which is
// exactly what CudaPreferNHWC must not do.
// The execution mode is applied by applyProfile rather than by a provider appender, so nothing else in this file
// covers it - and getting it wrong is silent, since the session still builds and still returns the right answer.
func TestExecutionModeFollowsTheProfile(t *testing.T) {
	tests := []struct {
		name    string
		profile EPProfile
		want    ort.ExecutionMode
	}{
		{"the zero value keeps the shipped behaviour", EPProfile{}, ort.ExecutionModeParallel},
		{"a backbone graph opts out of the inter-op pool",
			EPProfile{ExecutionMode: ExecutionModeSequential}, ort.ExecutionModeSequential},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := tt.profile.ExecutionMode.mode(); got != tt.want {
				t.Fatalf("mode() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestResolveProfilePassesThePrecisionThrough(t *testing.T) {
	if got := ResolveProfile(nil, types.PrecisionFp16); !reflect.DeepEqual(got, EPProfile{}) {
		t.Fatalf("a variant with no profile must get the zero value, got %+v", got)
	}

	profile := func(precision types.Precision) EPProfile {
		return EPProfile{CudaPreferNHWC: precision == types.PrecisionFp16}
	}

	if !ResolveProfile(profile, types.PrecisionFp16).CudaPreferNHWC {
		t.Error("fp16 profile not resolved")
	}
	if ResolveProfile(profile, types.PrecisionFp32).CudaPreferNHWC {
		t.Error("fp32 resolved to the fp16 profile")
	}
}

func TestResolveProviders(t *testing.T) {
	trt := types.ExecutionProviderTensorRT
	cuda := types.ExecutionProviderCUDA
	coreml := types.ExecutionProviderCoreML
	directml := types.ExecutionProviderDirectML

	tests := []struct {
		name    string
		goos    string
		ep      types.ExecutionProvider
		profile EPProfile
		want    []types.ExecutionProvider
		wantErr bool
	}{
		{
			name: "an explicit request yields just that provider",
			goos: "linux", ep: cuda,
			want: []types.ExecutionProvider{cuda},
		},
		{
			name: "auto walks the platform chain",
			goos: "darwin", ep: types.ExecutionProviderAuto,
			want: []types.ExecutionProvider{coreml, types.ExecutionProviderOpenVINO},
		},
		{
			name: "an excluded provider is dropped from auto",
			goos: "linux", ep: types.ExecutionProviderAuto,
			profile: EPProfile{ExcludeEPs: []types.ExecutionProvider{trt}},
			want:    []types.ExecutionProvider{cuda, types.ExecutionProviderOpenVINO},
		},
		{
			name: "an excluded explicit request falls to the rest of the chain, never to the CPU",
			goos: "linux", ep: trt,
			profile: EPProfile{ExcludeEPs: []types.ExecutionProvider{trt}},
			want:    []types.ExecutionProvider{cuda, types.ExecutionProviderOpenVINO},
		},
		{
			name: "a provider the platform lacks is an error",
			goos: "linux", ep: coreml,
			wantErr: true,
		},
		{
			name: "DirectML is Windows-only",
			goos: "linux", ep: directml,
			wantErr: true,
		},
		{
			name: "an unknown platform is an error",
			goos: "plan9", ep: cuda,
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := resolveProviders(tt.goos, tt.ep, tt.profile)

			if tt.wantErr {
				if err == nil {
					t.Fatalf("want an error, got %v", got)
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}

			if len(got) != len(tt.want) {
				t.Fatalf("want %v, got %v", tt.want, got)
			}
			for i := range got {
				if got[i] != tt.want[i] {
					t.Fatalf("want %v, got %v", tt.want, got)
				}
			}
		})
	}
}

// Every provider the platform chains reference must have an appender, or createOptions would panic on a nil map
// entry the first time that platform selected it.
func TestEveryChainedProviderHasAnAppender(t *testing.T) {
	for goos, chain := range autoChain {
		for _, ep := range chain {
			if providerAppenders[ep] == nil {
				t.Fatalf("%s chains %s, which has no appender", goos, ep)
			}
		}
	}
}
