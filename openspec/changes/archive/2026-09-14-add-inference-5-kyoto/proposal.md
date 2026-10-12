## Why

Four slices in, `opai` can name a model, verify its weights onto disk, decide what it should run on,
open it as a resident session, and partition an image into tiles and stitch them back. Every piece of
an enhancement exists and **nothing joins them**: `inference/` is `pub(crate)` and carries an
`expect(dead_code)` reading *"the first caller is the slice that runs a pipeline"*, `Sessions::session`
has no caller outside its own tests, and `Operation::profile` and `Upscale::resolve` are both dead
code waiting for whoever runs a pass.

There is also no way in. The crate's public surface can load an image, name an operation and save an
image, and has no function between the second and the third. A front end built against `opai` today
can show a user every model this project publishes and run none of them.

This slice is the join: Kyoto upscales a real photograph end to end, through a public entry point,
reporting progress, stoppable partway, and at a richer bit depth for a caller that asks for one.

## What Changes

Task estimate: **15**. The inference feature estimates far past 15 and is split into six slices (see
design.md - Slices); this proposal covers slice 5 only.

**In `opai`, the first public inference surface and the pipeline behind it:**

- **`Opai::process`** — an image and an ordered chain of operations in, an enhanced image out. The
  chain shape is the entry point from the start even though one operation is served today, because
  slice 6 keys its cache on *the operations already applied* and the seven remaining families slot in
  behind it without the signature moving.
- **`ProcessOptions`**, passed as an `Option` — the execution provider, the output depth, the progress
  callback and the cancellation token, each with a default. The same idiom `image::save` already uses
  for `Option<EncodeOptions>`: only the picture and the operations are genuinely required, and a
  caller with no opinion passes `None`. It is also what keeps slice 6 additive — a cache mode is one
  more defaulted field rather than a changed signature at every call site.
- **`OutputDepth`** — 8 bits per channel, 16, or whatever the source carried, **defaulting to 8**. The
  reference has no depth concept at all and its decode is fixed at 8-bit, so a caller that says nothing
  gets exactly the Go behaviour and anything richer is a deliberate request. The third choice is
  resolved against the loaded image here, because this is where the image is known; slice 4 built both
  targets and left this choice named in a table. It is now written down as a type.
- **The convolutional upscale pipeline.** Kyoto's requested scale resolves to a sequence of native
  passes (2x, 4x, or 4x then 2x for an 8x request); each pass acquires its session, runs slice 4's
  driver over it, and hands its result to the next. Where the passes overshoot the request — a 1.5x
  request served by the 2x weights — the overshoot is corrected by a resample **after** inference,
  which is the pipeline's job and deliberately not the driver's.
- **Progress across the whole chain.** A new `InferenceProgress` naming the operation, whether its
  model is being installed or run, how far that operation has got, and how far the whole chain has.
  Three levels of weighting sit behind one number that only ever goes up: the chain's operations, the
  install-then-run split inside one operation, and the passes inside an upscale — weighted by input
  pixel area, so an 8x run does not spend half its bar on the pass that does a fifth of the work.
- **Cancellation.** A `CancellationToken` the caller holds and passes in the options, checked between
  tiles. A cancelled run produces no image at all and says it was cancelled.
- **Per-tile execution against a real session** — the first code in this project that feeds a tensor
  to ONNX Runtime. It binds slice 4's scratch buffers to the session rather than allocating per tile,
  which is what slice 4's out-parameter was shaped for.
- **`InferenceError`**, the public failure of a run, into which slice 3's `SessionError` and slice 4's
  `TilingError` both fold.

**What is refused, and each refusal is deliberate:** Osaka, which is diffusion and needs a different
pipeline entirely; Tokyo and Saitama, which share Kyoto's contract and would run unchanged but whose
execution-provider profiles nobody has measured; and the seven non-upscale families, whose pipelines
are the series' declared deferral. Every one is a named error rather than a silent wrong answer.

## Capabilities

### New Capabilities

- `image-enhancement`: running a chosen enhancement over an image end to end — what a run accepts and
  what it refuses, the bit depth the result carries, how a scale that no single set of weights serves
  is covered by a sequence of passes and corrected afterwards, what a run reports while it works, and
  what happens when a caller stops one.

  Distinct from the four capabilities around it, and none is widened. `models` names what can be run
  and validates its parameters. `model-install` puts files on disk. `model-sessions` opens a model and
  says nothing about what goes through it. `inference-tiling` covers one model run over one image at
  one scale — the geometry, the tensor and the seam — and is the layer this one stands on. This
  capability is the run a *user* asks for: a chain, a requested scale that no graph natively serves,
  a depth, a progress bar and a cancel button.

### Modified Capabilities

None. Nothing here changes an existing requirement: the tiling driver's contract is unchanged and
this slice supplies the per-tile function it was built to take; sessions are opened and cached
exactly as slice 3 specified; no install behaviour, provider resolution or image IO behaviour moves.

