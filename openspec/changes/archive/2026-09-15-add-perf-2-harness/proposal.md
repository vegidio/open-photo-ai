## Why

Nothing in this workspace has ever measured itself. The one number this project has about its own
inference is a paragraph in `inference/live.rs`'s module comment — *62.5 s on the CPU, 9.0 s on
CoreML* — taken by hand, from a `dev` build, by a test that exists to check that a seam does not
show. It is the only figure anyone can quote, it is explicitly useless as an absolute, and there is
no way to take another one without editing a test.

That is the gap this slice closes. `add-perf-1-opai` put the three facts a benchmark needs into the
library — release every resident session, find out what a run actually executed on, and use a model
file that was put on disk by hand — and wrote none of the harness that asked for them. This change
is that harness: `crates/perf`, producing the `perftest` binary, which turns "is Kyoto faster at
FP16 than FP32 on this machine" into one command.

It is also the first thing in this workspace to *use* the library. `gui` prints a window and `cli`
prints a version; neither has ever called `Opai::initialize` or `Opai::process`. Every signature
those two slices shaped for a front end has so far been checked only by `tests/public_api.rs`
naming it. This slice is the first consumer, and it is worth saying plainly that finding out the
surface is awkward to actually use is one of the things it is for.

## What Changes

- **A new crate, `crates/perf`, producing the `perftest` binary.** A third member of the workspace
  beside `gui` and `cli`, depending on `opai` exactly as they do — which is what the project context
  means by *"later consumers (a benchmark binary, for instance) are added as further crates depending
  on `opai`, never by widening a front end."*

- **The sweep, and what it measures.** Per model: an untimed warm-up (which is also what downloads
  the weights), then every resident session released, then one **timed cold start** — session
  construction plus one inference — then a number of **timed runs** in steady state. Minimum,
  median, mean, maximum, sample standard deviation, and input megapixels over the median. The
  measurement shape is the reference's and the reasoning behind each step is carried over with it.

- **Model selection comes from the library's own catalogue.** `opai::catalogue()` publishes every
  family, variant and parameter range, and `Operation::from_catalogue_with` turns a row back into a
  runnable operation. The reference needs a hand-written table of 21 models and four generic adapters
  to normalise their constructors; here there is no table to keep in step, and `perftest list` is a
  rendering of the library's answer rather than of the harness's own.

- **A model the library refuses is skipped, not reported as broken.** Today `chain::plan` refuses
  everything but Kyoto — the seven families whose pipelines are not written yet, the diffusion
  upscaler, and the two convolutional upscalers whose provider profiles are unmeasured. That is
  scaffolding on its way out rather than a property of those models, so the harness does not model
  it: it asks for no such thing from the library, keeps no list of its own, and marks nothing in
  `perftest list`. A sweep that was given no model names simply drops the ones the library declines,
  which costs nothing because the refusal precedes any transfer. A model named explicitly is
  attempted and its reason reported, which is how you find out why.

- **Two outputs: a plain report and a JSON document.** The plain one is the reference's — a header
  describing the conditions that produced the numbers, a summary table, failures, and the downgrade
  warnings. `--json` writes one machine-readable document to stdout instead, which the reference has
  no counterpart to and which is what makes two sweeps comparable by something other than a human
  reading two tables.

- **A run that was downgraded says so.** `Enhanced::providers` carries the requested provider beside
  what sessions were actually built on, and a row whose run did not get the provider the header
  claims is reported as a warning rather than being left to look like a slow GPU. That report is the
  whole of what slice 5 added it for.

- **Ctrl-C stops the run in flight**, prints the summary for the models that finished, and marks the
  interrupted model as interrupted rather than failed.

- **Not in this slice: the live view.** No spinner, no per-tile bar, no ETA, no TTY detection. That
  is slice 7 (`add-perf-3-tui`), and this slice deliberately registers no progress callback during a
  timed run — anything that writes to the terminal from inside the measurement is in the measurement.

## Capabilities

### New Capabilities

- `performance-benchmark`: what the benchmark measures and what it refuses to claim — the four
  stages and why the cold start is measured after the warm-up, what the image cache does to a
  timing, the statistics reported, what the header must state for a pasted table to be
  interpretable, how a refused, failed or interrupted model is reported without stopping the sweep,
  and what the exit status means.

