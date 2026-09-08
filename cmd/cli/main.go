// Command cli is a small end-to-end harness for the colorization models: it runs each one over an image at every
// requested precision and execution provider and writes the results out side by side.
//
// It exists to exercise the library the way an embedder would, which is why it goes through the public API only.
package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"path/filepath"
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
	// Every failure below has to reach the exit status, not just stdout. This used to return from main on each error
	// path, which printed the problem and then exited 0 - invisible to CI and to any shell that checked.
	if err := run(context.Background()); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func run(ctx context.Context) error {
	in := flag.String("in", "", "path to the input image (required)")
	out := flag.String("out", ".", "directory to write the results into")
	eps := flag.String("ep", "cpu", "comma-separated execution providers: auto, cpu, coreml, cuda, tensorrt, directml, openvino")
	precisions := flag.String("precision", "fp32,fp16", "comma-separated precisions: fp32, fp16, int8")
	quality := flag.Int("quality", 90, "JPEG quality of the written images")

	flag.Parse()

	if *in == "" {
		flag.Usage()
		return fmt.Errorf("-in is required")
	}

	providers, err := parseProviders(*eps)
	if err != nil {
		return err
	}

	wanted, err := parsePrecisions(*precisions)
	if err != nil {
		return err
	}

	// Set up file-based logging (rotated daily, kept 7 days); also activates the opai library logger.
	if logCloser, err := shared.SetupLogging(shared.AppName); err == nil {
		defer logCloser.Close()
	} else {
		// Without this the CLI runs on a discarding logger and every library log line vanishes, which looks exactly
		// like a library that logs nothing.
		fmt.Printf("Failed to set up file logging, continuing without it: %v\n", err)
	}

	// Every provider must actually run: the cache key is operation + input, so a cached CPU result would otherwise be
	// returned for the CoreML pass.
	opai.SetImageCacheEnabled(false)

	if err = opai.Initialize(ctx, shared.AppName, nil); err != nil {
		return fmt.Errorf("failed to initialize the AI runtime: %w", err)
	}
	defer opai.Destroy()

	inputData, err := utils.LoadImage(*in)
	if err != nil {
		return fmt.Errorf("failed to load %s: %w", *in, err)
	}

	if err = os.MkdirAll(*out, 0o755); err != nil {
		return fmt.Errorf("failed to create %s: %w", *out, err)
	}

	// An ordered slice rather than a map plus a separate key list: the run order is part of the definition, and a
	// model can't be added to one half and forgotten in the other.
	ops := []struct {
		name string
		op   func(types.Precision) types.Operation
	}{
		{"delhi", func(p types.Precision) types.Operation { return delhi.Op(p) }},
		{"mumbai", func(p types.Precision) types.Operation { return mumbai.Op(p) }},
		{"jaipur", func(p types.Precision) types.Operation { return jaipur.Op(p) }},
	}

	stem := strings.TrimSuffix(filepath.Base(*in), filepath.Ext(*in))

	for _, model := range ops {
		for _, precision := range wanted {
			for _, ep := range providers {
				tag := fmt.Sprintf("%s_%s_%s", model.name, precision, strings.ToLower(string(ep)))
				now := time.Now()

				outputData, err := opai.Process(ctx, inputData, ep, func(p types.Progress) {
					fmt.Printf("%s [%s %.0f%%] - Progress: %.1f%%\n", p.Operation, p.Phase, p.Fraction*100, p.Total*100)
				}, model.op(precision))
				if err != nil {
					return fmt.Errorf("%s: failed to colorize the image: %w", tag, err)
				}

				fmt.Printf("%s: time elapsed: %s\n", tag, time.Since(now))

				path := filepath.Join(*out, fmt.Sprintf("%s_%s.jpg", stem, tag))
				if _, err = utils.SaveImage(&types.ImageData{
					FilePath: path,
					Pixels:   outputData.Pixels,
				}, types.FormatJpeg, *quality); err != nil {
					return fmt.Errorf("%s: failed to save %s: %w", tag, path, err)
				}
			}
		}
	}

	return nil
}

// parseProviders resolves the -ep list. The names are matched case-insensitively against the published constants,
// whose own spelling ("CoreML", "TensorRT") is not what anyone types on a command line.
func parseProviders(list string) ([]types.ExecutionProvider, error) {
	known := []types.ExecutionProvider{
		types.ExecutionProviderAuto,
		types.ExecutionProviderCPU,
		types.ExecutionProviderCoreML,
		types.ExecutionProviderCUDA,
		types.ExecutionProviderTensorRT,
		types.ExecutionProviderDirectML,
		types.ExecutionProviderOpenVINO,
	}

	var out []types.ExecutionProvider

	for _, name := range splitList(list) {
		matched := false

		for _, ep := range known {
			if strings.EqualFold(name, string(ep)) {
				out = append(out, ep)
				matched = true

				break
			}
		}

		if !matched {
			return nil, fmt.Errorf("unknown execution provider %q", name)
		}
	}

	if len(out) == 0 {
		return nil, fmt.Errorf("-ep named no execution providers")
	}

	return out, nil
}

// parsePrecisions resolves the -precision list through the same validation the library uses everywhere else.
func parsePrecisions(list string) ([]types.Precision, error) {
	var out []types.Precision

	for _, name := range splitList(list) {
		p, err := types.ParsePrecision(strings.ToLower(name))
		if err != nil {
			return nil, err
		}

		out = append(out, p)
	}

	if len(out) == 0 {
		return nil, fmt.Errorf("-precision named no precisions")
	}

	return out, nil
}

func splitList(list string) []string {
	var out []string

	for _, part := range strings.Split(list, ",") {
		if part = strings.TrimSpace(part); part != "" {
			out = append(out, part)
		}
	}

	return out
}
