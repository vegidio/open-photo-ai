package main

import (
	"fmt"
	"time"

	"github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/upscale"
	"github.com/vegidio/open-photo-ai/types"
)

const AppName = "open-photo-ai"

func main() {
	if err := opai.Initialize(AppName); err != nil {
		fmt.Printf("Failed to initialize the AI runtime: %v\n", err)
		return
	}
	defer opai.Destroy()

	inputData, err := opai.LoadInputData("/Users/vegidio/Desktop/test1.jpg")
	if err != nil {
		fmt.Printf("Failed to load the input image: %v\n", err)
		return
	}

	op := upscale.Op(upscale.ModeGeneral, 4, types.PrecisionFp32)

	now := time.Now()
	outputData, err := opai.Execute(inputData, func(progress float32) {
		fmt.Printf("Progress: %.1f%%\n", progress*100)
	}, op)

	if err != nil {
		fmt.Printf("Failed to upscale the image: %v\n", err)
		return
	}
	since := time.Since(now)
	fmt.Println("Time elapsed: ", since)

	err = opai.SaveOutputData(&types.OutputData{
		FilePath: fmt.Sprintf("/Users/vegidio/Desktop/test1_%s.jpg", op.Id()),
		Pixels:   outputData.Pixels,
		Format:   types.FormatJpeg,
	}, 90)

	if err != nil {
		fmt.Printf("Failed to save the output image: %v\n", err)
		return
	}
}
