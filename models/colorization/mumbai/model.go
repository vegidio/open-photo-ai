package mumbai

import (
	"context"
	"fmt"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/deps"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/colorization"
	"github.com/vegidio/open-photo-ai/types"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

type Mumbai struct {
	name      string
	operation OpClMumbai
	*utils.Session
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Mumbai, error) {
	op := operation.(OpClMumbai)

	modelFile := op.Id() + ".onnx"
	name := fmt.Sprintf("Mumbai (%s)", cases.Upper(language.English).String(string(op.precision)))

	if err := deps.Install(ctx, deps.ModelDependency(op.Id()), onProgress); err != nil {
		return nil, errors.Wrap(err, "failed to prepare Mumbai model")
	}

	session, err := utils.CreateSession(
		modelFile,
		[]string{"input"},
		[]string{"output"},
		ep,
	)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create Mumbai session")
	}

	return &Mumbai{
		name:      name,
		operation: op,
		Session:   session,
	}, nil
}

var _ types.Model[image.Image] = (*Mumbai)(nil)
var _ types.Measurable = (*Mumbai)(nil)

// region - Model methods

func (m *Mumbai) Id() string {
	return m.operation.Id()
}

func (m *Mumbai) Name() string {
	return m.name
}

func (m *Mumbai) Run(
	ctx context.Context,
	img image.Image,
	_ map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	if onProgress != nil {
		onProgress("cl", 0)
	}
	if err := ctx.Err(); err != nil {
		return nil, errors.Wrap(err, "context cancelled")
	}

	result, err := colorization.Process(ctx, m.Session, img)
	if err != nil {
		return nil, errors.Wrap(err, "failed to process image")
	}

	if onProgress != nil {
		onProgress("cl", 1)
	}

	return result, nil
}

// endregion
