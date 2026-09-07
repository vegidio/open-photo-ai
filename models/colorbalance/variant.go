package colorbalance

import (
	"context"
	"fmt"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// Variant is the complete description of one colour balance model. Everything that distinguishes one from another lives in
// this struct, so a per-variant package is a thin registration shim and adding a model is a data change rather than
// another copy of the same model/operation file pair.
type Variant struct {
	// Codename identifies the model in both its operation Id (`cb_<codename>_<precision>`) and the session loader.
	Codename string

	// Label is the display name shown in the UI, before the precision suffix is appended.
	Label string

	// Canvas is the fixed square this variant's graph is exported at.
	Canvas Canvas

	// Profile is the provider tuning this variant needs. A nil Profile means the provider defaults, which is what a
	// variant nobody has measured should get: the right settings follow the graph's op mix, so carrying one
	// variant's findings to another because both correct colour is how a profile ends up pessimising a model it was
	// never measured against.
	Profile func(precision types.Precision) utils.EPProfile
}

// Canvas is the fixed square a variant's graph is exported at. It is per-variant data rather than a constant in
// process.go so that a variant can be re-exported at a different size without touching the shared geometry.
//
// A colour balance graph is fixed-shape for a different reason from the light adjustment ones, and the difference is
// worth knowing before changing this. Those architectures cannot be exported dynamically at all. This one can - it
// shipped that way - and the export is perfectly correct; what a dynamic export costs is the CoreML provider, which
// refuses every node of a graph whose spatial axes vary and hands the whole thing back to CPU kernels.
//
// The size costs less here than it does there, because the model's output is never shown. It is only used to fit the
// global polynomial in Process, and an 11-term fit over a few hundred thousand samples barely moves with the
// resolution it was sampled at - which is why a square canvas is affordable at all on a model that used to run at the
// image's own aspect ratio.
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
		return nil, errors.Errorf("expected a colour balance operation, got %T", operation)
	}

	session, err := utils.LoadSingleSession(ctx, "cb", v.Codename, op.precision, ep, onProgress,
		utils.ResolveProfile(v.Profile, op.precision))
	if err != nil {
		return nil, errors.Wrapf(err, "failed to load the %s session", v.Codename)
	}

	return &Model{
		name:      utils.FormatModelName(v.Label, v.Codename, op.precision),
		operation: op,
		variant:   v,
		Session:   session,
	}, nil
}

// region - Operation

// Op identifies a colour balance run. The variant and precision form the model identity; the intensity is deliberately not
// part of it, so the registry reuses a single session across every intensity the user drags through.
type Op struct {
	variant   *Variant
	intensity float32
	precision types.Precision
}

func (o Op) Id() string {
	return fmt.Sprintf("cb_%s_%s", o.variant.Codename, o.precision)
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

// Model is a loaded colour balance session together with the variant it was built from.
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
