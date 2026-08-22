package delhi

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

type Delhi struct {
	name      string
	operation OpClDelhi
	*utils.Session
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Delhi, error) {
	op := operation.(OpClDelhi)

	modelFile := op.Id() + ".onnx"
	name := fmt.Sprintf("Delhi (%s)", cases.Upper(language.English).String(string(op.precision)))

	if err := deps.Install(ctx, deps.ModelDependency(op.Id()), onProgress); err != nil {
		return nil, errors.Wrap(err, "failed to prepare Delhi model")
	}

	session, err := utils.CreateSession(
		modelFile,
		[]string{"input"},
		[]string{"output"},
		ep,
	)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create Delhi session")
	}

	return &Delhi{
		name:      name,
		operation: op,
		Session:   session,
	}, nil
}

var _ types.Model[image.Image] = (*Delhi)(nil)
var _ types.Measurable = (*Delhi)(nil)

// region - Model methods

func (m *Delhi) Id() string {
	return m.operation.Id()
}

func (m *Delhi) Name() string {
	return m.name
}

func (m *Delhi) Run(
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
