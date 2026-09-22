package utils

import (
	"testing"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/types"
)

// The fp16 exports come back wrong from the plugin (see webgpuSupportsGraph), and the only thing that keeps them off
// it is the file name. A rename convention drifting - `fp16` in the middle of the stem, say - would silently put them
// back on the GPU.
func TestWebGPUOnlyTakesTheFp32Graphs(t *testing.T) {
	tests := []struct {
		model string
		want  bool
	}{
		{"dn_gothenburg_fp32.onnx", true},
		{"dn_gothenburg_fp16.onnx", false},
		{"up_kyoto_2x_fp16.onnx", false},
		{"up_osaka_int8.onnx", true},
		{"up_osaka_vae_decoder_fp16.onnx", false},
		{"cl_jaipur_fp32.onnx", true},
	}

	for _, tt := range tests {
		t.Run(tt.model, func(t *testing.T) {
			if got := webgpuSupportsGraph(tt.model); got != tt.want {
				t.Fatalf("webgpuSupportsGraph(%q) = %v, want %v", tt.model, got, tt.want)
			}
		})
	}
}

// Every fallback-tier provider must sit in the chains, or the rule in createOptions guards a provider Auto never
// reaches; and it must never be first, or "nothing above it attached" is vacuously true and the tier means nothing.
func TestFallbackTierProvidersSitBelowAVendorProvider(t *testing.T) {
	for ep := range fallbackTier {
		for goos, chain := range autoChain {
			found := -1
			for i, candidate := range chain {
				if candidate == ep {
					found = i
				}
			}

			if found < 0 {
				t.Errorf("%s: %s is a fallback-tier provider but is not in the chain", goos, ep)
			} else if found == 0 {
				t.Errorf("%s: %s heads the chain, so it can never be a fallback", goos, ep)
			}
		}
	}
}

// The plugin declines quietly when it was never installed, which is the path every NVIDIA machine on Auto takes on
// every session build. If that stopped being errProviderUnavailable it would become a Warn per model.
func TestWebGPUDeclinesQuietlyWhenNotInstalled(t *testing.T) {
	SetWebGPULibrary("")
	ResetWebGPU()

	err := appendWebGPU(cachePaths{model: "dn_gothenburg_fp32.onnx"}, nil, EPProfile{})
	if err == nil {
		t.Fatal("expected the provider to decline without a library")
	}

	if !errors.Is(err, errProviderUnavailable) {
		t.Fatalf("expected errProviderUnavailable, got %v", err)
	}
}

// A machine that has the plugin but no GPU Dawn can use registers once and publishes nothing. Asking again must give
// the same quiet answer rather than a duplicate-registration error, which is what keying off the device slice did.
func TestARegisteredPluginWithNoDeviceStaysQuietOnEveryLaterSession(t *testing.T) {
	t.Cleanup(func() { SetWebGPULibrary(""); ResetWebGPU() })

	SetWebGPULibrary("/nonexistent/libonnxruntime_providers_webgpu.so")
	ResetWebGPU()

	// The environment is not up in a unit test, so registration itself fails; what is pinned here is that the
	// failure does not latch the flag, so the state stays retryable rather than wedged.
	if _, err := webgpuDevice(); err == nil {
		t.Fatal("expected registering a nonexistent library to fail")
	}

	webgpuMu.Lock()
	registered := webgpuRegistered
	webgpuMu.Unlock()

	if registered {
		t.Error("a failed registration must not mark the plugin as registered")
	}
}

func TestSerializesRunsOnlyForWebGPU(t *testing.T) {
	if serializesRuns([]types.ExecutionProvider{types.ExecutionProviderCUDA}) {
		t.Error("CUDA sessions must not be serialized")
	}

	if !serializesRuns([]types.ExecutionProvider{types.ExecutionProviderCUDA, types.ExecutionProviderWebGPU}) {
		t.Error("a chain with WebGPU attached must serialize its runs")
	}
}
