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

// TestOnlyTheDiTFollowsTheOperationPrecision pins the split that makes the int8 build possible at all: the diffusion
// transformer is published at both precisions, the two VAE halves only at fp16, and one pair of VAE files serves both
// builds. If a VAE graph lost its pin it would follow an int8 operation into `up_osaka_vae_encoder_int8`, which does
// not exist on the remote - and a missing artifact is not a build error, it is an unverified download of a 404.
func TestOnlyTheDiTFollowsTheOperationPrecision(t *testing.T) {
	pinned := map[string]types.Precision{
		roleEncoder: types.PrecisionFp16,
		roleDecoder: types.PrecisionFp16,
		roleDiT:     "",
	}

	if len(graphs) != len(pinned) {
		t.Fatalf("graphs has %d entries, want %d - a new graph needs a precision decision here", len(graphs), len(pinned))
	}

	for _, g := range graphs {
		want, known := pinned[g.Role]
		if !known {
			t.Errorf("unexpected graph role %q", g.Role)
			continue
		}

		if g.Precision != want {
			t.Errorf("%s: Precision = %q, want %q", g.Role, g.Precision, want)
		}
	}
}

func contains(eps []types.ExecutionProvider, want types.ExecutionProvider) bool {
	return slices.Contains(eps, want)
}
