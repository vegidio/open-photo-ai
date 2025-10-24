package upscale

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

type Upscale struct {
	id        string
	name      string
	operation OpUpscale
	session   *ort.DynamicAdvancedSession
	appName   string
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model = (*Upscale)(nil)

func New(appName string, operation types.Operation) (*Upscale, error) {
	var modelName string
	op := operation.(OpUpscale)
	name := fmt.Sprintf("Upscale %dx (%s, %s)",
		op.scale,
		cases.Title(language.English).String(string(op.mode)),
		cases.Upper(language.English).String(string(op.precision)),
	)

	modelName = op.Id() + ".onnx"
	session, err := utils.CreateSession(appName, modelName, "upscale/1.0.0", nil)
	if err != nil {
		return nil, err
	}

	return &Upscale{
		name:      name,
		operation: op,
		session:   session,
		appName:   appName,
	}, nil
}

// region - Model methods

func (m *Upscale) Id() string {
	return m.operation.Id()
}

func (m *Upscale) Name() string {
	return m.name
}

func (m *Upscale) Run(input *types.InputData) (*types.OutputData, error) {
	// Create the input tensor
	data, h, w := imageToNCHW(input.Pixels)
	inTensor, err := createTensor(data, h, w, m.operation.precision)
	if err != nil {
		return nil, err
	}
	defer inTensor[0].Destroy()

	// Create the output tensor with x upscaling
	outTensor, err := createEmptyTensor(h*m.operation.scale, w*m.operation.scale, m.operation.precision)
	if err != nil {
		return nil, err
	}
	defer outTensor[0].Destroy()

	if err = m.session.Run(inTensor, outTensor); err != nil {
		return nil, err
	}

	rawData, shape, err := valueToTensorData(outTensor, m.operation.precision)
	if err != nil {
		return nil, err
	}

	img, err := tensorToRGBA(rawData, shape)
	if err != nil {
		return nil, err
	}

	return &types.OutputData{
		Pixels: img,
	}, nil
}

func (m *Upscale) Destroy() {
	m.session.Destroy()
}

// endregion