## Impact

**Prerequisite change:** `refactor-image-picture-type` lands first. It renames `SourceImage` to
`Picture` and corrects that type's identity documentation, which this slice is written against —
`Opai::process` takes a `Picture` and returns one. The rename is worth doing before rather than after
because this is the change that first makes the type a return value and the first to produce an
identity that is not the hash of any file; both are things the old name and the old wording got
wrong. It carries no behaviour and no spec delta.

**Crates touched:** `opai` only. `gui` and `cli` are untouched — deliberately, and it is worth saying
why rather than leaving it to be noticed: both front ends have to run an enhancement, so the pipeline,
the pass sequencing, the progress weighting and the depth resolution are the definition of shared
logic. What the front ends will add later is a Tauri command, an event and a React control, none of
which can exist before there is something public to call. This slice is what makes that possible;
wiring it is the GUI slice's work.

**`rust-sak` - what was found before anything was proposed.**

- **Nothing covers this slice's subject.** A pass sequence over ONNX graphs, an inference progress
  split and a model-run pipeline are this application's own contract, not general-purpose
  infrastructure — the same judgement slice 4 recorded for the tile grid.
- `image` is the near miss again and again is not the answer: a codec module, decode in and encode
  out, with no resampling. The one resample this slice needs (correcting a pass sequence's overshoot)
  is `image::imageops`, which the `image` crate this project already depends on directly brings with
  it.
- `memo` **is** the right module for the per-operation image cache, and is exactly why that cache is
  slice 6 rather than this one — it is a whole module's worth of adoption, not a corner of this slice.
- `fetch`, `crypto`, `fs` and `sysinfo` are reached only through slices 1-3, unchanged.

**New third-party crates:** one, `tokio-util 0.7.19`, for `CancellationToken` — and it costs nothing
to compile. It is **already in this crate's dependency graph**, pulled in by `reqwest` through `h2`,
so naming it directly adds a manifest line and no code. `default-features = false`: `CancellationToken`
lives in `tokio_util::sync`, which is unconditional, so no feature is enabled on its account.

The alternative already in the graph was weighed and rejected: `enough`, a synchronous
cooperative-cancellation trait, arrives through `rust-sak`'s `image-raw` feature. It is not
`rust-sak`'s vocabulary — `rust-sak` never accepts one, it passes `Unstoppable` to satisfy `zenraw`'s
signature — and it has no async half, so nothing in `opai` could ever await it. A front end cancelling
an export queue wants exactly what it cannot give: one parent token whose children stop every image in
flight, and a future a Tauri command can select on.

**Parity divergences from the Go implementation, deliberate:**

- **The result's bit depth is the user's choice, though not by default.** The reference decodes every
  model output to 8-bit RGBA, so a 16-bit RAW enhanced by the Go app comes back at 8 bits per channel
  whatever it is then exported to. `OutputDepth` is the fix and it is the visible half of what slice 4
  built — but it **defaults to 8**, so the divergence is opt-in and a caller that says nothing gets
  the reference's behaviour exactly. Design D3 records what that costs and why a front end's default
  should be the other one.
- **Alpha is dropped as a channel rather than carried as a column of 255**, inherited unchanged from
  slice 4. A transparent source composites against black in both implementations; only the channel
  count of the result differs.
- **No per-operation image cache yet.** The reference's `Process` reads the cache before each
  operation and writes back after it, which is what makes dragging a slider cheap. Slice 6 is that,
  and it is the last slice of this series. Until then every run is a full run.
- **A chained call's identity is composed now, not in the cache slice.** The reference had a real bug
  here — it returned the *input's* hash on the output, so a second `Process` over the result looked
  its operations up under the original image's identity and got the first call's picture. The fix is
  a one-line composition, so this slice does it rather than leaving slice 6 to retrofit correctness
  onto a signature already in use.
- **Nothing is logged**, unchanged from slices 3 and 4 and for the same reason: this project has no
  logging. The reference logs the per-pass timings, the chain duration and every model run around this
  code. Each fact is carried as data instead — progress through the callback, failure through the
  error, the provider a run got through the handle.
- **No divergence guard**, inherited from slice 4 and landing with the denoise pipeline that can
  demonstrate it.
- **Only Kyoto runs.** The reference runs all twenty operations. This is the series' own slicing
  rather than a permanent gap: three of the refusals (Tokyo, Saitama, and Osaka) are a table entry and
  a diffusion pipeline away, and the seven families are named as later changes.

**Not in scope:** the per-operation image cache (slice 6); Osaka's diffusion pipeline, the seven
non-upscale family pipelines, and the eleven unmeasured execution-provider profiles, each deferred
series-wide with a reason in design.md; alpha preservation; the model memory budget; the divergence
guard; logging; and any `gui` or `cli` change.
