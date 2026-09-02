package athens

import (
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// Athens is the one face-recovery variant with provider tuning, and the tuning is what stops CoreML from putting the
// fp16 graph on the Neural Engine - where it measured 14x slower. Nothing else asserts the variant carries it, so a
// refactor that dropped the field would cost that silently: the model would still load and still return the right
// answer, just far slower.
func TestVariantKeepsAthensOffTheNeuralEngine(t *testing.T) {
	if variant.Profile == nil {
		t.Fatal("athens must carry a provider profile")
	}

	for _, precision := range []types.Precision{types.PrecisionFp32, types.PrecisionFp16} {
		if got := variant.Profile(precision).CoreMLComputeUnits; got != utils.CoreMLComputeUnitsCPUAndGPU {
			t.Errorf("%s: CoreMLComputeUnits = %v, want CPUAndGPU", precision, got)
		}
	}
}

// The CUDA layout is the one setting here that differs by precision, and both halves are load-bearing: NHWC is what
// reaches cuDNN's fp16 tensor-core kernels (-4% end to end), and it is a measured +8% on the fp32 export, which has
// no such kernels to reach and pays only the layout conversion. Neither half announces itself if it is dropped or
// copied to the other precision - the model loads and returns the right answer either way.
func TestVariantRunsOnlyTheFp16GraphInNHWC(t *testing.T) {
	if !variant.Profile(types.PrecisionFp16).CudaPreferNHWC {
		t.Error("the fp16 graph must run in NHWC on CUDA")
	}

	if variant.Profile(types.PrecisionFp32).CudaPreferNHWC {
		t.Error("the fp32 graph must stay in NCHW on CUDA: NHWC measured 8% slower end to end")
	}
}
