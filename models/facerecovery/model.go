package facerecovery

import (
	"fmt"
	"image"
	"image/draw"

	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/models/facedetection"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

type FaceRecovery struct {
	id        string
	name      string
	operation OpFaceRecovery
	session   *ort.DynamicAdvancedSession
	fdModel   types.Model[[]facedetection.Face]
	appName   string
}

const (
	modelTag = "face-recovery/1.0.0" // The place where the models are stored
	tileSize = 512                   // Fixed size for all tiles (static shape for ONNX)
)

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[*types.OutputImage] = (*FaceRecovery)(nil)

func New(appName string, operation types.Operation) (*FaceRecovery, error) {
	// Init the Face Detection model, which is a dependency of this model
	fdModel, err := getFdModel(appName)
	if err != nil {
		return nil, err
	}

	op := operation.(OpFaceRecovery)
	modelFile := op.Id() + ".onnx"
	name := fmt.Sprintf("Face Recovery (%s, %s)",
		cases.Title(language.English).String(string(op.mode)),
		cases.Upper(language.English).String(string(op.precision)),
	)

	// Download the model, if needed
	url := fmt.Sprintf("https://github.com/vegidio/open-photo-ai/releases/download/%s/%s", modelTag, modelFile)
	if err = utils.PrepareDependency(appName, url, "models", modelFile, nil); err != nil {
		return nil, err
	}

	session, err := utils.CreateSession(appName, modelFile, []string{"input"}, []string{"output"})
	if err != nil {
		return nil, err
	}

	return &FaceRecovery{
		name:      name,
		operation: op,
		session:   session,
		fdModel:   fdModel,
		appName:   appName,
	}, nil
}

// region - Model methods

func (m *FaceRecovery) Id() string {
	return m.operation.Id()
}

func (m *FaceRecovery) Name() string {
	return m.name
}

func (m *FaceRecovery) Run(input *types.InputImage, onProgress func(float32)) (*types.OutputImage, error) {
	// Create a copy of the original image for pasting restored faces
	resultImg := image.NewRGBA(input.Pixels.Bounds())
	draw.Draw(resultImg, resultImg.Bounds(), input.Pixels, image.Point{}, draw.Src)

	// First, we detect the faces that need to be recovered
	faces, err := m.fdModel.Run(input, nil)
	if err != nil {
		return nil, err
	}

	if len(faces) == 0 {
		return &types.OutputImage{Pixels: resultImg}, nil
	}

	// Pre-allocate tensor data buffer to reuse across all faces
	tensorData := make([]float32, 3*tileSize*tileSize)

	// Process each detected face
	for _, face := range faces {
		// Add 30% padding around face bounds
		paddedRect := addPadding(face.Box, input.Pixels.Bounds(), 0.3)

		// Make the padded rectangle square
		squareRect := makeSquare(paddedRect, input.Pixels.Bounds())
		squareSize := squareRect.Dx()

		// Crop square face region from the original image
		croppedFace := imaging.Crop(input.Pixels, squareRect)
		scaledFace := imaging.Resize(croppedFace, tileSize, tileSize, imaging.Lanczos)

		// Convert image to tensor with normalization to [-1, 1]
		// Reuse the pre-allocated tensorData buffer
		data, _, _ := utils.ImageToNCHW(scaledFace)
		copy(tensorData, data)
		for j := range tensorData {
			tensorData[j] = tensorData[j]*2.0 - 1.0
		}

		inTensor, tErr := ort.NewTensor(ort.NewShape(1, 3, tileSize, tileSize), tensorData)
		if tErr != nil {
			return nil, tErr
		}

		outTensor, tErr := ort.NewEmptyTensor[float32](ort.NewShape(1, 3, tileSize, tileSize))
		if tErr != nil {
			return nil, tErr
		}

		iErr := m.session.Run([]ort.Value{inTensor}, []ort.Value{outTensor})
		if iErr != nil {
			return nil, iErr
		}

		// Convert tensor to image with denormalization from [-1, 1] to [0, 1]
		outData := outTensor.GetData()
		for j := range outData {
			outData[j] = (outData[j] + 1.0) / 2.0
		}

		// Scale back to the original square size
		restoredFace, _ := utils.TensorToRGBA(outTensor)
		scaledBack := imaging.Resize(restoredFace, squareSize, squareSize, imaging.Lanczos)

		// Paste restored face back to the result image
		draw.Draw(resultImg, squareRect, scaledBack, image.Point{}, draw.Src)

		inTensor.Destroy()
		outTensor.Destroy()
	}

	return &types.OutputImage{
		Pixels: resultImg,
	}, nil
}

func (m *FaceRecovery) Destroy() {
	m.session.Destroy()
	if m.fdModel != nil {
		m.fdModel.Destroy()
	}
}

// endregion

// region - Private functions

func getFdModel(appName string) (types.Model[[]facedetection.Face], error) {
	var model interface{}
	var err error
	faceOp := facedetection.Op(types.PrecisionFp32)

	model, exists := internal.Registry[faceOp.Id()]
	if !exists {
		model, err = facedetection.New(appName, faceOp)
		if err != nil {
			return nil, err
		}

		internal.Registry[faceOp.Id()] = model
	}

	return model.(types.Model[[]facedetection.Face]), nil
}

// endregion
