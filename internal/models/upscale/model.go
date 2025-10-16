package upscale

import (
	"github.com/vegidio/open-photo-ai/internal/types"
	"github.com/vegidio/open-photo-ai/internal/utils"
	ort "github.com/yalue/onnxruntime_go"
)

type Upscale struct {
	id      string
	name    string
	session *ort.DynamicAdvancedSession
	appName string
}

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model = (*Upscale)(nil)

func New(appName string) *Upscale {
	return &Upscale{
		id:      "upscale",
		name:    "Upscale",
		appName: appName,
	}
}

// region - Model methods

func (m *Upscale) Id() string {
	return m.id
}

func (m *Upscale) Name() string {
	return m.name
}

func (m *Upscale) IsLoaded() bool {
	return m.session != nil
}

func (m *Upscale) Load() error {
	var err error
	m.session, err = utils.CreateSession(
		m.appName,
		"real-esrgan_4x_standard.onnx",
		"upscale/1.0.0",
		nil,
	)

	if err != nil {
		return err
	}

	return nil
}

func (m *Upscale) Run(operation types.Operation, input types.InputData) (*types.OuputData, error) {
	//op := operation.(*OpUpscale)

	// Create the input tensor
	data, h, w := imageToNCHW(input.Pixels)
	inShape := ort.NewShape(1, 3, int64(h), int64(w))
	inTensor, err := ort.NewTensor[float32](inShape, data)
	if err != nil {
		return nil, err
	}
	defer inTensor.Destroy()

	// Create the output tensor with 4x upscaling
	scale := 4
	outShape := ort.NewShape(1, 3, int64(h*scale), int64(w*scale))
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

	return &types.OuputData{
		Pixels: img,
	}, nil
}

func (m *Upscale) Unload() {
	m.session.Destroy()
}

// endregion
