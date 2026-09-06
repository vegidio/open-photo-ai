package kyoto

import (
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// Kyoto is the one model in the catalogue that ASKS for the Neural Engine, and the request is worth 20% on its 4x
// fp16 pass. Nothing else asserts it: dropping the field would still load every model and still return the right
// image, just 20% slower on the pass most runs use.
func TestProfilePutsKyotoFp16OnTheNeuralEngine(t *testing.T) {
	if variant.Profile == nil {
		t.Fatal("kyoto must carry a provider profile")
	}

	if got := variant.Profile(types.PrecisionFp16).CoreMLComputeUnits; got != utils.CoreMLComputeUnitsCPUAndNeuralEngine {
		t.Fatalf("CoreMLComputeUnits = %v, want CPUAndNeuralEngine", got)
	}
}

// The same setting at fp32 is a 9x to 20x REGRESSION, because CoreML bars an fp32 MLProgram from the Neural Engine
// and asking for it anyway drops the graph onto the CPU. The precision condition is the whole safety of this
// profile, so it is pinned separately from the setting it guards.
func TestProfileLeavesKyotoFp32OnTheDefaultComputeUnits(t *testing.T) {
	if got := variant.Profile(types.PrecisionFp32).CoreMLComputeUnits; got != utils.CoreMLComputeUnitsAll {
		t.Fatalf("CoreMLComputeUnits = %v, want ALL: CoreML cannot put an fp32 MLProgram on the Neural Engine", got)
	}
}

// Tokyo sets the sequential execution mode and kyoto is the sibling that must not copy it: RRDB's dense blocks feed
// each concatenation from several branches at once, which is the independent structure the inter-op pool exists for,
// and sequential measured a consistent loss on the CPU provider at both precisions.
func TestProfileLeavesKyotoOnTheParallelExecutionMode(t *testing.T) {
	for _, precision := range []types.Precision{types.PrecisionFp32, types.PrecisionFp16} {
		if got := variant.Profile(precision).ExecutionMode; got != utils.ExecutionModeParallel {
			t.Errorf("%s: ExecutionMode = %v, want Parallel", precision, got)
		}
	}
}
