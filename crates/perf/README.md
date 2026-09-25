# `perftest` — the inference benchmark

Measures what Open Photo AI's inference actually costs on the machine in front of you, and reports
it in a form that can be quoted.

```sh
cargo run --release -p perf -- kyoto
```

Build it with `--release`. A debug build measures the debug build.

## What it measures

One model at a time, against one image, in four stages — and only two of them are timed:

| Stage          | Timed | What it covers                                                                 |
|----------------|-------|--------------------------------------------------------------------------------|
| **Warm-up**    | no    | Puts the model on disk if it is not there already, and builds a first session. |
| **Cold start** | yes   | Building a session and running one inference.                                  |
| **Timed runs** | yes   | Steady state: the session is already built.                                    |
| **Statistics** | —     | Minimum, median, mean, maximum and sample standard deviation over the runs.    |

**The cold start is measured *after* the warm-up, and that ordering is the point.** The warm-up is
what downloads a model that is not on disk yet; measured before it, a "cold start" would include a
multi-hundred-megabyte transfer and would describe nothing. Between the warm-up and the cold start
every resident session is released, so the session the cold start builds is genuinely built rather
than found — a cold start that reused the warm-up's session would report the cost of a lookup and
present it as the cost of a build.

Sessions are also released **between models**, so one model's memory is not still held while the
next is measured and a downgrade is attributed to the model that actually hit it.

`--warmup 0` is allowed and the report says what it costs you: the cold start may then include
obtaining the model.

## Reading the output

```
  image      embedded sample, 640x640 (0.41 MPix)
  provider   Auto (this machine supports: CPU, CoreML)
  precision  fp32
  runs       3 timed, 1 warm-up   (cold start measured after the warm-up, so it excludes obtaining the model)
  params     scale 4, strength 1, bias 0
  cache      off — timings are inference, not encoding and storing the result   (store: Disk)
  log        ~/Library/Application Support/io.vinicius.opai/logs/opai.log
  models     1 selected
```

**The header is not decoration.** A table of durations pasted into an issue cannot be told apart
from one taken at a different precision, on a different provider, or with the cache on — and every
one of those differences is larger than the differences the table exists to show. Paste the header
with it.

In the table:

- **COLD** is session construction plus one inference. **MIN/MEDIAN/MEAN/MAX/STDDEV** are over the
  timed runs; the deviation is the sample deviation (`n-1`), and is zero for a single run.
- **MPIX/S** is **input** megapixels over the median run. Input, because it is the one thing every
  model has in common: an upscaler's output is many times its input and a denoiser's is the same
  size, so a figure over the output would not compare two models at all.
- **NOTE** says what the run actually executed on. `⚠ ran on …` means the row is a timing of what
  ran rather than of what was asked for, and `⚠ nothing executed` means no session was built at all.

The numbers are comparable **within one invocation**. Nothing here controls for CPU pinning or
thermal throttling, so two sweeps on two machines — or on one machine an hour apart — are not
directly comparable, and the footer says so.

`--verbose` additionally prints every timed run in run order, which is how a thermal ramp that the
median hides becomes visible.

## Watching a sweep

A sweep is a long job with nothing to show for itself until it is over — a single model on the CPU
is minutes, and a full sweep is hours — so while it runs it draws one line on **stderr**:

```
  ⠹ kyoto        (1/1)   run 3/5        ████████████░░░░░░░░  last  412.3ms  ~1.245s
  ⠹ kyoto        (1/1)   downloading    ██████░░░░░░░░░░░░░░  up_kyoto_4x_fp32 — 41 MB of 128 MB
```

Left to right: a spinner, the model's codename, its position among the models selected, the stage in
progress, how far that stage has got, and then whatever the stage has to say for itself.

- **The stage** is `warm-up 1/1`, `cold start`, `run 3/5`, or — while the model's files are being
  obtained — `downloading` or `extracting`. A model that is not on disk is downloaded **inside the
  warm-up**, which is several hundred megabytes for one model; that is what the second line above
  is, and no timing covers it.
- **The bar** is how far the run in flight has got, reported by the library per tile.
- **The tail** is the last timed run and an estimate of the time left during the timed runs, and the
  artifact being obtained with its byte counts during a transfer. A run that executed nothing
  because the result was already known says `from an earlier run` instead — which is the same
  finding the table's `⚠ nothing executed` reports, a minute earlier.

Durations are spelled exactly as the table spells them, so a figure read off the line while a sweep
runs and the same figure read out of the table afterwards are the same figure. A narrow terminal
loses the estimate first, then the last run, then the bar; nothing ever wraps.

**Each model goes into the terminal's own scrollback as it finishes**, above the line, carrying its
cold start and median or the reason it was declined, failed or interrupted. They are the terminal's
lines rather than the program's, so they survive the models measured after them, they survive a
Ctrl-C, and they are still on screen after `perftest` has exited. What is over the sweep as a whole —
the throughput figures, the downgrade warnings — is still the summary table's, printed at the end.

