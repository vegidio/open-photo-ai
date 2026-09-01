package main

import (
	"context"
	_ "embed"
	"fmt"
	"log/slog"
	"maps"
	"os"
	"os/signal"
	"slices"
	"strings"
	"syscall"

	"github.com/charmbracelet/x/term"
	"github.com/urfave/cli/v3"
	"github.com/vegidio/go-sak/fs"
	opai "github.com/vegidio/open-photo-ai"
	"github.com/vegidio/open-photo-ai/shared"
	"github.com/vegidio/open-photo-ai/types"
	"github.com/vegidio/open-photo-ai/utils"
)

//go:embed test.dat
var testDataBinary []byte

// providers and precisions back both the flag validators and the string-to-value mapping, so a value can never pass
// validation and then fail to map.
var providers = map[string]types.ExecutionProvider{
	"auto":     types.ExecutionProviderAuto,
	"cpu":      types.ExecutionProviderCPU,
	"cuda":     types.ExecutionProviderCUDA,
	"tensorrt": types.ExecutionProviderTensorRT,
	"directml": types.ExecutionProviderDirectML,
	"openvino": types.ExecutionProviderOpenVINO,
	"coreml":   types.ExecutionProviderCoreML,
}

var precisions = map[string]types.Precision{
	"fp32": types.PrecisionFp32,
	"fp16": types.PrecisionFp16,

	// int8 is published for osaka alone. Asking any other model for it resolves to a file that does not exist.
	"int8": types.PrecisionInt8,
}