### Modified Capabilities

None. Nothing in the library changes: this slice is a consumer of the surface `add-perf-1-opai` left
behind, and every behaviour it needs is already specified.

## Impact

**Crates:** a new `crates/perf`, and nothing else. `opai`, `gui` and `cli` are untouched.

The split is the project's own test — *if both `gui` and `cli` would need it, it belongs in `opai`*.
Nothing here passes it. What lands in `crates/perf` is a front end's work and only that: argument
parsing, which models to sweep, the loop that times them, the arithmetic over the durations, and two
renderings. None of it is something a GUI would want, and none of it is logic about enhancement.

**rust-sak:** nothing, and nothing here duplicates a module. Its eight are `crypto`, `fetch`, `fs`,
`image`, `image-raw`, `memo`, `o11y` and `sysinfo`; none covers argument parsing, terminal tables or
statistics, and none is close enough to be worth extending — a summary table is presentation for one
binary, not infrastructure another project would want from a first-party utility crate. The sample
photograph is loaded through `opai::image::load`, which is already `rust-sak`'s `image` module, and
the durations come from `std::time::Instant`.

**New crates:** `clap` 4 for the argument parsing, which the series' plan named at slice 5 and which
is the Rust standard; `comfy-table` for the summary table; `serde`/`serde_json` for `--json`;
`tokio` for the asynchronous entry point and for Ctrl-C. Each is weighed against its alternatives in
`design.md`.

**Public API:** unchanged. Nothing is added to `opai` and nothing existing changes shape, so
`tests/public_api.rs` is untouched and no call site moves.

**Shared state:** `perftest` runs against the same configuration directory as the GUI and the CLI,
under the identity `PERF`. That is what lets it reuse an installed runtime and installed weights
instead of downloading a second copy of both, and it is what makes `--skip-verify` usable the way it
is documented — drop a re-exported `.onnx` into the application's own models directory and measure
it. Two consequences are stated plainly in the header and in the crate's README rather than being
left to be discovered: it is **refused while the GUI is running**, by the single-instance claim slice
1 of this series took; and `--cache`, which is off by default, writes into the image cache the GUI
shares.

**Behavioural divergences from the Go implementation, and why each is worth it:**

1. **A no-argument sweep drops the models the library declines, rather than failing them.** The
   reference sweeps all 21 models because all 21 run there. Here `chain::plan` refuses everything but
   Kyoto, so the reference's shape would make the default invocation 20 rows of `FAILED`, a failure
   tally of 20 and an exit status of 1 on a workspace with nothing wrong with it. A refusal is not a
   measurement and is not a failure. This is temporary in both directions: as the model series lands,
   more of the catalogue is measured by the same sweep with nothing here edited, and when all of it
   runs the rule has nothing left to drop.

2. **A JSON output, which the reference has not got.** Two sweeps of a model across a change to it
   are the thing this binary exists to make possible, and comparing them through a rendered table
   means a human reading two tables. It is a new output, not a changed one: the plain report is
   unchanged by its existence.

3. **No memory-budget line in the header.** The reference reports `ModelMemoryStats`, and this
   project has not built per-pool memory budgets. Reporting a budget for a mechanism that does not
   exist would be worse than omitting the line, which is the conclusion slice 5 already recorded.

4. **No detection and no face-recovery rows.** The reference runs an untimed detection pass before
   the sweep and feeds its faces to Athens and Santorini. Neither family has a pipeline here, and
   `opai` publishes no way to run an operation whose result is data rather than an image, so there is
   nothing to time and nothing to feed it with. They are dropped from a sweep like every other family
   the library declines.

5. **No OpenVINO.** `ExecutionProvider` publishes five values and OpenVINO is not among them, so it
   is not a spelling `--provider` accepts.

6. **The downgrade report is read off the result, not off a registered handler**, and the cache is
   avoided by giving each run its own identity rather than by mutating the input's hash. Both follow
   from what slice 5 built, and both are already argued there.

**Estimated tasks:** 13, within the 15 the project allows, so this stays one change. The series is
listed in `design.md`; this is perf slice 2, change 6 of the seven, and slice 7
(`add-perf-3-tui`) is proposed when it is reached.
