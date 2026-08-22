package main

import (
	"context"
	"fmt"
	"strings"
	"time"

	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/models/colorization/delhi"
	"github.com/vegidio/open-photo-ai/models/colorization/jaipur"
	"github.com/vegidio/open-photo-ai/models/colorization/mumbai"
	"github.com/vegidio/open-photo-ai/shared"
	"github.com/vegidio/open-photo-ai/types"
	"github.com/vegidio/open-photo-ai/utils"
)

func main() {
	ctx := context.Background()

	// Set up file-based logging (rotated daily, kept 7 days); also activates the opai library logger.
	if logCloser, err := shared.SetupLogging(shared.AppName); err == nil {
		defer logCloser.Close()
	}

	// Every provider must actually run: the cache key is operation + input, so a cached CPU result would otherwise be
	// returned for the CoreML pass.
	opai.SetImageCacheEnabled(false)

	if err := opai.Initialize(ctx, shared.AppName, nil); err != nil {
		fmt.Printf("Failed to initialize the AI runtime: %v\n", err)
		return
	}
	defer opai.Destroy()

	inputData, err := utils.LoadImage("/Users/vegidio/Desktop/test/bw.jpg")
	if err != nil {
		fmt.Printf("Failed to load the input image: %v\n", err)
		return
	}

	ops := map[string]func(types.Precision) types.Operation{
		"delhi":  func(p types.Precision) types.Operation { return delhi.Op(p) },
		"mumbai": func(p types.Precision) types.Operation { return mumbai.Op(p) },
		"jaipur": func(p types.Precision) types.Operation { return jaipur.Op(p) },
	}
	precisions := []types.Precision{types.PrecisionFp32, types.PrecisionFp16}
	providers := []types.ExecutionProvider{types.ExecutionProviderCPU, types.ExecutionProviderCoreML}

	for _, name := range []string{"delhi", "mumbai", "jaipur"} {
		for _, precision := range precisions {
			for _, ep := range providers {
				tag := fmt.Sprintf("%s_%s_%s", name, precision, strings.ToLower(string(ep)))
				now := time.Now()

				outputData, err := opai.Process(ctx, inputData, ep, func(p types.Progress) {
					fmt.Printf("%s [%s %.0f%%] - Progress: %.1f%%\n", p.Operation, p.Phase, p.Fraction*100, p.Total*100)
				}, ops[name](precision))
				if err != nil {
					fmt.Printf("%s: failed to colorize the image: %v\n", tag, err)
					return
				}

				fmt.Printf("%s: time elapsed: %s\n", tag, time.Since(now))

				_, err = utils.SaveImage(&types.ImageData{
					FilePath: "/Users/vegidio/Desktop/test/bw_go_" + tag + ".jpg",
					Pixels:   outputData.Pixels,
				}, types.FormatJpeg, 90)
				if err != nil {
					fmt.Printf("%s: failed to save the output image: %v\n", tag, err)
					return
				}
			}
		}
	}
}
