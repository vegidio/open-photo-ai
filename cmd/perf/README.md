# perftest

A benchmark harness for the open-photo-ai inference models. It runs one model at a time against an embedded 640x640
sample image and reports how long a single `opai.Process` call takes, both cold and in steady state.

```
perftest [options] [model...]
perftest list
```

## What it measures

Each model is benchmarked in four stages:

| Stage                            | Timed   | What it covers                                                                                                                                                                                                                            |
|----------------------------------|---------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Warm-up (`--warmup`, default 1)  | no      | Downloads the model if it isn't on disk yet, and builds a first session                                                                                                                                                                   |
| Cold start                       | **yes** | The registry is drained, then one full run: session construction (graph optimisation plus the provider's own compilation — the cuDNN algo search on CUDA, an engine build on TensorRT, an MLProgram compile on CoreML) plus one inference |
| Timed runs (`--runs`, default 5) | **yes** | Steady state, reusing the session the cold start built                                                                                                                                                                                    |

The cold start is measured **after** the warm-up, so it never includes a model download. That is what makes it
comparable between a fresh machine and a warm one — and it is why the first run of a model is slow in wall-clock terms
without that showing up in the reported number.

Two things about the numbers are worth knowing before you trust them:

- **The image cache is off by default.** `opai.Process` normally memoizes its result on disk, which means PNG-encoding
  the output and writing it inside the call being timed. For a 4x upscale of the sample that is a 2560x2560 PNG per
  run: measured on an M-series Mac, kyoto's median goes from 2.48 s to 2.93 s with the cache on, so roughly 15% of the
  cached figure is encoding rather than inference. `perftest` disables the cache via `opai.SetImageCacheEnabled(false)`
  so the timings are inference. Pass `--cache` to put it back and measure what `Process` actually costs a caller.
- **The registry is drained between models.** Every model gets a genuine cold start, GPU memory doesn't accumulate
  across a sweep, and a GPU→CPU fallback is attributed to the model that actually hit it.

## Build and run

```bash
task perf arch=arm64          # or arch=amd64; produces build/perftest
./build/perftest list
```

The first run downloads the ONNX runtime, and every model you benchmark downloads its weights — a full sweep pulls
several gigabytes. Start with `list` and a single model name.

## Scenarios

**See what can be benchmarked.** No runtime initialisation, so it is instant:

```bash
./build/perftest list
```

**Benchmark one model.** Use this when you have changed something and want a number for it:

```bash
./build/perftest saitama
```

**Benchmark several.** They are swept in catalog order and share one summary table:

```bash
./build/perftest saitama kyoto stockholm
```

**Benchmark everything.** Use this for a full picture of the machine:

```bash
./build/perftest
```

Budget hours, not minutes. Some models are far slower than the rest on some platforms — `osaka` is orders of magnitude
slower than the others without a capable GPU, and every model is slow on the CPU provider — so a full sweep is an
overnight job rather than something to run while you wait. Name the models you care about instead when you just want a
number.

**Compare models of the same type.** Fix the parameter so the comparison is fair:

```bash
./build/perftest -s 2 tokyo kyoto saitama
```

**Quick smoke run.** Fewest inferences that still produces a number, for checking the harness itself:

```bash
./build/perftest -n 3 -w 0 kyoto
```

With `-w 0` there is no warm-up, so on a machine that doesn't have the model yet the cold start includes the download.
The header says so when you use it.

**A statistically calmer number.** More runs shrink the standard deviation and make the median meaningful:

```bash
./build/perftest -n 20 kyoto
```

**CPU baseline versus GPU.** Run both and compare the medians:

```bash
./build/perftest -p cpu -n 3 kyoto
./build/perftest -p cuda -n 3 kyoto
```

If the GPU session can't be built, the library silently rebuilds on the CPU. `perftest` catches that and prints a
`Warnings:` section — check for it before concluding a GPU is slow.

**Check which models have fp16 weights.** Not every model publishes them:

```bash
./build/perftest --precision fp16
```

Models without fp16 weights show a `FAILED` row and are listed under `Failures:`; the rest of the sweep still
completes and the exit status is 1.

**Measure the end-to-end cost including the cache write.** This is what a real caller of `Process` pays:

```bash
./build/perftest --cache -n 5 kyoto
```

**Benchmark the face models.** Face recovery needs faces, so an untimed detection pass runs first and its cost is not
charged to the model under test. The embedded sample has two faces:

```bash
./build/perftest newyork athens santorini
```

**Benchmark a model you re-exported but haven't published.** The installer compares what is on disk against the
hashes in the remote manifest, so a locally rebuilt model is normally downloaded back over before it can be measured —
which silently benchmarks the model you were trying to replace. `--skip-verify` uses what is on disk as it is:

```bash
cp up_tokyo_4x_fp32.onnx ~/Library/Application\ Support/open-photo-ai/models/
./build/perftest --skip-verify tokyo
```

Drop the flag once the model is published; the normal verified path then picks it up on its own.

**Capture the results to a file.** Non-TTY output is detected automatically: the report goes to stdout, progress to
stderr, and the colour codes are stripped:

```bash
./build/perftest > results.txt
```

**See what the harness is actually doing.** Both forms enable the library's debug log and print every timed run in
order:

```bash
./build/perftest -v kyoto
OPAI_PERF_DEBUG=1 ./build/perftest kyoto
```

Useful assertion: the log should show `cache disabled` (or `cache miss`) once per run and **never** `cache hit`. A
cache hit in the timed loop means you are timing a PNG decode.

## Reading the output

```
perftest — open-photo-ai inference benchmark

  input       embedded sample, 640x640 (0.41 MPix, 2 faces)
  provider    Auto   probes: CUDA no | TensorRT no | CoreML yes
  precision   fp32
  runs        3 timed, 1 warm-up  (cold start measured after the warm-up, so it excludes model downloads)
  params      scale 4x (upscale models)   intensity 1 (denoise/sharpen/light/color models)
  cache       off — timings are inference, not PNG encoding
  models      2 selected

  ✓ stockholm   cold 977.0ms   median 371.3ms
  ✓ moscow      cold 6.108s    median 3.443s

┌───────────┬──────┬─────────┬─────────┬─────────┬─────────┬─────────┬────────┬────────┬─────────┬──────┐
│ MODEL     │ TYPE │ COLD    │ MIN     │ MEDIAN  │ MEAN    │ MAX     │ STDDEV │ MPIX/S │ OUTPUT  │ NOTE │
├───────────┼──────┼─────────┼─────────┼─────────┼─────────┼─────────┼────────┼────────┼─────────┼──────┤
│ stockholm │ dn   │ 977.0ms │ 369.2ms │ 371.3ms │ 372.7ms │ 377.5ms │ 4.3ms  │ 1.10   │ 640x640 │      │
│ moscow    │ sh   │ 6.108s  │ 3.441s  │ 3.443s  │ 3.475s  │ 3.541s  │ 57.4ms │ 0.12   │ 640x640 │      │
└───────────┴──────┴─────────┴─────────┴─────────┴─────────┴─────────┴────────┴────────┴─────────┴──────┘
```

On a terminal the model lines appear live, with a spinner and a progress bar fed by the library's per-tile progress
callback, and stay in the scrollback as each model finishes.

- **COLD** — session construction plus one inference. The gap between this and MEDIAN is what keeping models resident
  buys you.
- **MEDIAN** — the headline number. Prefer it to MEAN: it isn't moved by a single scheduling hiccup.
- **STDDEV** — sample standard deviation (Bessel-corrected, `n-1`). If it is a large fraction of the median, the
  machine was busy and the run is not worth quoting.
- **MPIX/S** — *input* megapixels over the median run. Input, because it is the one thing every model has in common: a
  4x upscaler's output is 16x its input, a denoiser's is 1x, and a detector has no image output at all. It is a
  cross-model comparison figure, not a throughput promise.
- **OUTPUT** — the produced image's dimensions, or `-` for models that return data.
- **NOTE** — the face count for the face models, or `FAILED` / `INTERRUPTED`.

`Warnings:` means the requested execution provider couldn't build a session and the library rebuilt the model on the
CPU. Those rows are CPU timings regardless of what the header says the provider was — the warning exists precisely
because a silent downgrade otherwise looks like a slow GPU.

Ctrl-C stops the in-flight run, prints the summary for the models that finished, and exits 1. The interrupted model is
marked `INTERRUPTED` rather than `FAILED`, so a cancelled run is never mistaken for a broken model.

## Flags

| Flag            | Alias | Default | Meaning                                                                                   |
|-----------------|-------|---------|-------------------------------------------------------------------------------------------|
| `--runs`        | `-n`  | 5       | Timed runs per model (min 1)                                                              |
| `--warmup`      | `-w`  | 1       | Untimed warm-up runs; also what downloads a missing model                                 |
| `--provider`    | `-p`  | `auto`  | `auto`, `cpu`, `cuda`, `tensorrt`, `directml`, `openvino` or `coreml`                     |
| `--precision`   |       | `fp32`  | `fp32` or `fp16`                                                                          |
| `--scale`       | `-s`  | 4       | Upscale factor, 1–8. Ignored by non-upscale models                                        |
| `--intensity`   | `-i`  | 1       | Blend intensity, -1–1, for the denoise/sharpen/light/colour models. Ignored by the others |
| `--cache`       |       | off     | Keep the library's image cache on, so timings include its PNG encode and disk write       |
| `--skip-verify` |       | off     | Use the model files already on disk without checking them against the remote manifest     |
| `--plain`       |       | auto    | Force line-by-line output instead of the live view                                        |
| `--verbose`     | `-v`  | off     | Library debug log plus every timed run in order; implies `--plain`                        |

## Caveats

- **No GC control and no CPU pinning.** Numbers are comparable within one invocation. Across machines, across
  processes, or against a thermally throttled laptop, they are not.
- **`--intensity` is a post-inference blend** (`internal/utils.BlendWithIntensity`). It changes the output and the
  cache key but barely the timing, so a flat curve across intensities is expected, not a bug.
- **`--scale` only applies to upscale models**, and `--intensity` only to denoise/sharpen/light/colour models. The
  other models ignore them.
- **DirectML and OpenVINO have no capability probe**, so the header reports only CUDA, TensorRT and CoreML. Their
  absence from the probe line says nothing about whether they work — the probes are informational and never block a
  run.
- **`--cache` churns the shared cache.** The benchmark uses the same config directory as the GUI, so a cached sweep
  evicts whatever the GUI had stored (500 entries / 1 GB). With the cache off, the default, nothing is written.

## Adding a model

One line in `catalog` in [models.go](models.go), using whichever adapter matches the model's `Op` signature:

| `Op` signature                              | Adapter          |
|---------------------------------------------|------------------|
| `func(float64, types.Precision) T`          | `scaleEntry`     |
| `func(float32, types.Precision) T`          | `intensityEntry` |
| `func(types.Precision) T` (returns data)    | `detectEntry`    |
| `func(types.Precision, []detection.Face) T` | `faceEntry`      |

The `list` output, the sweep order and the summary table all follow the catalog, so nothing else needs touching.
