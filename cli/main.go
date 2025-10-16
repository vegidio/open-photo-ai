package main

import (
	"fmt"

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

	inputData, err := opai.LoadInputData("/Users/vegidio/Desktop/test2.jpg")
	if err != nil {
		fmt.Printf("Failed to load the input image: %v\n", err)
		return
	}

	outputData, err := opai.Process(inputData, upscale.Op(4, "high"))
	if err != nil {
		fmt.Printf("Failed to upscale the image: %v\n", err)
		return
	}

	err = opai.SaveOutputData(&types.OutputData{
		FilePath: "/Users/vegidio/Desktop/test2_upscaled_4x.png",
		Format:   "png",
		Pixels:   outputData.Pixels,
	})

	if err != nil {
		fmt.Printf("Failed to save the output image: %v\n", err)
		return
	}
}
