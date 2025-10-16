package main

import (
	"fmt"
	"image"
	_ "image/jpeg"
	"image/png"
	"os"

	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/internal/models/upscale"
	"github.com/vegidio/open-photo-ai/internal/types"
)

const AppName = "open-photo-ai"

func main() {
	if err := opai.Initialize(AppName); err != nil {
		fmt.Printf("Failed to initialize the model runtime: %v\n", err)
		return
	}

	defer opai.Destroy()

	inputFile, _ := os.Open("/Users/vegidio/Desktop/test2.jpg")
	img, _, _ := image.Decode(inputFile)
	defer inputFile.Close()

	inputData := types.InputData{
		Pixels: img,
	}

	outputData, err := opai.Process(inputData, upscale.Operation())
	if err != nil {
		fmt.Printf("Failed to process the image: %v\n", err)
		return
	}

	outputFile, _ := os.Create("/Users/vegidio/Desktop/test2_upscaled.png")
	defer outputFile.Close()
	png.Encode(outputFile, outputData.Pixels)
}