func main() {
	// The context is cancelled on Ctrl-C. Process and Execute both honour it, so an in-flight inference stops, the
	// sweep breaks out, and the partial summary still prints - which is why nothing here calls os.Exit directly.
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	cmd := &cli.Command{
		Name:      "perftest",
		Usage:     "benchmark open-photo-ai models",
		ArgsUsage: "[model...]",
		Description: "Benchmarks one inference pass per model against an embedded 640x640 sample image, reporting\n" +
			"the cold start (session build + one inference) and the steady-state distribution over --runs.\n\n" +
			"With no arguments every model in the catalog is benchmarked, which on a fresh machine downloads\n" +
			"several gigabytes of weights. Start with \"perftest list\" and a single model name.",
		Flags:    flags(),
		Action:   run,
		Commands: []*cli.Command{{Name: "list", Usage: "list the available models", Action: list}},
	}

	if err := cmd.Run(ctx, os.Args); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

// flags declares the run parameters. Every flag is Local: in urfave/cli v3 flags are persistent by default, which
// would leak all of these into "perftest list --help".
func flags() []cli.Flag {
	return []cli.Flag{
		&cli.IntFlag{
			Name: "runs", Aliases: []string{"n"}, Value: 5, Local: true,
			Usage: "number of timed runs per model",
			Validator: func(v int) error {
				if v < 1 {
					return fmt.Errorf("must be at least 1, got %d", v)
				}

				return nil
			},
		},
		&cli.IntFlag{
			Name: "warmup", Aliases: []string{"w"}, Value: 1, Local: true,
			Usage: "number of untimed warm-up runs; also what downloads a missing model",
			Validator: func(v int) error {
				if v < 0 {
					return fmt.Errorf("cannot be negative, got %d", v)
				}

				return nil
			},
		},
		&cli.StringFlag{
			Name: "provider", Aliases: []string{"p"}, Value: "auto", Local: true,
			Usage:            "execution provider: auto, cpu, cuda, tensorrt, directml, openvino or coreml",
			Validator:        oneOf(providers),
			ValidateDefaults: true,
		},
		&cli.StringFlag{
			Name: "precision", Value: "fp32", Local: true,
			Usage:            "model precision: fp32, fp16 or int8 (not every model publishes every precision)",
			Validator:        oneOf(precisions),
			ValidateDefaults: true,
		},
		&cli.FloatFlag{
			Name: "scale", Aliases: []string{"s"}, Value: 4, Local: true,
			Usage:     "upscale factor; ignored by every model that isn't an upscaler",
			Validator: inRange(1, 8),
		},
		&cli.FloatFlag{
			Name: "intensity", Aliases: []string{"i"}, Value: 1, Local: true,
			Usage: "blend intensity for the denoise/sharpen/light/color models; applied after inference, " +
				"so it barely moves the timing. Ignored by the other models",
			Validator: inRange(-1, 1),
		},
		&cli.BoolFlag{
			Name: "cache", Local: true,
			Usage: "keep the library's image cache on, so the timings include its PNG encode and disk write " +
				"— i.e. what Process costs a real caller",
		},
		&cli.BoolFlag{
			Name: "skip-verify", Local: true,
			Usage: "use the model files already on disk without checking them against the remote manifest, " +
				"so a re-exported model can be benchmarked before it is published",
		},
		&cli.BoolFlag{
			Name: "plain", Local: true,
			Usage: "force the line-by-line output instead of the live view (automatic when stdout isn't a terminal)",
		},
		&cli.BoolFlag{
			Name: "verbose", Aliases: []string{"v"}, Local: true,
			Usage: "print the library's debug log and every timed run in order; implies --plain",
		},
	}
}

func oneOf[T any](allowed map[string]T) func(string) error {
	// Sorted so the error message is stable: Go randomizes map iteration, and an error that reorders itself between
	// two runs of the same command reads like a different error.
	keys := slices.Sorted(maps.Keys(allowed))

	return func(s string) error {
		if _, ok := allowed[strings.ToLower(s)]; ok {
			return nil
		}

		return fmt.Errorf("invalid value %q, want one of: %s", s, strings.Join(keys, ", "))
	}
}

func inRange(low, high float64) func(float64) error {
	return func(v float64) error {
		if v < low || v > high {
			return fmt.Errorf("must be between %g and %g, got %g", low, high, v)
		}

		return nil
	}
}

func list(_ context.Context, _ *cli.Command) error {
	printCatalog()
	return nil
}

func run(ctx context.Context, cmd *cli.Command) error {
	// Resolved before anything else touches the runtime: a typo in a model name should fail in milliseconds, not
	// after the ONNX runtime and a few gigabytes of weights have been downloaded.
	selection, err := resolveSelection(cmd.Args().Slice())
	if err != nil {
		return err
	}

	verbose := cmd.Bool("verbose") || os.Getenv("OPAI_PERF_DEBUG") != ""

	// The library is silent unless a logger is installed. Opting in shows the per-run "cache hit"/"creating model"
	// lines, which is how you check the harness is measuring what you think it is. Installed before Initialize so the
	// runtime setup is logged too.
	if verbose {
		opai.SetLogger(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelDebug})))
	}

	cfg := config{
		provider:  providers[strings.ToLower(cmd.String("provider"))],
		precision: precisions[strings.ToLower(cmd.String("precision"))],
		scale:     cmd.Float("scale"),
		intensity: float32(cmd.Float("intensity")),
		runs:      cmd.Int("runs"),
		warmup:    cmd.Int("warmup"),
		cache:     cmd.Bool("cache"),
	}

	// Off by default: Process writes every result to the disk image cache, which means a PNG encode of the output
	// inside the call being timed. Measuring that as if it were inference is the single biggest distortion this tool
	// used to have.
	opai.SetImageCacheEnabled(cfg.cache)

	// Benchmarking a model that is not published yet is the whole point of re-exporting one: without this the
	// installer sees a hash that does not match the remote manifest and downloads the old weights back over it, and
	// the run silently measures the model being replaced rather than the one under test.
	if cmd.Bool("skip-verify") {
		opai.SetSkipModelVerification(true)
	}

	if err = opai.Initialize(ctx, shared.AppName, printDownloadProgress); err != nil {
		return fmt.Errorf("failed to initialize the model runtime: %w", err)
	}
	defer opai.Destroy()

	rec := &fallbackRecorder{}
	opai.SetFallbackHandler(rec.record)

	input, cleanup, err := loadInput()
	if err != nil {
		return err
	}
	defer cleanup()

	// Runs once, here, and is reused by the face-recovery models below. Reported in the header so the face-recovery
	// rows can be trusted: on an image with no faces they would be benchmarking nothing.
	cfg.faces = detectFacesOnce(ctx, input, cfg)

	printHeader(cfg, input, len(cfg.faces.faces), selection)

	// A live view needs a terminal it owns. --verbose writes the debug log to stderr, which would tear it apart, and
	// a redirected stdout has nothing to animate.
	live := !cmd.Bool("plain") && !verbose && isTerminal(os.Stdout)

	var results []result

	if live {
		tiles := &tileProgress{}
		cfg.onProgress = tiles.callback()

		results = runLive(ctx, func(listener sweepListener) []result {
			return sweep(ctx, selection, input, cfg, rec, listener)
		}, len(selection), tiles)
	} else {
		// No progress callback in plain mode: the models invoke it per tile, and anything that writes to the terminal
		// from inside the timed section would skew the measurement.
		results = sweep(ctx, selection, input, cfg, rec, plainListener{total: len(selection)})
	}

	outln()
	printSummary(results, cfg, input, verbose)

	// Counted without the models the user interrupted: a Ctrl-C is not a model failure, and reporting it as one would
	// make a cancelled run indistinguishable from a broken model in CI.
	failed := 0
	for _, r := range results {
		if !r.ok() && !r.interrupted() {
			failed++
		}
	}

	if ctx.Err() != nil {
		return cli.Exit("interrupted; the summary above covers the models that finished", 1)
	}

	if failed > 0 {
		return cli.Exit(fmt.Sprintf("%d model(s) failed", failed), 1)
	}

	return nil
}

