package main

import (
	"context"
	_ "embed"
	"fmt"
	"log"
	"log/slog"
	"os"
	"time"

	"github.com/vegidio/go-sak/fs"
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/upscale/kyoto"
	"github.com/vegidio/open-photo-ai/types"
	"github.com/vegidio/open-photo-ai/utils"
)

const AppName = "open-photo-ai"

//go:embed test.dat
var testDataBinary []byte

func main() {
	log.SetFlags(log.LstdFlags | log.Lmicroseconds)

	// The library is silent unless a logger is installed. Opting in shows the per-run "cache hit"/"creating model"
	// lines, which is how you check the harness is measuring what you think it is.
	if os.Getenv("OPAI_PERF_DEBUG") != "" {
		opai.SetLogger(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelDebug})))
	}

	if err := opai.Initialize(context.Background(), AppName, nil); err != nil {
		log.Fatalf("Failed to initialize the model runtime: %v\n", err)
	}
	defer opai.Destroy()

	// First argument is the directory, not a name pattern: "" means the system temp dir. The pattern needs the .jpg
	// suffix because LoadImage picks the decoder from the file extension.
	tempFile, cleanup, err := fs.MkTempFile("", "open-photo-ai-*.jpg")
	if err != nil {
		log.Fatalf("Error creating temp file: %v\n", err)
	}
	defer cleanup()

	_, err = tempFile.Write(testDataBinary)
	if err != nil {
		log.Fatalf("Error writing temp file: %v\n", err)
	}

	inputData, err := utils.LoadImage(tempFile.Name())
	if err != nil {
		log.Fatalf("Failed to load the input image: %v\n", err)
	}

	startUpscaleTest(inputData)
}

func startUpscaleTest(inputData *types.ImageData) {
	// Warm-up; the first run is not included in the measurements because it's a cold-start
	log.Printf("UPSCALE: Warming up!\n")
	ctx := context.Background()

	op := kyoto.Op(4, types.PrecisionFp32)
	baseHash := inputData.Hash

	// Process short-circuits on a hit in the disk image cache, which is keyed by image hash + operations and outlives
	// the process (500 entries / 1 GB / 24 h). Reusing one hash made every run after the warm-up a cache read rather
	// than an inference, and made the warm-up itself a cache read on the second invocation - so nothing was measuring
	// what it claimed to. Each run therefore gets a hash unique to this invocation. The pixels are untouched, so every
	// run does identical work; only the cache key differs.
	runId := fmt.Sprintf("%s-perf-%d", baseHash, time.Now().UnixNano())

	// The cold start covers building the ONNX session (graph optimization, and the provider's own compilation: the
	// cuDNN algo search on CUDA, an engine build on TensorRT, an MLProgram compile on CoreML) plus one inference.
	// The gap between this and the steady-state figure below is what makes keeping models resident worth it.
	inputData.Hash = runId + "-warmup"
	coldStart := time.Now()
	_, err := opai.Process(ctx, inputData, types.ExecutionProviderAuto, nil, op)
	if err != nil {
		log.Fatalf("Failed to upscale the image: %v\n", err)
	}
	coldElapsed := time.Since(coldStart)

	now := time.Now()

	for i := 0; i < 5; i++ {
		log.Printf("Running test %d...\n", i+1)

		inputData.Hash = fmt.Sprintf("%s-%d", runId, i)

		_, err = opai.Process(ctx, inputData, types.ExecutionProviderAuto, nil, op)
		if err != nil {
			log.Fatalf("Failed to upscale the image: %v\n", err)
		}
	}

	inputData.Hash = baseHash
	log.Printf("Cold start (session build + first run): %v\n", coldElapsed)

	since := time.Since(now) / 5
	log.Printf("Time elapsed: %v", since)
}
