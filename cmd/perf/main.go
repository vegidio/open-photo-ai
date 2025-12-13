package main

import (
	"context"
	_ "embed"
	"log"
	"time"

	"github.com/vegidio/go-sak/fs"
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/types"
)

const AppName = "open-photo-ai"

//go:embed test.dat
var testDataBinary []byte

func main() {
	log.SetFlags(log.LstdFlags | log.Lmicroseconds)

	if err := opai.Initialize(AppName); err != nil {
		log.Fatalf("Failed to initialize the model runtime: %v\n", err)
	}
	defer opai.Destroy()

	tempFile, cleanup, err := fs.MkTempFile("open-photo-ai-*", "test.jpg")
	if err != nil {
		log.Fatalf("Error creating temp file: %v\n", err)
	}
	defer cleanup()

	_, err = tempFile.Write(testDataBinary)
	if err != nil {
		log.Fatalf("Error writing temp file: %v\n", err)
	}

	inputData, err := opai.LoadImage(tempFile.Name())
	if err != nil {
		log.Fatalf("Failed to load the input image: %v\n", err)
	}

	startUpscaleTest(inputData)
}

func startUpscaleTest(inputData *types.ImageData) {
	// Warm-up; the first run is not included in the measurements because it's a cold-start
	log.Printf("UPSCALE: Warming up!\n")
	ctx := context.Background()

	op := kyoto.Op(kyoto.ModeGeneral, 4, types.PrecisionFp32)
	_, err := opai.Process(ctx, inputData, nil, op)
	if err != nil {
		log.Fatalf("Failed to upscale the image: %v\n", err)
	}

	now := time.Now()

	for i := 0; i < 5; i++ {
		log.Printf("Running test %d...\n", i+1)

		_, err = opai.Process(ctx, inputData, nil, op)
		if err != nil {
			log.Fatalf("Failed to upscale the image: %v\n", err)
		}
	}

	since := time.Since(now) / 5
	log.Printf("Time elapsed: %v", since)
}
