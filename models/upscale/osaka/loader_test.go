package osaka

import (
	"slices"
	"testing"

	"github.com/vegidio/open-photo-ai/types"
)

// Both exclusions were established empirically and are cheap to lose in a refactor, so they are pinned here with the
// reason attached rather than left to the comment in loader.go alone.
func TestProfileExcludesTheProvidersThatCannotRunThisModel(t *testing.T) {
	p := profileFor()

	excluded := map[types.ExecutionProvider]string{
		types.ExecutionProviderCoreML:   "the VAE cannot initialize, and the DiT either aborts or silently returns a wrong result",
		types.ExecutionProviderTensorRT: "needs optimization profiles this pipeline has no measured shape ranges for",
	}

	for ep, why := range excluded {
		if !contains(p.ExcludeEPs, ep) {
			t.Errorf("%s must stay excluded: it %s", ep, why)
		}
	}

}

// The graph's spatial axes are dynamic and the region size varies, so neither of these may be dropped: telling a
// provider to expect static shapes makes it decline the varying subgraphs, and the memory-pattern planner reserves
// for the largest region it has seen and never gives it back.
func TestProfileDeclaresTheGraphDynamic(t *testing.T) {
	p := profileFor()

	if !p.DynamicShapes {
		t.Error("DynamicShapes must be set: all three graphs have dynamic spatial axes")
	}
	if !p.DisableMemPattern {
		t.Error("DisableMemPattern must be set: the region size varies across a run")
	}
	// ONNX Runtime 1.26, which this app bundles, miscompiles the graph without these disabled - session creation
	// fails outright. Newer runtimes do not, so the temptation to drop them returns whenever someone tests against a
	// different build; they must stay until the bundled runtime moves.
	if len(p.DisableOptimizers) == 0 {
		t.Error("the miscompiling graph transformers must stay disabled or the session will not initialize")
	}
}

func contains(eps []types.ExecutionProvider, want types.ExecutionProvider) bool {
	return slices.Contains(eps, want)
}
