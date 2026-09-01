package newyork

import "testing"

// Leaving newyork on the provider defaults is a measured decision, not an oversight, and nothing else in the codebase
// records it: every CoreML setting EPProfile can express was benchmarked against this graph and none of them won.
//
// The failure this guards is silent. athens keeps itself off the Neural Engine and santorini asks for FastPrediction,
// so a profile is the obvious thing to add here for symmetry - and the model would still load and still return the
// same faces, just slower. CPUAndGPU is the expensive one: 11.9ms against 9.9ms on the fp16 graph, because RetinaFace
// is dense convolution end to end and the Neural Engine is the right place for it. See the variant's comment for the
// full table.
func TestVariantLeavesNewyorkOnTheProviderDefaults(t *testing.T) {
	if variant.Profile != nil {
		t.Fatal("newyork must carry no provider profile: measured, every CoreML setting is a tie or a loss for " +
			"this graph, and CPUAndGPU costs the fp16 graph 20%")
	}
}
