package athens

import (
	"context"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
)

// variant holds everything specific to athens; the shared implementation lives in the facerecovery package.
var variant = &facerecovery.Variant{
	Codename: "athens",
	Label:    "Athens",
	// CodeFormer takes the fidelity weight as a second input; 1.0 is full fidelity to the original face.
	Inputs:   []string{"input", "weight"},
	TileSize: 512,
	Fidelity: 1.0,
	Profile:  profileFor,
}

// profileFor keeps athens off the Neural Engine.
//
// Measured through this code path on an M2 Max (macOS 26.6, ONNX Runtime 1.26), one 512x512 face, median of 7 runs
// against the fp32 result:
//
//	                       fp16              fp32
//	MLComputeUnits=ALL     146.7ms           104.7ms
//	CPUAndGPU               93.5ms (1.57x)   104.7ms (unchanged)
//	CPUAndNeuralEngine       1.313s (14x slower)  1.017s (10x slower)
//
// The fp32 graph is unaffected because the Neural Engine is fp16-only, so CoreML never had work to put there. The
// fp16 graph is a different matter: ALL makes most of the graph eligible, CoreML takes it, and the transitions cost
// more than the Neural Engine saves. CodeFormer is why - 72 GroupNorms, 19 LayerNorms held at fp32 for precision,
// and several hundred reshapes and transposes through the transformer, which is close to the worst case for a unit
// built for dense convolution. The CPUAndNeuralEngine row is the tell: an order of magnitude is not a slower engine,
// it is an engine rejecting most of the graph and thrashing on what is left.
//
// Both precisions are set, though only fp16 moves. It documents the intent, and it means a future export that shifts
// what the Neural Engine will accept cannot silently re-enable it.
//
// How far this carries to other Macs: the cause is the graph's op mix, which is the same everywhere, and the Neural
// Engine is close to uniform across Apple Silicon while the GPU is not - so the direction should hold on any Mac,
// and on Intel there is no Neural Engine and the setting is a no-op. The 1.57x is this machine's number, though. A
// smaller GPU narrows the gap, so expect less there. It has not been measured on another chip or another macOS.
func profileFor() utils.EPProfile {
	return utils.EPProfile{CoreMLComputeUnits: utils.CoreMLComputeUnitsCPUAndGPU}
}

// New loads the athens session for the given operation.
func New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*facerecovery.Model, error) {
	return variant.New(ctx, operation, ep, onProgress)
}

// Op builds an athens operation at the given precision, for the given pre-detected faces.
func Op(precision types.Precision, faces []detection.Face) facerecovery.Op {
	return variant.Op(precision, faces)
}
