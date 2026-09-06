package saitama

import (
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// The Neural Engine is worth 54% on saitama's fp16 pass, and nothing else asserts it: dropping the field would still
// load the model and still return the right image, just at twice the time.
//
// It is only worth that much on the re-exported fp16 graph. Against the export that shipped before it - the one whose
// two upsample Resize nodes stayed in fp32 - this same setting was a 50% REGRESSION, the worst of the three choices
// rather than the best. So this test guards a setting whose sign depends on the weights it is paired with: if the
// published fp16 artifact is ever rolled back, this profile has to go with it.
func TestProfilePutsSaitamaFp16OnTheNeuralEngine(t *testing.T) {
	if variant.Profile == nil {
		t.Fatal("saitama must carry a provider profile")
	}

	if got := variant.Profile(types.PrecisionFp16).CoreMLComputeUnits; got != utils.CoreMLComputeUnitsCPUAndNeuralEngine {
		t.Fatalf("CoreMLComputeUnits = %v, want CPUAndNeuralEngine", got)
	}
}

// The same setting at fp32 is a 19x REGRESSION, because CoreML bars an fp32 MLProgram from the Neural Engine and
// asking for it anyway drops the graph onto the CPU. The precision condition is the whole safety of this profile, so
// it is pinned separately from the setting it guards.
func TestProfileLeavesSaitamaFp32OnTheDefaultComputeUnits(t *testing.T) {
	if got := variant.Profile(types.PrecisionFp32).CoreMLComputeUnits; got != utils.CoreMLComputeUnitsAll {
		t.Fatalf("CoreMLComputeUnits = %v, want ALL: CoreML cannot put an fp32 MLProgram on the Neural Engine", got)
	}
}

// Tokyo sets the sequential execution mode and saitama is one of the two siblings that must not copy it: RRDB's dense
// blocks feed each concatenation from several branches at once, which is the independent structure the inter-op pool
// exists for, and sequential measured +8% on CoreML at fp16.
func TestProfileLeavesSaitamaOnTheParallelExecutionMode(t *testing.T) {
	for _, precision := range []types.Precision{types.PrecisionFp32, types.PrecisionFp16} {
		if got := variant.Profile(precision).ExecutionMode; got != utils.ExecutionModeParallel {
			t.Errorf("%s: ExecutionMode = %v, want Parallel", precision, got)
		}
	}
}

// Every remaining CoreML option measured within 2% on this graph, so the profile names none of them. The one that
// did not - ModelFormat=NeuralNetwork, at -38.9% on fp32 - is not reachable from an EPProfile at all, deliberately:
// it buys that by running the fp32 graph in half precision. See coreMLOptions in internal/utils/ep_profile.go.
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
