package facerecovery

import (
	"context"
	"fmt"
	"image"
	"sync"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/types"
)

// Variant is the complete description of one face-recovery model. Everything that distinguishes athens from santorini
// lives in this struct, so a per-variant package is a thin registration shim and adding a model is a data change
// rather than another copy of the same model/operation file pair.
type Variant struct {
	// Codename identifies the model in both its operation Id (`fr_<codename>_<precision>`) and the session loader.
	Codename string

	// Label is the display name shown in the UI, before the precision suffix is appended.
	Label string

	// Inputs names the graph's input tensors. CodeFormer-style graphs (athens) take a fidelity weight alongside the
	// image; the plain restorers (santorini) take the image only.
	Inputs []string

	// TileSize is the square resolution each aligned face is restored at.
	TileSize int

	// Fidelity is the weight bound to the graph's second input. It is only read when Inputs names one; -1 marks a
	// graph that takes no weight at all.
	Fidelity float32

	// Profile is the provider tuning this variant's graph needs. A nil Profile means the provider defaults, which
	// is what a variant nobody has measured should get: the tuning here is per-graph, and carrying one model's
	// findings over to another on the strength of them being the same kind of model is how a profile ends up
	// pessimising the model it was never measured against.
	Profile func() utils.EPProfile

	// The feathered blend mask is a pure function of TileSize, which is fixed per variant - but building it means
	// filling a TileSize-square image and then Gaussian-blurring it, which was being redone on every single Run.
	// Variants are package-level values that outlive any one run, so it is built once and shared from then on.
	maskOnce sync.Once
	mask     image.Image
}

// blendMask returns this variant's feathered circular blend mask, building it on first use.
func (v *Variant) blendMask() image.Image {
	v.maskOnce.Do(func() {
		v.mask = createCircularMask(v.TileSize, v.TileSize, maskBlurSigma)
	})

	return v.mask
}

// Op builds this variant's operation for the given pre-detected faces.
func (v *Variant) Op(precision types.Precision, faces []detection.Face) Op {
	return Op{
		variant:   v,
		precision: precision,
		faces:     faces,
	}
}

// New loads the ONNX session for this variant. operation must be an Op produced by the same variant.
func (v *Variant) New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*Model, error) {
	op, ok := operation.(Op)
	if !ok {
		return nil, errors.Errorf("expected a face recovery operation, got %T", operation)
	}

	// utils.ModelSpec is not usable here: athens takes a second "weight" input, so the tensor names come from the
	// variant rather than from the one-in-one-out convention.
	specs := []utils.SessionSpec{{ModelId: op.Id(), Inputs: v.Inputs, Outputs: []string{"output"}}}

	sessions, err := utils.LoadSessions(ctx, specs, ep, utils.ResolveProfile(v.Profile), onProgress)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to load the %s session", v.Codename)
	}

	return &Model{
		name:      utils.FormatModelName(v.Label, op.precision),
		operation: op,
		variant:   v,
		Session:   sessions[0],
	}, nil
}

// region - Operation

// Op identifies a face-recovery run. The variant and precision form the model identity; the faces are deliberately
// not part of it, so the registry reuses a single session across every selection the user makes.
type Op struct {
	variant   *Variant
	precision types.Precision
	faces     []detection.Face
}

func (o Op) Id() string {
	return fmt.Sprintf("fr_%s_%s", o.variant.Codename, o.precision)
}

func (o Op) Precision() types.Precision {
	return o.precision
}

// Params exposes the pre-detected faces to Model.Run. Faces are not part of the operation's identity, so they are
// passed per-run rather than stored on the (registry-cached) model.
func (o Op) Params() map[string]any {
	return map[string]any{ParamFaces: o.faces}
}

// CacheKey folds the selected faces into the image cache key — they are not in Id() but change the recovered output.
func (o Op) CacheKey() string {
	return FacesCacheKey(o.faces)
}

var (
	_ types.Operation     = (*Op)(nil)
	_ types.Parameterized = (*Op)(nil)
	_ types.CacheKeyer    = (*Op)(nil)
)

// endregion

// region - Model

// Model is a loaded face-recovery session together with the variant it was built from.
type Model struct {
	name      string
	operation Op
	variant   *Variant
	*utils.Session
}

var (
	_ types.Model[image.Image] = (*Model)(nil)
	_ types.Measurable         = (*Model)(nil)
)

func (m *Model) Id() string {
	return m.operation.Id()
}

func (m *Model) Name() string {
	return m.name
}

func (m *Model) Run(
	ctx context.Context,
	img image.Image,
	params map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	// Faces are detected independently and passed in via params (see ParamFaces); the model does not run face
	// detection itself. Read them from the per-run params, not the captured op: the registry caches one session per
	// Id and the captured op's faces would be stale across runs with different selections.
	faces, _ := params[ParamFaces].([]detection.Face)
	if len(faces) == 0 {
		return img, nil
	}

	result, err := RestoreFaces(ctx, m.Session, img, faces, m.variant, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to restore faces")
	}

	return result, nil
}

// endregion