// loadInput materializes the embedded sample image and decodes it.
func loadInput() (*types.ImageData, func(), error) {
	// The first argument is the directory, not a name pattern: "" means the system temp dir. The pattern needs the .jpg
	// suffix because LoadImage picks the decoder from the file extension.
	tempFile, cleanup, err := fs.MkTempFile("", "open-photo-ai-*.jpg")
	if err != nil {
		return nil, nil, fmt.Errorf("error creating temp file: %w", err)
	}

	if _, err = tempFile.Write(testDataBinary); err != nil {
		cleanup()
		return nil, nil, fmt.Errorf("error writing temp file: %w", err)
	}

	input, err := utils.LoadImage(tempFile.Name())
	if err != nil {
		cleanup()
		return nil, nil, fmt.Errorf("failed to load the input image: %w", err)
	}

	return input, cleanup, nil
}

// detectFacesOnce runs the sweep's single detection pass, before any model is benchmarked.
//
// It happens here rather than inside the face-recovery models because every model boundary drains the registry: a
// per-model detection would rebuild the detector's session each time. A failure is not fatal - the header simply omits
// the count, and the face-recovery models surface the real error when their turn comes.
func detectFacesOnce(ctx context.Context, input *types.ImageData, cfg config) facesResult {
	faces, err := detectFaces(ctx, input, cfg)

	// The registry is drained before the first model runs anyway, but doing it here keeps the detection session from
	// counting towards the first model's memory.
	opai.CleanRegistry()

	return facesResult{faces: faces, err: err}
}

// isTerminal reports whether the live view has a terminal to draw on.
//
// This asks the OS, rather than checking os.ModeCharDevice on the file info: /dev/null is a character device too, so
// the cheaper test says "terminal" for `perftest > /dev/null` and the live view then animates into the void.
func isTerminal(f *os.File) bool {
	return term.IsTerminal(f.Fd())
}
