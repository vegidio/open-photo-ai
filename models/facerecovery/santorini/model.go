package santorini

import (
	"context"
	"fmt"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/detection"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

const (
	tileSize = 512
)

type Santorini struct {
	name      string
	operation OpFrSantorini
	session   *utils.Session
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Santorini, error) {
	modelFile, err := facerecovery.LoadModel(ctx, operation, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Santorini model")
	}

	session, err := utils.CreateSession(
		modelFile,
		[]string{"input"},
		[]string{"output"},
		ep,
	)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create Santorini session")
	}

	modelName := fmt.Sprintf("Santorini (%s)", cases.Upper(language.English).String(string(operation.Precision())))

	return &Santorini{
		name:      modelName,
		operation: operation.(OpFrSantorini),
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Santorini)(nil)
var _ types.Measurable = (*Santorini)(nil)

// region - Model methods

func (m *Santorini) Id() string {
	return m.operation.Id()
}

func (m *Santorini) Name() string {
	return m.name
}

func (m *Santorini) Run(
	ctx context.Context,
	img image.Image,
	params map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	// Faces are detected independently and passed in via params (see facerecovery.ParamFaces); the model no longer runs
	// face detection itself.
	faces, _ := params[facerecovery.ParamFaces].([]detection.Face)
	if len(faces) == 0 {
		return img, nil
	}

	result, err := facerecovery.RestoreFaces(ctx, m.session, img, faces, tileSize, -1, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to restore faces")
	}

	return result, nil
}

// ResidentBytes reports the size of the model files backing this session, which is what the registry
// budgets against when deciding what can stay loaded.
func (m *Santorini) ResidentBytes() int64 {
	return m.session.Bytes()
}

func (m *Santorini) Destroy() {
	m.session.Destroy()
}

// endregion
