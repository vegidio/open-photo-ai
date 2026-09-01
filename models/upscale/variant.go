package upscale

import (
	"context"
	"fmt"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
)

// Variant is the complete description of one upscale model. Everything that distinguishes kyoto from saitama lives in
// this struct, so a per-variant package is a thin registration shim and adding a model is a data change rather than
// another copy of the same model/operation file pair.
//
// Two contracts live here. The convolutional one (kyoto, saitama, tokyo) loads one session per native scale pass and
// puts the scale in the operation Id. The diffusion one (osaka) loads a fixed set of named graphs and carries the
// scale per-run instead - see DiffusionSpec. Everything else, from the registry key down to the tiling, is shared.
type Variant struct {
	// Codename identifies the model in both its operation Id and the session loader.
	Codename string

	// Label is the display name shown in the UI, before the scale and precision suffixes are appended.
	Label string

	// ScaleBuckets drives the convolutional contract: it maps a requested scale onto the sequence of native passes
	// that cover it, which is what differs between a variant shipping native 2x and 4x weights and one shipping only
	// 4x. Ignored when Diffusion is set.
	ScaleBuckets []ScaleBucket

	// Diffusion, when non-nil, replaces the convolutional contract wholesale: a fixed set of named graphs, the scale
	// carried per-run rather than in the Id, and a variant-supplied Run.
	Diffusion *DiffusionSpec

	// Profile is the provider tuning this variant's passes need. A nil Profile means the provider defaults, which is
	// what a variant nobody has measured should get: the right settings follow the graph's op mix, so carrying one
	// variant's findings to another because both upscale is how a profile ends up pessimising a model it was never
	// measured against. Ignored when Diffusion is set - that contract carries its own DiffusionSpec.Profile.
	Profile func() utils.EPProfile
}

// profile returns the variant's provider tuning, or the zero value when it declares none.
func (v *Variant) profile() utils.EPProfile {
	if v.Profile == nil {
		return utils.EPProfile{}
	}

	return v.Profile()
}

// GraphSpec names one graph of a multi-stage variant and the tensors it takes and returns. The names are not shared
// with the rest of the codebase, which uses "input"/"output" everywhere: these graphs were exported with meaningful
// names.
type GraphSpec struct {
	// Role is how the pipeline asks for this graph, via Model.Graph. Binding by name rather than by position is
	// deliberate: binding by the order of the slice would make reordering that literal swap the encoder and the
	// decoder, which compiles, runs, and returns a wrong image with nothing to catch it.
	Role string

	// Suffix distinguishes this graph's artifact from the variant's base model id.
	Suffix string

	// Precision pins this graph's artifact regardless of the precision the operation asks for. Empty - the usual
	// case - means the graph follows the operation.
	//
	// It exists because a multi-stage model is not published as one precision throughout. Osaka's diffusion
	// transformer holds nearly all of its weight and is published both as fp16 and quantized to int8, while its two
	// VAE halves are convolutional, small by comparison, and published only as fp16 - one pair of files shared by
	// both builds. Without this the int8 build would resolve them to `_int8` names that do not exist.
	Precision types.Precision

	Inputs  []string
	Outputs []string
}

// modelId is the artifact this graph resolves to: the variant's codename, this graph's suffix, and a precision that
// is the operation's unless the graph pinned its own.
//
// It is a method rather than three lines inside newDiffusion so that the naming rule can be tested without opening a
// session, which for this family means a multi-gigabyte download.
func (g GraphSpec) modelId(codename string, precision types.Precision) string {
	if g.Precision != "" {
		precision = g.Precision
	}

	return fmt.Sprintf("up_%s%s_%s", codename, g.Suffix, precision)
}

// DiffusionSpec is the alternative contract, for a model that restores detail at a fixed resolution rather than
// scaling by an integer factor. It is a struct rather than a handful of optional fields on Variant so that the two
// contracts stay legible side by side.
type DiffusionSpec struct {
	// Graphs is the fixed set of stages loaded together for one pass.
	Graphs []GraphSpec

	// Profile is the provider tuning every graph of this variant needs.
	Profile func() utils.EPProfile

	// Run is the variant's own pipeline, given the loaded model and the per-run scale.
	Run func(ctx context.Context, m *Model, img image.Image, scale float64,
		onProgress types.InferenceProgress) (image.Image, error)
}

// Op builds this variant's operation at the given scale, clamped to the supported range.
//
// The precision is the caller's, whichever contract the variant uses: which precisions a model publishes is implicit
// in what exists on the remote, exactly as it is for the convolutional variants. A precision that was never published
// resolves to a model id with no manifest entry, which deps.ModelDependency downloads unverified and says so.
func (v *Variant) Op(scale float64, precision types.Precision) Op {
	return Op{
		variant:   v,
		precision: precision,
		scale:     ClampScale(scale),
	}
}

// New loads this variant's sessions. operation must be an Op produced by the same variant.
func (v *Variant) New(
	ctx context.Context,
	operation types.Operation,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*Model, error) {
	op, ok := operation.(Op)
	if !ok {
		return nil, errors.Errorf("expected an upscale operation, got %T", operation)
	}

	if v.Diffusion != nil {
		return v.newDiffusion(ctx, op, ep, onProgress)
	}

	scales := SelectScaleMatrix(op.scale, v.ScaleBuckets)

	sessions, err := LoadSessions(ctx, v.Codename, op.precision, scales, ep, v.profile(), onProgress)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to load the %s sessions", v.Codename)
	}

	return &Model{
		name:      FormatUpscaleName(v.Label, op.scale, op.precision),
		operation: op,
		variant:   v,
		Sessions:  sessions,
		scales:    scales,
	}, nil
}

