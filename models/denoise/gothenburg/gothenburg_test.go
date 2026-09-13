package gothenburg

import (
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// Keeping the fp16 graph off the Neural Engine is worth 25.7% per tile, and nothing else asserts it: dropping the
// field would still load the model and still return the right image, just a third slower. It is also what makes the
// precision choice read correctly, since fp16 is only faster than fp32 on this model with the setting in place.
func TestProfileKeepsGothenburgFp16OffTheNeuralEngine(t *testing.T) {
	if variant.Profile == nil {
		t.Fatal("gothenburg must carry a provider profile")
	}

	if got := variant.Profile(types.PrecisionFp16).CoreMLComputeUnits; got != utils.CoreMLComputeUnitsCPUAndGPU {
		t.Fatalf("CoreMLComputeUnits = %v, want CPUAndGPU", got)
	}
}

// At fp32 the same setting measured +0.1%, because CoreML bars an fp32 MLProgram from the Neural Engine and so ALL
// and CPUAndGPU compile to the same session. The precision condition is therefore not load-bearing for speed here -
// it is load-bearing for not inviting the next editor to widen a setting that has never been measured at fp32 on
// this graph.
func TestProfileLeavesGothenburgFp32OnTheDefaultComputeUnits(t *testing.T) {
	if got := variant.Profile(types.PrecisionFp32).CoreMLComputeUnits; got != utils.CoreMLComputeUnitsAll {
		t.Fatalf("CoreMLComputeUnits = %v, want ALL", got)
	}
}

// Sequential is the largest CoreML win on several graphs here, and this is not one of them: it measured -0.4% at
// fp32 and +2.9% at fp16, because CoreML fuses the re-exported graph into a single node and leaves the inter-op pool
// nothing to schedule. Pinned so a sweep of another model does not get copied onto this one.
func TestProfileLeavesGothenburgOnTheParallelExecutionMode(t *testing.T) {
	for _, precision := range []types.Precision{types.PrecisionFp32, types.PrecisionFp16} {
		if got := variant.Profile(precision).ExecutionMode; got != utils.ExecutionModeParallel {
			t.Errorf("%s: ExecutionMode = %v, want Parallel", precision, got)
		}
	}
}

// Every other CoreML option measured inside the run-to-run spread on this graph, so the profile names none of them.
func TestProfileNamesNothingElse(t *testing.T) {
	for _, precision := range []types.Precision{types.PrecisionFp32, types.PrecisionFp16} {
		p := variant.Profile(precision)

		if p.CoreMLSpecialization != utils.CoreMLSpecializationDefault {
			t.Errorf("%s: CoreMLSpecialization = %v, want Default", precision, p.CoreMLSpecialization)
		}

		if p.GraphOptimization != utils.GraphOptimizationDefault {
			t.Errorf("%s: GraphOptimization = %v, want Default", precision, p.GraphOptimization)
		}

		if len(p.Extra) != 0 {
			t.Errorf("%s: Extra = %v, want empty", precision, p.Extra)
		}
	}
}
