package jaipur

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

type Jaipur struct {
	name      string
	operation OpClJaipur
	*utils.Session
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Jaipur, error) {
	op := operation.(OpClJaipur)

	modelFile := op.Id() + ".onnx"
	name := fmt.Sprintf("Jaipur (%s)", cases.Upper(language.English).String(string(op.precision)))

	if err := deps.Install(ctx, deps.ModelDependency(op.Id()), onProgress); err != nil {
		return nil, errors.Wrap(err, "failed to prepare Jaipur model")
	}

	session, err := utils.CreateSession(
		modelFile,
		[]string{"input"},
		[]string{"output"},
		ep,
	)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create Jaipur session")
	}

	return &Jaipur{
		name:      name,
		operation: op,
		Session:   session,
	}, nil
}

var _ types.Model[image.Image] = (*Jaipur)(nil)
var _ types.Measurable = (*Jaipur)(nil)

// region - Model methods

func (m *Jaipur) Id() string {
	return m.operation.Id()
}

func (m *Jaipur) Name() string {
	return m.name
}

func (m *Jaipur) Run(
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

	result, err := colorization.ProcessDeOldify(ctx, m.Session, img)
	if err != nil {
		return nil, errors.Wrap(err, "failed to process image")
	}

	if onProgress != nil {
		onProgress("cl", 1)
	}

	return result, nil
}

// endregion
