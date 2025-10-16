package main

import (
	"fmt"
	"image"
	"image/color"
	"image/draw"
	_ "image/jpeg"
	"image/png"
	"math"
	"os"

	ort "github.com/yalue/onnxruntime_go"
)

func main() {
	ort.SetSharedLibraryPath("/Users/vegidio/Development/Source/open-photo-ai/libs/darwin_arm64/libonnxruntime.dylib")
	if err := ort.InitializeEnvironment(); err != nil {
		panic(err)
	}

	f, _ := os.Open("/Users/vegidio/Desktop/test2.jpg")
	img, _, _ := image.Decode(f)
	defer f.Close()

	// Prepare input tensor [1,3,H,W]
	data, h, w := toNCHW(img)
	inShape := ort.NewShape(1, 3, int64(h), int64(w))
	inTensor, err := ort.NewTensor[float32](inShape, data)
	if err != nil {
		panic(err)
	}
	defer inTensor.Destroy()

	// Prepare output tensor [1,3,4H,4W] (Real-ESRGAN x4)
	scale := int64(4)
	outShape := ort.NewShape(1, 3, scale*int64(h), scale*int64(w))
	outTensor, err := ort.NewEmptyTensor[float32](outShape)
	if err != nil {
		panic(err)
	}
	defer outTensor.Destroy()

	// Use your model’s real IO names here:
	inputName := "input"   // e.g. "input" or "input.1"
	outputName := "output" // e.g. "output" or "output.1"

	// Create session with names + tensors
	sess, err := ort.NewSession[float32](
		"/Users/vegidio/Development/Source/open-photo-ai/models/real-esrgan_4x_standard.onnx",
		[]string{inputName},
		[]string{outputName},
		[]*ort.Tensor[float32]{inTensor},
		[]*ort.Tensor[float32]{outTensor},
	)
	if err != nil {
		panic(err)
	}
	defer sess.Destroy()

	err = sess.Run()
	if err != nil {
		panic(err)
	}

	outImg, err := tensorToRGBA(outTensor)
	if err != nil {
		panic(err)
	}

	nf, _ := os.Create("/Users/vegidio/Desktop/test2_upscaled.png")
	defer nf.Close()
	png.Encode(nf, outImg)
}

func toNCHW(img image.Image) ([]float32, int, int) {
	rgba := image.NewRGBA(img.Bounds())
	draw.Draw(rgba, rgba.Bounds(), img, img.Bounds().Min, draw.Src)
	h, w := rgba.Bounds().Dy(), rgba.Bounds().Dx()
	data := make([]float32, 3*w*h)
	for y := 0; y < h; y++ {
		row := y * rgba.Stride
		for x := 0; x < w; x++ {
			i := row + 4*x
			idx := y*w + x
			data[0*w*h+idx] = float32(rgba.Pix[i+0]) / 255.0 // R
			data[1*w*h+idx] = float32(rgba.Pix[i+1]) / 255.0 // G
			data[2*w*h+idx] = float32(rgba.Pix[i+2]) / 255.0 // B
		}
	}
	return data, h, w
}

func tensorToRGBA(t *ort.Tensor[float32]) (*image.RGBA, error) {
	data := t.GetData()   // flat []float32
	shape := t.GetShape() // []int64, e.g. [1, 3, H, W]
	if len(shape) != 4 || shape[1] != 3 {
		return nil, fmt.Errorf("unexpected tensor shape: %v", shape)
	}

	h := int(shape[2])
	w := int(shape[3])
	planeSize := w * h
	expected := 3 * planeSize
	if len(data) < expected {
		return nil, fmt.Errorf("tensor data too short: got %d, need %d", len(data), expected)
	}

	rgba := image.NewRGBA(image.Rect(0, 0, w, h))
	rPlane := data[0*planeSize : 1*planeSize]
	gPlane := data[1*planeSize : 2*planeSize]
	bPlane := data[2*planeSize : 3*planeSize]

	for y := 0; y < h; y++ {
		for x := 0; x < w; x++ {
			i := y*w + x
			r := uint8(math.Round(math.Max(0, math.Min(1, float64(rPlane[i]))) * 255))
			g := uint8(math.Round(math.Max(0, math.Min(1, float64(gPlane[i]))) * 255))
			b := uint8(math.Round(math.Max(0, math.Min(1, float64(bPlane[i]))) * 255))
			rgba.SetRGBA(x, y, color.RGBA{r, g, b, 255})
		}
	}
	return rgba, nil
}
