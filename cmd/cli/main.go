package main

import (
	"fmt"
	"time"

	"github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/types"
)

func main() {
	if err := opai.Initialize("open-photo-ai"); err != nil {
		fmt.Printf("Failed to initialize the AI runtime: %v\n", err)
		return
	}
	defer opai.Destroy()

	fileName := "test"

	inputData, err := opai.LoadImage("/Users/vegidio/Desktop/" + fileName + ".jpg")
	if err != nil {
		fmt.Printf("Failed to load the input image: %v\n", err)
		return
	}

	ops := []types.Operation{
		//newyork.Op(types.PrecisionFp32),
		//athens.Op(types.PrecisionFp32),
		//santorini.Op(types.PrecisionFp32),
		kyoto.Op(kyoto.ModeGeneral, 4, types.PrecisionFp32),
		//tokyo.Op(4, types.PrecisionFp32),
	}

	now := time.Now()
	outputData, err := opai.Process(inputData, func(name string, progress float64) {
		fmt.Printf("%s - Progress: %.1f%%\n", name, progress*100)
	}, ops...)

	if err != nil {
		fmt.Printf("Failed to upscale the image: %v\n", err)
		return
	}
	since := time.Since(now)
	fmt.Println("Time elapsed: ", since)

	err = opai.SaveImage(&types.ImageData{
		FilePath: "/Users/vegidio/Desktop/" + fileName + "_new.jpg",
		Pixels:   outputData.Pixels,
	}, types.FormatJpeg, 90)

	if err != nil {
		fmt.Printf("Failed to save the output image: %v\n", err)
		return
	}
}
