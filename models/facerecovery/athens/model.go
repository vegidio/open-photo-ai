package athens

import (
	"context"
	"fmt"
	"image"

	"github.com/cockroachdb/errors"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/models/facerecovery"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

const (
	fidelity = 1.0
	tileSize = 512
)

type Athens struct {
	name      string
	operation OpFrAthens
	session   *ort.DynamicAdvancedSession
}

func New(ctx context.Context, operation types.Operation, ep types.ExecutionProvider, onProgress types.DownloadProgress) (*Athens, error) {
	modelFile, err := facerecovery.LoadModel(ctx, operation, ep, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to load Athens model")
	}

	session, err := utils.CreateSession(
		modelFile,
		[]string{"input", "weight"},
		[]string{"output"},
		ep,
	)
	if err != nil {
		return nil, errors.Wrap(err, "failed to create Athens session")
	}

	modelName := fmt.Sprintf("Athens (%s)", cases.Upper(language.English).String(string(operation.Precision())))

	return &Athens{
		name:      modelName,
		operation: operation.(OpFrAthens),
		session:   session,
	}, nil
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[image.Image] = (*Athens)(nil)

// region - Model methods

func (m *Athens) Id() string {
	return m.operation.Id()
}

func (m *Athens) Name() string {
	return m.name
}

func (m *Athens) Run(
	ctx context.Context,
	img image.Image,
	params map[string]any,
	onProgress types.InferenceProgress,
) (image.Image, error) {
	// Faces are detected independently and passed in via params (see facerecovery.ParamFaces); the model no longer runs
	// face detection itself.
	faces, _ := params[facerecovery.ParamFaces].([]facedetection.Face)
	if len(faces) == 0 {
		return img, nil
	}

	result, err := facerecovery.RestoreFaces(ctx, m.session, img, faces, tileSize, fidelity, onProgress)
	if err != nil {
		return nil, errors.Wrap(err, "failed to restore faces")
	}

	return result, nil
}

func (m *Athens) Destroy() {
	m.session.Destroy()
}

// endregion
