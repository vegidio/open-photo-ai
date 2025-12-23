package main

import (
	"context"
	"fmt"
	"time"

	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/types"
	"github.com/vegidio/open-photo-ai/utils"
)

func main() {
	if err := opai.Initialize("open-photo-ai", nil); err != nil {
		fmt.Printf("Failed to initialize the AI runtime: %v\n", err)
		return
	}
	defer opai.Destroy()

	fileName := "test"

	inputData, err := utils.LoadImage("/Users/vegidio/Desktop/" + fileName + ".jpg")
	if err != nil {
		fmt.Printf("Failed to load the input image: %v\n", err)
		return
	}

	ops, err := opai.SuggestEnhancements(inputData)
	if err != nil {
		fmt.Printf("Failed to get enhancement suggestions: %v\n", err)
		return
	}

	ctx := context.Background()
	now := time.Now()

	outputData, err := opai.Process(ctx, inputData, func(name string, progress float64) {
		fmt.Printf("%s - Progress: %.1f%%\n", name, progress*100)
	}, ops...)

	if err != nil {
		fmt.Printf("Failed to upscale the image: %v\n", err)
		return
	}

	since := time.Since(now)
	fmt.Println("Time elapsed: ", since)

	_, err = utils.SaveImage(&types.ImageData{
		FilePath: "/Users/vegidio/Desktop/" + fileName + "_new.jpg",
		Pixels:   outputData.Pixels,
	}, types.FormatJpeg, 90)

	if err != nil {
		fmt.Printf("Failed to save the output image: %v\n", err)
		return
	}
}
