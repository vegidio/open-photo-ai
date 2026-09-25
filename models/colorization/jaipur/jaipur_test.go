package jaipur

import (
	"strings"
	"testing"

	"github.com/vegidio/open-photo-ai/types"
)

// The two decoder convolutions hang the GPU on WebGPU's Vulkan path whatever the precision, so the profile has to
// carry the CPU pin for both exports - unlike the CoreML tuning beside it, which is fp16-only.
func TestProfilePinsTheDecoderConvolutionsToTheCPUOnWebGPU(t *testing.T) {
	for _, precision := range []types.Precision{types.PrecisionFp32, types.PrecisionFp16} {
		names := profile(precision).WebGPUOptions["forceCpuNodeNames"]

		for _, node := range []string{"/m/layers.10/layers.0/layers.0.0/Conv", "/m/layers.10/layers.1/layers.1.0/Conv"} {
			if !strings.Contains(names, node) {
				t.Errorf("%s: %s is not pinned to the CPU", precision, node)
			}
		}
	}
}

func TestProfileKeepsTheCoreMLTuningFp16Only(t *testing.T) {
	if got := profile(types.PrecisionFp32).CoreMLComputeUnits; got != 0 {
		t.Fatalf("fp32 CoreMLComputeUnits = %v, want the default", got)
	}
}
