package osaka

import (
	"slices"
	"testing"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// This model excludes no provider at all now, and both former exclusions are pinned here with the reason attached
// rather than left to the comment in loader.go alone.
func TestProfileExcludesNoProvider(t *testing.T) {
	p := profileFor(types.PrecisionFp32)

	// The three CoreML failures the exclusion used to carry - the VAE's "axis 4 is not in valid range" error, the
	// DiT's MPSNDArray abort, and a silently wrong result - were all export defects, and all three are gone. Each
	// graph is now a single CoreML partition matching the CPU at cosine 0.99999 or better, and CoreML is worth
	// roughly 40x end to end here, so re-adding that exclusion would be a very expensive way to fix nothing.
	//
	// TensorRT was excluded on the dynamic-shape export, where it had to rebuild an engine per tile size. The graphs
	// are fixed-shape now and it is the fastest provider this model has: 1.797s against the CUDA provider's 4.494s
	// end to end on an RTX 5090, and 140.3ms against 441.1ms on one region.
	//
	// Asserting the list is empty rather than naming the two says what the test name says, and catches a third
	// exclusion arriving as well.
	if len(p.ExcludeEPs) != 0 {
		t.Errorf("no provider may be excluded, got %v: CoreML is worth ~40x here and TensorRT 2.5x the CUDA provider",
			p.ExcludeEPs)
	}
}

// The builder optimization level is the one TensorRT option this model overrides, and it is worth pinning because
// dropping it is invisible: the model still runs, at the same speed, and only the engine build gets 87 seconds
// slower - which nobody attributes to a setting.
func TestProfileLowersTheTensorRTBuilderLevel(t *testing.T) {
	p := profileFor(types.PrecisionFp32)

	if p.TrtOptions["trt_builder_optimization_level"] != "3" {
		t.Errorf("trt_builder_optimization_level = %q, want 3: level 5 costs 87s of engine build for no runtime gain",
			p.TrtOptions["trt_builder_optimization_level"])
	}
}

// The two TensorRT precision flags are refused for the same reason as each other: both are the largest number a
// provider sweep finds, and both buy it with precision the caller did not ask for.
func TestProfileRefusesTensorRTPrecisionFlags(t *testing.T) {
	p := profileFor(types.PrecisionFp32)

	// Not a no-op on the int8 export: it takes the DiT from 220.3ms to 78.4ms, and the decoded region from cosine
	// 0.9999 to 0.9954 against the same graph on CUDA. The fp16 export is faster than that AND accurate, so this
	// speed is only ever bought with quality the caller did not ask for.
	if p.Fp16 {
		t.Error("Fp16 must stay unset: it is a no-op on the fp16 export and a precision downgrade on the int8 one")
	}

	// Weight-only dequantization is not something TensorRT accelerates - the DiT measures 218.1ms against 220.3ms
	// with it - and the flag reaches the two VAE halves too, where the encoder goes 21.4ms to 124.6ms and the decoder
	// 45.5ms to 298.9ms.
	if p.TrtOptions["trt_int8_enable"] == "1" {
		t.Error("trt_int8_enable must stay off: it does nothing for the DiT and wrecks both VAE halves")
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

// The execution mode is the largest single setting in this profile on the CUDA provider - -13.5% at fp16 and -10.1%
// at int8 on one region - and it is the easiest to lose, because dropping it changes nothing a test would otherwise
// notice: the session still builds and the output is identical.
func TestProfileOptsOutOfTheInterOpPool(t *testing.T) {
	for _, precision := range []types.Precision{types.PrecisionFp16, types.PrecisionInt8} {
		if got := profileFor(precision).ExecutionMode; got != utils.ExecutionModeSequential {
			t.Errorf("%s: ExecutionMode = %v, want sequential: the DiT is 12,940 nodes and pays an inter-op "+
				"handoff at every one of them", precision, got)
		}
	}
}

// prefer_nhwc splits by graph rather than by precision here: it is a win on the two convolutional VAE halves and a
// small loss on a diffusion transformer that contains no convolution at all. Both halves of that are pinned, since an
// override that silently stopped applying would look exactly like one that was never there.
func TestPreferNHWCIsHeldOffTheDiT(t *testing.T) {
	for _, precision := range []types.Precision{types.PrecisionFp16, types.PrecisionInt8} {
		if !profileFor(precision).CudaPreferNHWC {
			t.Errorf("%s: the VAE halves want prefer_nhwc - -8%% on the encoder and -9%% on the decoder", precision)
		}

		dit := ditProfile(precision)

		if dit.CudaPreferNHWC {
			t.Errorf("%s: the DiT has no Conv node to make NHWC worth its layout transform", precision)
		}

		// The override is the variant's profile with one field changed, not a profile written from scratch. A DiT
		// that quietly lost the execution mode or the broken-optimizer list would fail to build a session at all.
		if dit.ExecutionMode != utils.ExecutionModeSequential {
			t.Errorf("%s: the DiT override dropped the execution mode", precision)
		}
		if !slices.Equal(dit.DisableOptimizers, brokenOptimizers) {
			t.Errorf("%s: the DiT override dropped the disabled optimizers, which is what lets it load at all",
				precision)
		}
	}
}

// The override has to be attached to the DiT's GraphSpec to have any effect, and to no other graph. Setting it on a
// VAE half would apply the transformer's tuning to the two graphs it was measured against.
func TestOnlyTheDiTOverridesTheProfile(t *testing.T) {
	for _, g := range graphs {
		if (g.Role == roleDiT) != (g.Profile != nil) {
			t.Errorf("%s: per-graph profile set = %v, want %v", g.Role, g.Profile != nil, g.Role == roleDiT)
		}
	}
}
