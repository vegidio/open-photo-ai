package osaka

import (
	"slices"
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// TensorRT is still excluded and CoreML deliberately is not, so both halves are pinned here with the reason attached
// rather than left to the comment in loader.go alone.
func TestProfileExcludesOnlyTensorRT(t *testing.T) {
	p := profileFor(types.PrecisionFp32)

	if !contains(p.ExcludeEPs, types.ExecutionProviderTensorRT) {
		t.Error("TensorRT must stay excluded: nobody has measured the re-exported graphs on it")
	}

	// The three CoreML failures this exclusion used to carry - the VAE's "axis 4 is not in valid range" error, the
	// DiT's MPSNDArray abort, and a silently wrong result - were all export defects, and all three are gone. Each
	// graph is now a single CoreML partition matching the CPU at cosine 0.99999 or better, and CoreML is worth
	// roughly 40x end to end here, so re-adding this exclusion would be a very expensive way to fix nothing.
	if contains(p.ExcludeEPs, types.ExecutionProviderCoreML) {
		t.Error("CoreML must not be excluded: the re-exported graphs run correctly on it and it is the whole win")
	}
}

// Every graph is fixed-shape, so DynamicShapes must stay off - and that is not cosmetic. It feeds CoreML's
// RequireStaticInputShapes, and setting it is what used to make the CoreML EP fail session creation on the VAE.
func TestProfileDeclaresTheGraphStatic(t *testing.T) {
	p := profileFor(types.PrecisionFp32)

	if p.DynamicShapes {
		t.Error("DynamicShapes must stay unset: every graph is fixed-shape, and setting it breaks the VAE on CoreML")
	}
	// Not because shapes vary - they do not - but because the planner is a measured loss on activations this large:
	// +22% on the VAE encoder with it enabled.
	if !p.DisableMemPattern {
		t.Error("DisableMemPattern must be set: the memory-pattern planner costs ~22% on the VAE encoder")
	}
	// ONNX Runtime 1.26, which this app bundles, miscompiles the graph without these disabled - session creation
	// fails outright. Newer runtimes do not, so the temptation to drop them returns whenever someone tests against a
	// different build; they must stay until the bundled runtime moves.
	if len(p.DisableOptimizers) == 0 {
		t.Error("the miscompiling graph transformers must stay disabled or the session will not initialize")
	}
	// ALL lets CoreML reach the Neural Engine, which these graphs are 2.5x to 4.2x worse on.
	if p.CoreMLComputeUnits != utils.CoreMLComputeUnitsCPUAndGPU {
		t.Error("CoreMLComputeUnits must be CPUAndGPU: the Neural Engine is far slower for these graphs")
	}
}

// The DiT takes the packed latent alone - the timestep is a constant inside the graph. Passing a second input would
// fail at session creation with "Invalid input name: timestep", so this is pinned against a well-meaning revert.
func TestTheDiTTakesOnlyTheLatent(t *testing.T) {
	for _, g := range graphs {
		if g.Role != roleDiT {
			continue
		}

		if len(g.Inputs) != 1 || g.Inputs[0] != "vid_input" {
			t.Errorf("DiT Inputs = %v, want [vid_input]: the timestep is baked into the graph", g.Inputs)
		}
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
