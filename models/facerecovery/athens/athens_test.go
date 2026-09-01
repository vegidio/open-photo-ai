package athens

import (
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
)

// Athens is the one face-recovery variant with provider tuning, and the tuning is what stops CoreML from putting the
// fp16 graph on the Neural Engine - where it measured 14x slower. Nothing else asserts the variant carries it, so a
// refactor that dropped the field would cost that silently: the model would still load and still return the right
// answer, just far slower.
func TestVariantKeepsAthensOffTheNeuralEngine(t *testing.T) {
	if variant.Profile == nil {
		t.Fatal("athens must carry a provider profile")
	}

	if got := variant.Profile().CoreMLComputeUnits; got != utils.CoreMLComputeUnitsCPUAndGPU {
		t.Fatalf("CoreMLComputeUnits = %v, want CPUAndGPU", got)
	}
}