### The estimate covers one model, deliberately

`~1.245s` is what is left of **this model's** timed runs: the runs still to come plus what is left of
the one in flight, times the median of the runs already taken. It is not offered until the first
timed run has completed, and it excludes the cold start, which is several times a steady-state run by
construction.

It does not span models, and that is not an omission. What Kyoto at 4x costs says close to nothing
about what the next family will cost when the next family has a pipeline — the published figures
spread over more than an order of magnitude — so a sweep-wide figure would be presented with an
authority it has not got. The model counter answers *how far through the sweep am I*; this answers
*how long until this row appears*, which is the question with an answer.

### When the line is drawn, and when it is not

| stderr       | stdout       | `--json` | `--plain` | What you get                    |
|--------------|--------------|----------|-----------|---------------------------------|
| a terminal   | a terminal   | no       | no        | the live view                   |
| a terminal   | a terminal   | no       | yes       | one line per model              |
| a terminal   | redirected   | no       | either    | one line per model              |
| redirected   | either       | no       | either    | one line per model              |
| either       | either       | yes      | either    | nothing: stdout is the document |

The view **draws** on stderr, so `perftest kyoto 2> /dev/null` correctly gets no view and the report
still reaches stdout intact.

**Both streams have to be a terminal, and that is a limitation rather than a preference.** To put a
line below output that is already on screen, the view has to ask the terminal where the cursor is —
and the layer that asks writes that question to stdout, not to the stream the view draws on. With
stdout redirected the question lands in your report file instead of on screen, nothing answers it,
and the view gives up after a two-second wait. Testing stdout up front is what avoids all three: no
stray escape character in the captured report, no pause before the sweep starts, and no terminal mode
entered to ask a question nobody can hear.

So `perftest kyoto > results.txt` **is still watchable** — as one line per model on stderr — and the
file still holds the report and nothing else. What it does not get is the animated view.

`--verbose` does not force the plain form: the library's own records go to `opai.log`, so nothing
writes to stderr during a sweep except the renderer itself.

Reach for `--plain` where the automatic choice gets it wrong — a terminal whose handling of scrolling
regions the view is not welcome on. It is the documented other half of the choice rather than a
workaround, and the report is byte-for-byte the same either way.

### What watching costs the measurement

Nothing you can distinguish from the run-to-run variation the report already publishes — and getting
there turned up something worth knowing.

The progress callback is registered on **every** run, timed ones included, which is the only reason a
model's transfer is visible at all: an install reports itself through a *run's* callback. What that
callback does per tile is store a number and return. Nothing is drawn from inside a timed section;
the line is drawn by a timer of its own.

Measuring it on macOS arm64, the same model at a high run count once watched and once under
`--plain`, the two did not agree — and the watched arm was **faster**, by about 5% on the CPU. A
callback that stores a number cannot do that, so it was tested directly: with the callback deleted
from every timed run the gap was still there. What actually differed was that the watched sweep woke
its task fifteen times a second while the plain one sat idle for the length of each run, which is
what a scheduler reads as a process that does not need a fast core.

**So every renderer now keeps the same beat**, including the two that draw nothing. The two arms then
agreed to within 0.84% on the CPU and 2.11% on CoreML, with the medians inside each other's spread.
It also made the benchmark itself better: across that comparison both arms got faster (≈15.4s →
≈13.4s) and the run-to-run deviation fell by three to six times, because the process no longer drifts
between core types while it is being timed.

This is one machine and one platform, and the core-scheduling explanation is inference rather than
something measured directly. What does not depend on the explanation is the rule: every renderer runs
the sweep the same way, so what a sweep measures does not depend on who was watching it.

**Neither raw mode nor the alternate screen is used, and no key is read.** Ctrl-C reaches the process
as a signal and stops the sweep exactly as it does unwatched, and the only terminal state taken is
the hidden cursor, which is given back by a guard and by a panic hook — so a panic, a `kill -9` or a
crash inside ONNX Runtime cannot leave the shell without an echo.

## Flags

| Flag               | Default    | What it does                                                        |
|--------------------|------------|---------------------------------------------------------------------|
| `[MODEL...]`       | every one  | Which models to measure, by codename. See `perftest list`.          |
| `-n`, `--runs`     | `5`        | Timed runs per model. At least one.                                 |
| `-w`, `--warmup`   | `1`        | Untimed warm-up runs. Zero is allowed; the report warns.            |
| `-p`, `--provider` | `auto`     | `auto`, `cpu`, `coreml`, `cuda` or `tensorrt`.                      |
| `--precision`      | `fp32`     | `fp32`, `fp16` or `int8`. Not every variant publishes every one.    |
| `-s`, `--scale`    | `4`        | For the families whose published parameter is the scale.            |
| `--strength`       | `1`        | For the families whose published parameter is the strength.         |
| `--bias`           | `0`        | For the families whose published parameter is the bias.             |
| `--image <PATH>`   | the sample | Measure a photograph of your own instead.                           |
| `--cache`          | off        | See below.                                                          |
| `--skip-verify`    | off        | See below.                                                          |
| `--plain`          | auto       | Report progress as one line per model instead of the live view.     |
| `--json`           | off        | Write one machine-readable document to stdout instead of the table. |
| `-v`, `--verbose`  | off        | Print every timed run in run order.                                 |

