package upscale

import (
	"fmt"
	"image"
	"image/draw"
	"math"

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

const (
	tileSize = 256 // Fixed size for all tiles (static shape for ONNX)
	tilePad  = 10  // Padding to avoid seam artifacts
)

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
	width := input.Pixels.Bounds().Dx()
	height := input.Pixels.Bounds().Dy()

	// Calculate output dimensions
	outWidth := width * m.operation.scale
	outHeight := height * m.operation.scale
	output := image.NewRGBA(image.Rect(0, 0, outWidth, outHeight))

	// Calculate step size (tile size minus overlap)
	stepSize := tileSize - 2*tilePad

	// Calculate the number of tiles needed
	tilesX := int(math.Ceil(float64(width) / float64(stepSize)))
	tilesY := int(math.Ceil(float64(height) / float64(stepSize)))

	for tileY := 0; tileY < tilesY; tileY++ {
		for tileX := 0; tileX < tilesX; tileX++ {
			// Calculate tile position in source image
			x1 := tileX * stepSize
			y1 := tileY * stepSize
			x2 := min(x1+tileSize, width)
			y2 := min(y1+tileSize, height)

			tile := extractTile(input.Pixels, x1, y1, x2, y2, tileSize, tileSize)
			upscaledTile, err := m.upscaleTile(tile)
			if err != nil {
				return nil, err
			}

			// Calculate the valid region to paste (excluding overlap and padding)
			validWidth := min(tileSize, x2-x1)
			validHeight := min(tileSize, y2-y1)

			// Calculate source rect in upscaled tile (skip overlap padding)
			srcX1 := 0
			srcY1 := 0
			if tileX > 0 {
				srcX1 = tilePad * m.operation.scale
			}
			if tileY > 0 {
				srcY1 = tilePad * m.operation.scale
			}

			srcX2 := validWidth * m.operation.scale
			srcY2 := validHeight * m.operation.scale
			if tileX < tilesX-1 {
				srcX2 -= tilePad * m.operation.scale
			}
			if tileY < tilesY-1 {
				srcY2 -= tilePad * m.operation.scale
			}

			// Calculate destination position in output
			dstX1 := x1 * m.operation.scale
			dstY1 := y1 * m.operation.scale
			if tileX > 0 {
				dstX1 += tilePad * m.operation.scale
			}
			if tileY > 0 {
				dstY1 += tilePad * m.operation.scale
			}

			dstX2 := min(dstX1+srcX2-srcX1, outWidth)
			dstY2 := min(dstY1+srcY2-srcY1, outHeight)

			// Paste the valid region
			srcRect := image.Rect(srcX1, srcY1, srcX2, srcY2)
			dstRect := image.Rect(dstX1, dstY1, dstX2, dstY2)

			draw.Draw(output, dstRect, upscaledTile, srcRect.Min, draw.Src)
		}
	}

	return &types.OutputData{
		Pixels: output,
	}, nil
}

func (m *Upscale) Destroy() {
	m.session.Destroy()
}

// endregion

// region - Private methods

func (m *Upscale) upscaleTile(tile image.Image) (*image.RGBA, error) {
	// Create the input tensor
	data, h, w := imageToNCHW(tile)
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

	// Run the inference
	if err = m.session.Run([]ort.Value{inTensor}, []ort.Value{outTensor}); err != nil {
		return nil, err
	}

	// Convert the output tensor to RGBA
	return tensorToRGBA(outTensor)
}

// endregion
