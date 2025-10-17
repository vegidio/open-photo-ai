package upscale

import (
	"fmt"

	"github.com/vegidio/open-photo-ai/internal/types"
	"github.com/vegidio/open-photo-ai/internal/utils"
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
	name := fmt.Sprintf("Upscale %dx (%s)", op.scale, cases.Title(language.English).String(string(op.mode)))

	switch op.mode {
	case ModeCartoon:
		modelName = "real-esrgan_4x_cartoon.onnx"
	case ModeMedium:
		modelName = "real-esrgan_4x_medium.onnx"
	case ModeHigh:
		if op.scale == 2 {
			modelName = "real-esrgan_2x_high.onnx"
		} else {
			modelName = "real-esrgan_4x_high.onnx"
		}
	}

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
	inShape := ort.NewShape(1, 3, int64(h), int64(w))
	inTensor, err := ort.NewTensor[float32](inShape, data)
	if err != nil {
		return nil, err
	}
	defer inTensor.Destroy()

	// Create the output tensor with x upscaling
	outShape := ort.NewShape(1, 3, int64(h*m.operation.scale), int64(w*m.operation.scale))
	outTensor, err := ort.NewEmptyTensor[float32](outShape)
	if err != nil {
		return nil, err
	}
	defer outTensor.Destroy()

	if err = m.session.Run([]ort.Value{inTensor}, []ort.Value{outTensor}); err != nil {
		return nil, err
	}

	img, err := tensorToRGBA(outTensor)
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
