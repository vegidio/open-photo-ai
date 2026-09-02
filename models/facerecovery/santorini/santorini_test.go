package santorini

import (
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// The tuning is the only thing asserting santorini's CoreML settings, and getting either half wrong is silent: the
// model still loads and still returns the right answer, just slower. Both halves are checked because they pull in
// opposite directions - the specialisation is the win, and leaving the compute units alone is what keeps the fp16
// graph on the Neural Engine, which is worth more than the specialisation is.
func TestVariantSpecialisesSantoriniForLatency(t *testing.T) {
	if variant.Profile == nil {
		t.Fatal("santorini must carry a provider profile")
	}

	profile := variant.Profile(types.PrecisionFp16)

	if got := profile.CoreMLSpecialization; got != utils.CoreMLSpecializationFastPrediction {
		t.Fatalf("CoreMLSpecialization = %v, want FastPrediction", got)
	}

	// Athens keeps itself off the Neural Engine and santorini is the same kind of model, so this is the setting
	// most likely to be copied over by someone tidying the two variants into agreement. Measured, it costs ~9%.
	if got := profile.CoreMLComputeUnits; got != utils.CoreMLComputeUnitsAll {
		t.Fatalf("CoreMLComputeUnits = %v, want ALL: santorini's fp16 graph is faster on the Neural Engine", got)
	}
}

// The execution mode is the one setting in this profile that is not about CoreML, and it is the one a reader is most
// likely to drop as redundant - it looks like a default being restated. It is worth 4-9% through the CUDA provider,
// and both precisions want it, which is why the assertion covers both.
func TestVariantRunsSantoriniSequentially(t *testing.T) {
	for _, precision := range []types.Precision{types.PrecisionFp32, types.PrecisionFp16} {
		t.Run(string(precision), func(t *testing.T) {
			got := utils.ResolveProfile(variant.Profile, precision).ExecutionMode

			if got != utils.ExecutionModeSequential {
				t.Fatalf("ExecutionMode = %v, want Sequential: santorini's graph is a backbone, so the "+
					"inter-op pool only costs it a thread handoff per node", got)
			}
		})
	}
}
