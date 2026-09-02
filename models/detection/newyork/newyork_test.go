package newyork

import (
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// Both CUDA settings are load-bearing and neither announces itself if it is dropped: the model still loads and still
// returns the same two faces, just slower. Together they are -17.5% in fp32 and -35.1% on the fp16 graph.
//
// The fp32 half is the one most at risk. athens sets CudaPreferNHWC for its fp16 export ONLY, because there NHWC
// measured +8% in fp32 - so a reader tidying this for symmetry with the other variant that sets the flag would drop
// exactly the half that is worth 17.5% here. See the variant's comment for the full table.
func TestVariantRunsNewyorkInNHWCAtBothPrecisions(t *testing.T) {
	if variant.Profile == nil {
		t.Fatal("newyork must carry a provider profile")
	}

	for _, precision := range []types.Precision{types.PrecisionFp32, types.PrecisionFp16} {
		profile := variant.Profile(precision)

		if !profile.CudaPreferNHWC {
			t.Errorf("%s: the graph must run in NHWC on CUDA; unlike athens, this one wins in fp32 too", precision)
		}

		if profile.ExecutionMode != utils.ExecutionModeSequential {
			t.Errorf("%s: ExecutionMode = %v, want sequential", precision, profile.ExecutionMode)
		}
	}
}

// Leaving newyork on the CoreML defaults is a measured decision, not an oversight, and nothing else in the codebase
// records it: every CoreML setting EPProfile can express was benchmarked against this graph and none of them won.
//
// The failure this guards is silent. athens keeps itself off the Neural Engine and santorini asks for FastPrediction,
// so those are the obvious things to add here for symmetry - and the model would still load and still return the same
// faces, just slower. CPUAndGPU is the expensive one: 11.9ms against 9.9ms on the fp16 graph, because RetinaFace is
// dense convolution end to end and the Neural Engine is the right place for it.
func TestVariantLeavesNewyorkOnTheCoreMLDefaults(t *testing.T) {
	for _, precision := range []types.Precision{types.PrecisionFp32, types.PrecisionFp16} {
		profile := variant.Profile(precision)

		if got := profile.CoreMLComputeUnits; got != utils.CoreMLComputeUnitsAll {
			t.Errorf("%s: CoreMLComputeUnits = %v, want ALL; CPUAndGPU costs the fp16 graph 20%%", precision, got)
		}

		if got := profile.CoreMLSpecialization; got != utils.CoreMLSpecializationDefault {
			t.Errorf("%s: CoreMLSpecialization = %v, want Default; FastPrediction measured as a tie", precision, got)
		}
	}
}