// newDiffusion loads the fixed set of graphs a diffusion variant runs as one pass and binds each to its role.
func (v *Variant) newDiffusion(
	ctx context.Context,
	op Op,
	ep types.ExecutionProvider,
	onProgress types.DownloadProgress,
) (*Model, error) {
	specs := make([]utils.SessionSpec, 0, len(v.Diffusion.Graphs))
	for _, g := range v.Diffusion.Graphs {
		specs = append(specs, utils.SessionSpec{
			ModelId: g.modelId(v.Codename, op.precision),
			Inputs:  g.Inputs,
			Outputs: g.Outputs,
		})
	}

	// The shared loader destroys a partially-opened set for us, so a failure here leaks nothing - which matters most
	// for this kind of model, whose first graph alone can be nearly 7 GB.
	sessions, err := utils.LoadSessions(ctx, specs, ep, v.Diffusion.Profile(), onProgress)
	if err != nil {
		return nil, errors.Wrapf(err, "failed to load the %s sessions", v.Codename)
	}

	// LoadSessions returns the sessions in spec order, and specs was built from Graphs just above, so the two are
	// index-aligned by construction within this function - unlike the map below, which is read from another package
	// and so is keyed by role.
	graphs := make(map[string]*utils.Session, len(v.Diffusion.Graphs))
	for i, g := range v.Diffusion.Graphs {
		graphs[g.Role] = sessions[i]
	}

	return &Model{
		// No scale in the name, unlike the convolutional branch: one set of sessions serves every scale, so a name
		// naming one would be frozen at whichever scale happened to build the model first.
		name:      utils.FormatModelName(v.Label, op.precision),
		operation: op,
		variant:   v,
		Sessions:  sessions,
		graphs:    graphs,
		ep:        ep,
	}, nil
}

// region - Operation

// Op identifies an upscale run: the variant, the requested scale, and the precision together form the model identity.
//
// For a diffusion variant the scale is NOT part of the identity. SeedVR2-style models do not change resolution: the
// image is resampled to the target size first and the model restores detail at that size, so one set of sessions
// serves every scale. Since those sessions are over 7 GB, putting the scale in the Id would turn a scale change into
// a registry miss and a multi-gigabyte rebuild. It travels in Params instead, exactly as the denoise intensity does.
type Op struct {
	variant   *Variant
	precision types.Precision
	scale     float64
}

func (o Op) Id() string {
	if o.variant.Diffusion != nil {
		return fmt.Sprintf("up_%s_%s", o.variant.Codename, o.precision)
	}

	return fmt.Sprintf("up_%s_%.4gx_%s", o.variant.Codename, o.scale, o.precision)
}

func (o Op) Precision() types.Precision {
	return o.precision
}

// Params carries the per-run scale for a diffusion variant. A convolutional variant's scale is fully described by its
// Id, so it returns nil and Model.Run ignores the map entirely.
func (o Op) Params() map[string]any {
	if o.variant.Diffusion == nil {
		return nil
	}

	return map[string]any{ParamScale: o.scale}
}

// CacheKey folds the scale into the image cache key for a diffusion variant, so different scales don't collide on the
// shared Id. A convolutional variant contributes nothing: returning "" here leaves its cache key as Id() alone, which
// is what it has always been.
func (o Op) CacheKey() string {
	if o.variant.Diffusion == nil {
		return ""
	}

	return ScaleCacheKey(o.scale)
}

var (
	_ types.Operation     = (*Op)(nil)
	_ types.Parameterized = (*Op)(nil)
	_ types.CacheKeyer    = (*Op)(nil)
)

// endregion

// region - Model

// Model is the set of loaded sessions for one upscale variant, plus the pass sequence they are run in.
type Model struct {
	name      string
	operation Op
	variant   *Variant
	scales    []int
	ep        types.ExecutionProvider

	// graphs indexes the sessions by GraphSpec.Role, for diffusion variants. They are not counted twice:
	// ResidentBytes and Destroy are promoted from the embedded slice, which is the only owner.
	graphs map[string]*utils.Session

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

// Graph returns the session bound to role, as named by the variant's GraphSpec.
func (m *Model) Graph(role string) *utils.Session {
	return m.graphs[role]
}

// EP is the execution provider the sessions were opened on, which a variant's pipeline may need in order to warn
// about provider-specific limits.
func (m *Model) EP() types.ExecutionProvider {
	return m.ep
}

func (m *Model) Run(
	ctx context.Context,
	img image.Image,
	params map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	if d := m.variant.Diffusion; d != nil {
		// Read the scale from the per-run params, not the captured op: the registry caches one model per Id and the
		// captured op's scale would be stale across runs at different scales.
		return d.Run(ctx, m, img, ScaleFromParams(params), onProgress)
	}

	return RunPipeline(ctx, m.Sessions, img, m.scales, m.operation.scale, onProgress)
}

// endregion
