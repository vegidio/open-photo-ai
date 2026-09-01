package tokyo

import (
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
)

// Tokyo is the one convolutional upscale variant with provider tuning, and the tuning is what stops CoreML from
// offering the graph to the Neural Engine - where its ANE compiler fails outright, taking 1067 seconds to build a
// session that then runs 22x slower than the GPU. Nothing else asserts the variant carries it, so a refactor that
// dropped the field would cost that silently: the model would still load and still return the right answer.
func TestVariantKeepsTokyoOffTheNeuralEngine(t *testing.T) {
	if variant.Profile == nil {
		t.Fatal("tokyo must carry a provider profile")
	}

	if got := variant.Profile().CoreMLComputeUnits; got != utils.CoreMLComputeUnitsCPUAndGPU {
		t.Fatalf("CoreMLComputeUnits = %v, want CPUAndGPU", got)
	}
}

// FastPrediction routes through the same Neural Engine compiler that fails on this graph, and it did not merely run
// slowly when measured - it wedged the process at 11.5 GB resident. The default strategy is therefore load-bearing
// here rather than simply un-tuned, which is worth pinning so nobody adopts Santorini's setting by analogy.
func TestVariantLeavesTokyoOnTheDefaultSpecialization(t *testing.T) {
	if got := variant.Profile().CoreMLSpecialization; got != utils.CoreMLSpecializationDefault {
		t.Fatalf("CoreMLSpecialization = %v, want Default: FastPrediction hangs on tokyo's graph", got)
	}
}
