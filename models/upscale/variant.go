package upscale

import (
	"context"
	"fmt"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// Variant is the complete description of one convolutional upscale model. Everything that distinguishes kyoto from
// saitama lives in this struct, so a per-variant package is a thin registration shim and adding a model is a data
// change rather than another copy of the same model/operation file pair.
//
// Osaka is deliberately not built on this type: it is a diffusion model with its own tiling driver, not a variation
// on the shared convolutional pipeline.
type Variant struct {
	// Codename identifies the model in both its operation Id (`up_<codename>_<scale>x_<precision>`) and the session
	// loader.
	Codename string

	// ScaleBuckets maps a requested scale onto the sequence of native passes that cover it, which is what differs
	// between a variant shipping native 2x and 4x weights and one shipping only 4x.
	ScaleBuckets []ScaleBucket
}

// Op builds this variant's operation at the given scale, clamped to the supported range.
func (v *Variant) Op(scale float64, precision types.Precision) Op {
	return Op{
		variant:   v,
		precision: precision,
		scale:     ClampScale(scale),
	}
}

// New loads one ONNX session per native pass needed to reach the requested scale. operation must be an Op produced by
// the same variant.
func (v *Variant) New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*Model, error) {
	op := operation.(Op)
	scales := SelectScaleMatrix(op.scale, v.ScaleBuckets)

	sessions, err := LoadSessions(ctx, v.Codename, op.precision, scales, ep, onProgress)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to load the %s sessions", v.Codename)
	}

	return &Model{
		name:      FormatUpscaleName(op.scale, op.precision),
		operation: op,
		Sessions:  sessions,
		scales:    scales,
	}, nil
}

// region - Operation

// Op identifies an upscale run: the variant, the requested scale, and the precision together form the model identity.
type Op struct {
	variant   *Variant
	precision types.Precision
	scale     float64
}

func (o Op) Id() string {
	return fmt.Sprintf("up_%s_%.4gx_%s", o.variant.Codename, o.scale, o.precision)
}

func (o Op) Precision() types.Precision {
	return o.precision
}

var _ types.Operation = (*Op)(nil)

// endregion

// region - Model

// Model is the set of loaded sessions for one upscale variant, plus the pass sequence they are run in.
type Model struct {
	name      string
	operation Op
	scales    []int
	utils.Sessions
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
	return RunPipeline(ctx, m.Sessions, img, m.scales, m.operation.scale, onProgress)
}

// endregion
