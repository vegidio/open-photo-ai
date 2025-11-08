package facedetection

import (
	"fmt"
	"image"
	"math"

	"github.com/disintegration/imaging"
	"github.com/vegidio/open-photo-ai/internal/utils"
	"github.com/vegidio/open-photo-ai/types"
	ort "github.com/yalue/onnxruntime_go"
	"golang.org/x/text/cases"
	"golang.org/x/text/language"
)

type FaceDetection struct {
	id        string
	name      string
	operation OpFaceDetection
	session   *ort.DynamicAdvancedSession
	appName   string
}

const (
	modelTag   = "face-detection/1.0.0" // The place where the models are stored
	tileSize   = 640                    // Fixed size for all tiles (static shape for ONNX)
	numAnchors = int64(16800)           // The maximum number of anchors that can be detected in a single image
)

// Compile-time assertion to ensure it conforms to the Model interface.
var _ types.Model[[]Face] = (*FaceDetection)(nil)

func New(appName string, operation types.Operation) (*FaceDetection, error) {
	op := operation.(OpFaceDetection)
	modelFile := op.Id() + ".onnx"
	name := fmt.Sprintf("Face Detection (%s)",
		cases.Upper(language.English).String(string(op.precision)),
	)

	// Download the model, if needed
	url := fmt.Sprintf("https://github.com/vegidio/open-photo-ai/releases/download/%s/%s", modelTag, modelFile)
	if err := utils.PrepareDependency(appName, url, "models", modelFile, nil); err != nil {
		return nil, err
	}

	session, err := utils.CreateSession(appName, modelFile, []string{"input"}, []string{"loc", "conf", "landmarks"})
	if err != nil {
		return nil, err
	}

	return &FaceDetection{
		name:      name,
		operation: op,
		session:   session,
		appName:   appName,
	}, nil
}

// region - Model methods

func (m *FaceDetection) Id() string {
	return m.operation.Id()
}

func (m *FaceDetection) Name() string {
	return m.name
}

func (m *FaceDetection) Run(input *types.InputImage, onProgress func(float32)) ([]Face, error) {
	// Prepare input image
	canvas, scale, offsetX, offsetY := m.prepareInputImage(input)

	// Run inference
	locData, confData, landmarksData, locShape, confShape, landmarksShape, err := m.runInference(canvas)
	if err != nil {
		return nil, err
	}

	// Decode predictions
	faces := m.decodePredictions(locData, confData, landmarksData, locShape, confShape, landmarksShape, scale, offsetX, offsetY)

	// Apply Non-Maximum Suppression
	return filterOverlappingFaces(faces, 0.4), nil
}

func (m *FaceDetection) Destroy() {
	m.session.Destroy()
}

// endregion

// region - Private methods

func (m *FaceDetection) prepareInputImage(input *types.InputImage) (image.Image, float32, int, int) {
	bounds := input.Pixels.Bounds()
	origW, origH := bounds.Dx(), bounds.Dy()

	// Calculate scale to fit within 640x640 while maintaining the aspect ratio
	scaleW := float32(tileSize) / float32(origW)
	scaleH := float32(tileSize) / float32(origH)
	scale := float32(math.Min(float64(scaleW), float64(scaleH)))
	newW := int(float32(origW) * scale)
	newH := int(float32(origH) * scale)

	// Early return if the image is already the right size
	if newW == tileSize && newH == tileSize {
		return input.Pixels, scale, 0, 0
	}

	// Calculate the offsets
	offsetX := (tileSize - newW) >> 1
	offsetY := (tileSize - newH) >> 1

	// Resize the image to fit inside a 640x640 tile
	resized := imaging.Resize(input.Pixels, newW, newH, imaging.Lanczos)

	// Create canvas and paste with padding
	canvas := imaging.New(tileSize, tileSize, image.Black)
	canvas = imaging.Paste(canvas, resized, image.Pt(offsetX, offsetY))

	return canvas, scale, offsetX, offsetY
}

func (m *FaceDetection) runInference(canvas image.Image) ([]float32, []float32, []float32, []int64, []int64, []int64, error) {
	// Use ImageToNCHW for preprocessing
	inputData, _, _ := utils.ImageToNCHW(canvas)

	// Apply model-specific normalization in-place
	const norm = float32(255)
	const offset = float32(127.5)
	for i := 0; i < len(inputData); i++ {
		inputData[i] = inputData[i]*norm - offset
	}

	inShape := ort.NewShape(1, 3, int64(tileSize), int64(tileSize))
	inTensor, err := ort.NewTensor(inShape, inputData)
	if err != nil {
		return nil, nil, nil, nil, nil, nil, err
	}
	defer inTensor.Destroy()

	// Create output tensors
	locTensor, confTensor, landmarksTensor, err := m.createOutputTensors()
	if err != nil {
		return nil, nil, nil, nil, nil, nil, err
	}
	defer locTensor.Destroy()
	defer confTensor.Destroy()
	defer landmarksTensor.Destroy()

	// Run the inference
	if err = m.session.Run([]ort.Value{inTensor}, []ort.Value{locTensor, confTensor, landmarksTensor}); err != nil {
		return nil, nil, nil, nil, nil, nil, err
	}

	// Extract outputs
	locData, locShape := locTensor.GetData(), locTensor.GetShape()
	confData, confShape := confTensor.GetData(), confTensor.GetShape()
	landmarksData, landmarksShape := landmarksTensor.GetData(), landmarksTensor.GetShape()

	return locData, confData, landmarksData, locShape, confShape, landmarksShape, nil
}

