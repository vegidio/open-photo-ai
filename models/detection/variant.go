package detection

import (
	"context"
	"fmt"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// Variant is the complete description of one face-detection model. Everything that distinguishes one from another
// lives in this struct, so a per-variant package is a thin registration shim and adding a model is a data change
// rather than another copy of the same model/operation file pair.
type Variant struct {
	// Codename identifies the model in both its operation Id (`dt_<codename>_<precision>`) and the session loader.
	Codename string

	// Label is the display name shown in the UI, before the precision suffix is appended.
	Label string

	// Outputs names the graph's output tensors. Detection graphs return three of them - boxes, scores and landmarks -
	// rather than the single "output" the rest of the codebase's models use.
	Outputs []string

	// Profile is the provider tuning this variant's graph needs. A nil Profile means the provider defaults, which
	// is what a variant nobody has measured should get: the tuning here is per-graph, and carrying one model's
	// findings over to another on the strength of them being the same kind of model is how a profile ends up
	// pessimising the model it was never measured against.
	Profile func(precision types.Precision) utils.EPProfile
}

// Op builds this variant's operation at the given precision.
func (v *Variant) Op(precision types.Precision) Op {
	return Op{
		variant:   v,
		precision: precision,
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
		return nil, errors.Errorf("expected a detection operation, got %T", operation)
	}

	// utils.ModelSpec is not usable here: the graph returns three named tensors, so the output names come from the
	// variant rather than from the one-in-one-out convention.
	specs := []utils.SessionSpec{{ModelId: op.Id(), Inputs: []string{"input"}, Outputs: v.Outputs}}

	sessions, err := utils.LoadSessions(ctx, specs, ep, utils.ResolveProfile(v.Profile, op.precision), onProgress)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to load the %s session", v.Codename)
	}

	return &Model{
		name:      utils.FormatModelName(v.Label, op.precision),
		operation: op,
		Session:   sessions[0],
	}, nil
}

// region - Operation

// Op identifies a face-detection run.
type Op struct {
	variant   *Variant
	precision types.Precision
}

func (o Op) Id() string {
	return fmt.Sprintf("dt_%s_%s", o.variant.Codename, o.precision)
}

func (o Op) Precision() types.Precision {
	return o.precision
}

// Detection has no per-run inputs, so Op deliberately implements neither Parameterized nor CacheKeyer: the operation
// Id alone identifies both the model file and the cached result.
var _ types.Operation = (*Op)(nil)

// endregion

// region - Model

// Model is a loaded face-detection session. Unlike the other families it produces faces rather than an image, so it
// satisfies types.Model[[]Face].
type Model struct {
	name      string
	operation Op
	*utils.Session
}

var (
	_ types.Model[[]Face] = (*Model)(nil)
	_ types.Measurable    = (*Model)(nil)
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
	_ map[string]any,
	onProgress types.InferenceProgress,
) ([]Face, error) {
	return Run(ctx, m.Session, img, onProgress)
}

// endregion
