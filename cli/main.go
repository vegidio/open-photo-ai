package main

import (
	"fmt"
	"time"

	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

const AppName = "open-photo-ai"

func main() {
	if err := opai.Initialize(AppName); err != nil {
		fmt.Printf("Failed to initialize the model runtime: %v\n", err)
		return
	}
	defer opai.Destroy()

	inputData, err := opai.LoadInputData("/Users/vegidio/Desktop/test1.jpg")
	if err != nil {
		fmt.Printf("Failed to load the input image: %v\n", err)
		return
	}

	now := time.Now()
	outputData, err := opai.Execute(inputData, upscale.Op(4, upscale.ModeGeneral, types.PrecisionFp32))
	if err != nil {
		fmt.Printf("Failed to upscale the image: %v\n", err)
		return
	}
	since := time.Since(now)
	fmt.Println("Time elapsed: ", since)

	err = opai.SaveOutputData(&types.OutputData{
		FilePath: "/Users/vegidio/Desktop/test1_upscaled_4x.jpg",
		Pixels:   outputData.Pixels,
		Format:   types.FormatJpeg,
	}, 90)

	if err != nil {
		fmt.Printf("Failed to save the output image: %v\n", err)
		return
	}
}