The three parameter flags are named after the parameter the catalogue publishes, and each is bounded
by the range that parameter's own constructor enforces — so a value this accepts is a value the
library accepts. Which flag moves which family is what `perftest list` prints.

```sh
perftest list                          # every model the catalogue publishes
perftest kyoto --scale 2 --runs 10     # one model, ten timed runs, at 2x
perftest --precision fp16              # every model the catalogue publishes at fp16
perftest --json > results.json         # the same sweep, for a program to read
perftest kyoto > results.txt           # the report to a file, progress still on stderr
perftest kyoto --plain                 # one line per model instead of the live view
```

## Three things that will otherwise surprise you

**1. A sweep is refused while the GUI is running.** `perftest` runs against the *shared*
configuration directory — the same one the GUI and the CLI use — so that it reuses an installed
runtime and installed weights instead of pulling several gigabytes into a directory of its own. That
directory takes one process per user, so a sweep is refused while another OPAI process holds it, and
the message names which one and its process id. `perftest list` and `perftest --help` work
regardless: they read nothing in that directory.

**2. `--cache` writes into the run cache the GUI shares.** It is off by default, and off is the
right default for a measurement: with the cache on, the library encodes each result and writes it
*inside the call being timed*, which for a 4x upscale of the sample is a 2560x2560 PNG per run. Turn
it on to measure what a run costs an ordinary caller of the library — and know that the results land
in the store the GUI reads, evicting whatever it had. Every timed run still performs its own
inference either way: each one is given an identity of its own, so no run of a measurement can be
served from the result of an earlier one.

**3. A sweep given no model names skips the models the application does not yet run.** A model the
library has no pipeline for is refused with a reason, and a refusal is not a measurement and is not a
failure — so an unnamed sweep drops those models, says how many it dropped, and exits reporting
success. **Name one to see why it was refused** — a model named explicitly is attempted whatever the
library makes of it, and its reason is reported.

Which models those are is **not written down here, and deliberately not**. Nothing in this crate
keeps a list of what runs: it finds out by attempting, so as pipelines land the same sweep measures
more of the catalogue with nothing in this crate edited. A list here would be a second place to
maintain, and it has already been wrong twice — first when face recovery gained a pipeline, and again
when light adjustment did.

## `--skip-verify`

Uses the model files already on disk without checking them against the published hashes, so a model
you re-exported and have not published can be measured as it is. Drop the `.onnx` into the
application's own models directory and name the model.

It reaches nothing that is downloaded: a model whose files are not all present is still installed
and verified under the ordinary rules. A sweep run with it on says so in a line on stderr, and the
library writes its own warning per acquisition into the log — a surprising number is worth
suspecting this first.

## Where the output goes

The report is the **whole** of stdout, rendered or JSON. The live view, install progress, the
per-model lines, warnings about the configuration and every diagnostic go to stderr, so that

```sh
perftest --json > results.json
```

produces a parseable file even on a machine that downloads a runtime on the way.

The JSON document carries a schema version, the tool's version, the conditions the sweep ran under
and, per model, its outcome — durations in milliseconds as numbers where it completed, and the
reason where it was declined, failed or interrupted.

## The log

The library's own records — per session, per acquisition, per install — go to `opai.log` in the
configuration directory, whose path the header prints. `RUST_LOG` sets that file's filter:

```sh
RUST_LOG=debug perftest kyoto
```

## Interrupting

Ctrl-C stops the run in flight, prints the summary for the models that finished, marks the model
that was interrupted as **interrupted** rather than failed, and exits reporting failure. An
interruption is not a fault in a model.

It works the same way whether or not a live view is drawn: the view reads no keys and takes no
terminal mode, so the signal reaches the process exactly as it would have. The models that had
already finished are in the terminal's scrollback and stay there.

## Exit status

`0` where every measured model completed — including a sweep in which the only thing that happened
was the application declining models it has no pipeline for. `1` where a model failed to be
measured, or the sweep was interrupted.

## The sample image

A 640x640 JPEG embedded in the binary, carried over from the Go application's `cmd/perf` so that a
measurement taken here can be compared with one taken there. It is 0.41 megapixels, which is a
twentieth of the photographs this application exists for — `--image` measures one of your own
instead, and the header reports whichever was used with its dimensions.
