package rio

import (
	"context"
	"fmt"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/colorbalance"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

type Rio struct {
	name      string
	operation OpCbRio
	session   *ort.DynamicAdvancedSession
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Rio, error) {
	op := operation.(OpCbRio)

	// The intensity is a per-run input (carried via Params), not part of the model identity, so op.Id() already
	// resolves directly to the model file name.
	modelFile := op.Id() + ".onnx"
	name := fmt.Sprintf("Rio (%s)", cases.Upper(language.English).String(string(op.precision)))
	url := fmt.Sprintf("%s/%s", internal.ModelBaseUrl, modelFile)

	fileCheck := &types.FileCheck{
		Path: modelFile,
		Hash: op.Hash(),
	}

	if err := utils.PrepareDependency(ctx, url, "models", fileCheck, onProgress); err != nil {
		return nil, errors.Wrap(err, "failed to prepare Rio model")
	}

	session, err := utils.CreateSession(
		modelFile,
		[]string{"input"},
		[]string{"output"},
		ep,
	)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create Rio session")
	}

	return &Rio{
		name:      name,
		operation: op,
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Rio)(nil)

// region - Model methods

func (m *Rio) Id() string {
	return m.operation.Id()
}

func (m *Rio) Name() string {
	return m.name
}

func (m *Rio) Run(
	ctx context.Context,
	img image.Image,
	params map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	if onProgress != nil {
		onProgress("cb", 0)
	}
	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	result, err := colorbalance.Process(ctx, m.session, img)
	if err != nil {
		return nil, errors.Wrap(err, "failed to process image")
	}

	if onProgress != nil {
		onProgress("cb", 0.9)
	}
	if err = ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	// Read the intensity from the per-run params, not the captured op: the registry caches one session per Id and the
	// captured op's intensity would be stale across runs with different intensities.
	blendedImg := utils.BlendWithIntensity(img, result, utils.IntensityFromParams(params))

	if onProgress != nil {
		onProgress("cb", 1)
	}

	return blendedImg, nil
}

func (m *Rio) Destroy() {
	m.session.Destroy()
}

// endregion
