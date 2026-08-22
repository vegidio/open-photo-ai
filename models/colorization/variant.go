package colorization

import (
	"context"
	"fmt"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// Variant is the complete description of one colorization model. Everything that distinguishes delhi from jaipur lives
// in this struct, so a per-variant package is a thin registration shim and adding a model is a data change rather than
// another copy of the same model/operation file pair.
type Variant struct {
	// Codename identifies the model in both its operation Id (`cl_<codename>_<precision>`) and the session loader.
	Codename string

	// Label is the display name shown in the UI, before the precision suffix is appended.
	Label string

	// Spec is the graph contract this variant's model was exported to. DDColor variants (delhi, mumbai) predict the
	// Lab ab planes; DeOldify (jaipur) returns full RGB. This is the only behavioural difference between the
	// variants, so it is data on the variant rather than a forked Run method.
	Spec Spec
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
	op := operation.(Op)

	session, err := utils.LoadSingleSession(ctx, "cl", v.Codename, op.precision, ep, onProgress)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to load the %s session", v.Codename)
	}

	return &Model{
		name:      utils.FormatModelName(v.Label, op.precision),
		operation: op,
		variant:   v,
		Session:   session,
	}, nil
}

// region - Operation

// Op identifies a colorization run.
type Op struct {
	variant   *Variant
	precision types.Precision
}

func (o Op) Id() string {
	return fmt.Sprintf("cl_%s_%s", o.variant.Codename, o.precision)
}

func (o Op) Precision() types.Precision {
	return o.precision
}

// Colorization has no per-run inputs, so Op deliberately implements neither Parameterized nor CacheKeyer:
// the operation Id alone identifies both the model file and the cached result.
var _ types.Operation = (*Op)(nil)

// endregion

// region - Model

// Model is a loaded colorization session together with the variant it was built from.
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
	_ map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	if onProgress != nil {
		onProgress(0)
	}
	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	result, err := Process(ctx, m.Session, img, m.variant.Spec)
	if err != nil {
		return nil, errors.Wrap(err, "failed to process image")
	}

	if onProgress != nil {
		onProgress(1)
	}

	return result, nil
}

// endregion
