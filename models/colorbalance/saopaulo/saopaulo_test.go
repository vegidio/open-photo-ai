package saopaulo

import (
	"strings"
	"testing"

	"github.com/vegidio/open-photo-ai/types"
)

// The six grid convolutions return a wrong colour balance on WebGPU; both exports must keep them on the CPU, and the
// profile must not pick up tuning for any other provider along the way.
func TestProfilePinsTheGridConvolutionsToTheCPUOnWebGPU(t *testing.T) {
	for _, precision := range []types.Precision{types.PrecisionFp32, types.PrecisionFp16} {
		p := profile(precision)
		names := strings.Split(p.WebGPUOptions["forceCpuNodeNames"], "\n")

		if len(names) != 6 {
			t.Errorf("%s: %d nodes pinned, want the 6 grid convolutions", precision, len(names))
		}

		p.WebGPUOptions = nil
		if p.ExcludeEPs != nil || p.CoreMLComputeUnits != 0 || p.ExecutionMode != 0 {
			t.Errorf("%s: the profile must only carry the WebGPU pin, got %+v", precision, p)
		}
	}
}