func (m *FaceDetection) generatePriors() [][4]float32 {
	steps := []int{8, 16, 32}
	minSizes := [][]int{{16, 32}, {64, 128}, {256, 512}}
	totalPriors := 0

	for i, step := range steps {
		gridSize := tileSize / step
		totalPriors += gridSize * gridSize * len(minSizes[i])
	}

	priors := make([][4]float32, 0, totalPriors)
	invTileSize := 1.0 / float32(tileSize)

	for i, step := range steps {
		stepFloat := float32(step)
		gridSize := tileSize / step

		for y := 0; y < gridSize; y++ {
			cyBase := (float32(y) + 0.5) * stepFloat * invTileSize

			for x := 0; x < gridSize; x++ {
				cx := (float32(x) + 0.5) * stepFloat * invTileSize

				for _, minSize := range minSizes[i] {
					s := float32(minSize) * invTileSize
					priors = append(priors, [4]float32{cx, cyBase, s, s})
				}
			}
		}
	}

	return priors
}

func (m *FaceDetection) decodePredictions(
	locData, confData, landmarksData []float32,
	locShape, confShape, landmarksShape []int64,
	scale float32, offsetX, offsetY int,
) []Face {
	numAnchorsActual := locShape[1]
	priors := m.generatePriors()
	faces := make([]Face, 0, 32)
	confidenceThreshold := float32(0.5)
	confStride := confShape[2]

	maxI := numAnchorsActual
	if maxI > int64(len(priors)) {
		maxI = int64(len(priors))
	}

	for i := int64(0); i < maxI; i++ {
		// Get confidence score
		confIdx := i*confStride + 1
		if confIdx >= int64(len(confData)) {
			break
		}
		confidence := confData[confIdx]

		if confidence > confidenceThreshold {
			face := m.decodeFace(i, locData, landmarksData, locShape, landmarksShape, priors[i], confidence, scale, offsetX, offsetY)
			faces = append(faces, face)
		}
	}

	return faces
}

func (m *FaceDetection) decodeFace(
	i int64,
	locData, landmarksData []float32,
	locShape, landmarksShape []int64,
	prior [4]float32,
	confidence, scale float32,
	offsetX, offsetY int,
) Face {
	// Decode bounding box
	locIdx := i * locShape[2]
	if locIdx+3 >= int64(len(locData)) {
		return Face{}
	}

	offsetXFloat := float32(offsetX)
	offsetYFloat := float32(offsetY)
	tileSizeFloat := float32(tileSize)
	invScale := 1.0 / scale
	priorW := prior[2]
	priorH := prior[3]

	// Decode center and size
	cx := locData[locIdx]*0.1*priorW + prior[0]
	cy := locData[locIdx+1]*0.1*priorH + prior[1]
	w := priorW * float32(math.Exp(float64(locData[locIdx+2]*0.2)))
	h := priorH * float32(math.Exp(float64(locData[locIdx+3]*0.2)))

	// Calculate half-widths once
	halfW := w * 0.5
	halfH := h * 0.5

	// Convert to pixel coordinates and scale back
	x1 := int(((cx-halfW)*tileSizeFloat - offsetXFloat) * invScale)
	y1 := int(((cy-halfH)*tileSizeFloat - offsetYFloat) * invScale)
	x2 := int(((cx+halfW)*tileSizeFloat - offsetXFloat) * invScale)
	y2 := int(((cy+halfH)*tileSizeFloat - offsetYFloat) * invScale)

	// Decode landmarks
	landmarks := m.decodeLandmarks(i, landmarksData, landmarksShape, prior, scale, offsetX, offsetY)

	return Face{
		Box:        image.Rect(x1, y1, x2, y2),
		Confidence: confidence,
		Landmarks:  landmarks,
	}
}

func (m *FaceDetection) decodeLandmarks(
	i int64,
	landmarksData []float32,
	landmarksShape []int64,
	prior [4]float32,
	scale float32,
	offsetX, offsetY int,
) [][2]float32 {
	landmarkIdx := i * landmarksShape[2]
	landmarks := make([][2]float32, 0, 5)

	if landmarkIdx+9 >= int64(len(landmarksData)) {
		return landmarks
	}

	// Pre-calculate commonly used values
	priorW := prior[2]
	priorH := prior[3]
	priorCx := prior[0]
	priorCy := prior[1]
	offsetXFloat := float32(offsetX)
	offsetYFloat := float32(offsetY)
	tileSizeFloat := float32(tileSize)
	invScale := 1.0 / scale

	for j := range 5 {
		idx := landmarkIdx + int64(j*2)
		lx := landmarksData[idx]*0.1*priorW + priorCx
		ly := landmarksData[idx+1]*0.1*priorH + priorCy

		// Account for padding and scale back to the original size
		lx = (lx*tileSizeFloat - offsetXFloat) * invScale
		ly = (ly*tileSizeFloat - offsetYFloat) * invScale

		landmarks = append(landmarks, [2]float32{lx, ly})
	}

	return landmarks
}

func (m *FaceDetection) createOutputTensors() (*ort.Tensor[float32], *ort.Tensor[float32], *ort.Tensor[float32], error) {
	locTensor, err := ort.NewEmptyTensor[float32](ort.NewShape(1, numAnchors, 4))
	if err != nil {
		return nil, nil, nil, err
	}

	confTensor, err := ort.NewEmptyTensor[float32](ort.NewShape(1, numAnchors, 2))
	if err != nil {
		return nil, nil, nil, err
	}

	landmarksTensor, err := ort.NewEmptyTensor[float32](ort.NewShape(1, numAnchors, 10))
	if err != nil {
		return nil, nil, nil, err
	}

	return locTensor, confTensor, landmarksTensor, nil
}

// endregion
