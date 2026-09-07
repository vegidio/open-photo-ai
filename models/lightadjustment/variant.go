package lightadjustment

import (
	"context"
	"fmt"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// Variant is the complete description of one light adjustment model. Everything that distinguishes one from another lives in
// this struct, so a per-variant package is a thin registration shim and adding a model is a data change rather than
// another copy of the same model/operation file pair.
type Variant struct {
	// Codename identifies the model in both its operation Id (`la_<codename>_<precision>`) and the session loader.
	Codename string

	// Label is the display name shown in the UI, before the precision suffix is appended.
	Label string

	// Canvas is how this variant's graph wants the image framed before it is handed over.
	Canvas Canvas

	// Profile is the provider tuning this variant needs. A nil Profile means the provider defaults, which is what a
	// variant nobody has measured should get: the right settings follow the graph's op mix, so carrying one variant's
	// findings to another because both adjust light is how a profile ends up pessimising a model it was never
	// measured against.
	Profile func(precision types.Precision) utils.EPProfile
}

// Canvas is the fixed square a variant's graph is exported at. It is per-variant data rather than a constant in
// process.go so that a variant can be re-exported at a different size without touching the shared geometry.
//
// Both graphs this family ships are fixed-shape, and for the same reason rather than by coincidence: with dynamic
// axes, every reshape and slice in a window-attention graph has an unbounded dimension, CoreML's MLProgram runtime
// refuses all of them, and the graph falls apart into hundreds of partitions that then fail at run time. A dynamic
// export of that architecture is numerically correct - measured, pixel-identical to PyTorch at every resolution -
// and still unusable here.
//
// Two consequences follow from the fixed square, and both are its cost rather than oversights. Every run resizes, so
// every run pays buildResult's full-resolution passes. And a 400x300 thumbnail costs the same inference time as a
// 6000x4000 photo, because the canvas is the same either way.
type Canvas struct {
	// Size is the exact width and height the graph accepts. The image is fitted so its longest side lands on Size
	// and the short side is reflection-padded out to fill the square - padded, never stretched, so the pixels the
	// model sees are the ones the photo has.
	Size int
}

// Op builds this variant's operation at the given per-run intensity.
func (v *Variant) Op(intensity float32, precision types.Precision) Op {
	return Op{
		variant:   v,
		intensity: intensity,
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
		return nil, errors.Errorf("expected a light adjustment operation, got %T", operation)
	}

	// The spec is built here rather than through utils.LoadSingleSession, which is how detection, face recovery and
	// the diffusion upscalers all do it: op.Id() is already the model id, so composing it a second time from a prefix
	// and a codename would only add a way for the two to disagree.
	specs := []utils.SessionSpec{utils.ModelSpec(op.Id())}

	sessions, err := utils.LoadSessions(ctx, specs, ep, utils.ResolveProfile(v.Profile, op.precision), onProgress)
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

// Op identifies a light adjustment run. The variant and precision form the model identity; the intensity is deliberately not
// part of it, so the registry reuses a single session across every intensity the user drags through.
type Op struct {
	variant   *Variant
	intensity float32
	precision types.Precision
}

func (o Op) Id() string {
	return fmt.Sprintf("la_%s_%s", o.variant.Codename, o.precision)
}

func (o Op) Precision() types.Precision {
	return o.precision
}

// Params carries the per-run blend intensity, which is not part of the operation identity.
func (o Op) Params() map[string]any {
	return map[string]any{utils.ParamIntensity: o.intensity}
}

// CacheKey folds the intensity into the image cache key so different intensities don't collide.
func (o Op) CacheKey() string {
	return utils.IntensityCacheKey(o.intensity)
}

var (
	_ types.Operation     = (*Op)(nil)
	_ types.Parameterized = (*Op)(nil)
	_ types.CacheKeyer    = (*Op)(nil)
)

// endregion

// region - Model

// Model is a loaded light adjustment session together with the variant it was built from.
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
	if onProgress != nil {
		onProgress(0)
	}
	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	result, err := Process(ctx, m.Session, img, m.variant.Canvas)
	if err != nil {
		return nil, errors.Wrap(err, "failed to process image")
	}

	if onProgress != nil {
		onProgress(0.9)
	}
	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Read the intensity from the per-run params, not the captured op: the registry caches one session per Id and the
	// captured op's intensity would be stale across runs with different intensities.
	blendedImg := utils.BlendWithIntensity(img, result, utils.IntensityFromParams(params))

	if onProgress != nil {
		onProgress(1)
	}

	return blendedImg, nil
}

// endregion
