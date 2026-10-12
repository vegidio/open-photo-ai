## Slices

Running an image through a chain of models estimates far past 15 tasks. It is divided into six
changes, each leaving the project buildable and testable on its own. **This change is slice 5.**

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-inference-1-models` | A named model's files on disk, verified against the published manifest | the initialization series | **done** |
| 2 | `add-inference-2-providers` | The `ExecutionProvider` type including `Auto`, per-provider default options, and the per-model profile that overrides them | 1 | **done** |
| 3 | `add-inference-3-sessions` | Session construction, the resident session cache with its idle life, and CPU fallback | 2 | **done** |
| 4 | `add-inference-4-tiling` | The shared inference primitives — the tile grid, reflection padding, CHW tensor conversion at either output depth, seam blending — and the tiled driver over them, generic in the per-tile run | none (parallel with 1-3) | **done** |
| 5 | `add-inference-5-kyoto` | The upscale pipeline and the public chain entry point, with progress, cancellation and `OutputDepth` — Kyoto runs end to end | 3, 4 | **current** |
| 6 | `add-inference-6-cache` | The 24-hour per-step image cache, keyed on the input identity plus the operations applied | 5 | pending |

Slice 4 depended on nothing in 1-3. Everything else is a straight chain, and this slice is the first
that needs all of it at once.

**One prerequisite outside the series: `refactor-image-picture-type` lands first.** It renames
`SourceImage` to `Picture` and corrects that type's identity documentation. It is not part of this
series and delivers no inference behaviour, but this slice is written against `Picture` and is the
reason the rename is worth doing now: this is the change that first makes the type a *return* value
and the change that first produces an identity no file on disk hashes to. Both are what the old name
and the old wording got wrong. If the two are applied in the other order, this slice carries the
rename itself and that change becomes empty.

**Slice 5's row is the one the previous four wrote, unchanged in scope.** Slice 4 widened itself by
taking the tiled driver — the loop over the primitives — and narrowed this row by the same amount, so
what is left here is exactly what the row says: the pipeline, the public surface, progress,
cancellation and `OutputDepth`. One thing the row does not say is settled below and is worth naming
up front: **only Kyoto runs**. Tokyo and Saitama share Kyoto's contract and would run unchanged, and
are still refused — see D10.

Deferred past the series, each to its own change when something needs it: the model memory budget
with LRU eviction; alpha preservation through inference; the eleven measured execution-provider
profiles other than Kyoto's; the remaining seven families' pipelines; Osaka's diffusion pipeline;
logging; evicting sessions built on a provider the user has just switched away from; the per-tile
divergence guard; and — added by this slice — the front-end wiring (the Tauri command, the event and
the React control), which cannot be written before there is a public function to call.

## Context

The four slices before this one each end at something with no caller.

Slice 3 ends at `Sessions::session(artifact, profile, requested, on_progress) -> SessionHandle`,
async, installing the model inside a single-flighted build and downgrading to the CPU where a provider
cannot open it. Slice 4 ends at `run_tiled(source, geometry, scale, depth, norm, progress, cancelled,
run_tile) -> DynamicImage`, synchronous, holding no session and naming no `ort` type. `Upscale::resolve`
returns a `Resolution::Passes(Vec<Pass>)` — the native scale factors covering a requested one, with the
artifact serving each — and `Operation::profile` returns the tuning a session is built with. All four
carry `expect(dead_code)`, and three of those attributes name this slice in their reason.

So this slice writes almost no new mechanism. It writes the joins, the public vocabulary, and the one
thing genuinely missing: the code that hands a tensor to ONNX Runtime.

Four facts about the crates involved shape the design and were checked against their source rather
than assumed:

- **`ort::session::Session::run` takes `&mut self` and returns `SessionOutputs<'s>` borrowing the
  session** (`session/mod.rs:236`). Combined with slice 3's `Mutex<Session>`, this decides where the
  lock is taken and for how long — see D4.
- **`TensorRef::from_array_view` takes `(shape, &[T])`** and `TensorRefMut::from_array_view_mut` takes
  `(shape, &mut [T])` (`value/impl_tensor/create.rs:214,253`), each without a copy. The driver's input
  scratch therefore becomes a tensor by being borrowed.
- **`Value::try_extract_tensor::<f32>()` returns `(&Shape, &[f32])`** (`value/impl_tensor/extract.rs:155`)
  — a borrow of the runtime's own output buffer, valid until the `SessionOutputs` is dropped. Getting
  it into the driver's output scratch is one `copy_from_slice`; D4 is why that is the starting point.
- **`tokio_util::sync` is not behind a feature** — `pub mod sync` is unconditional in tokio-util's
  `lib.rs` and its `default` feature list is empty — and `tokio-util 0.7.19` is **already in this
  crate's graph** via `reqwest` → `h2`. `CancellationToken` therefore costs a manifest line and
  nothing else.

The reference implementation's own shape for this layer is `Process` → `runInference` →
`upscale.RunPipeline` → `RunTiledInference`, and three details in it are behavioural rather than
structural and are carried: the download taking the head of an operation's progress range, the passes
weighted by input pixel area, and the single resample after the last pass. Two more are carried as
**fixes** rather than as parity: its output hash (D2) and its fixed 8-bit decode (D3).

See proposal.md - Why for motivation, and specs/image-enhancement/spec.md for the behaviour.

## Goals / Non-Goals

**Goals:**

- One public function is the whole of how an enhancement is run, so a Tauri command and a CLI
  subcommand are each a thin call rather than two assemblies of the same five steps.
- The pipeline's arithmetic — the pass sequence, the three progress weightings, the depth resolution,
  the identity composition, every refusal — is exercised on every CI platform with no runtime, no GPU,
  no model file and no network, as slices 3 and 4 both managed for their own layers.
- A run that is cancelled or that fails produces **no image**, at every level: the driver already
  refuses to hand back a half-written buffer, and the chain refuses to hand back the operations that
  had already succeeded.
- Everything slice 6 needs in order to be additive exists here: the identity it keys on, and a chain
  entry point whose signature it does not have to change.

**Non-Goals:**

- A second family. Refusing seven of eight is the series' plan, not an oversight.
- Any front-end code. There is nothing to wire until this lands.
- Resampling anywhere except once, after the last pass. Between passes it would feed a model an
  interpolation instead of a photograph.
- Measuring or budgeting memory. An 8x 16-bit run's footprint is named as a risk and nothing governs
  it, unchanged from slice 4.

## Decisions

### D1. The pipeline grows inside `inference/`, beside the driver

`inference/chain.rs` for the chain — validation, the per-operation loop, the progress weighting across
operations. `inference/upscale.rs` for the convolutional pass pipeline. `inference/session.rs` for the
per-tile function that runs an `ort` session, which is the **only** file in the slice that names an
`ort` type. `inference/progress.rs` for the reported type and the splitter. Tests beside each, as the
project's convention requires.

This is what slice 4's D1 said this directory was for — *"the family pipelines are inference, and they
will sit beside the driver rather than inside the session cache"* — and the seam it draws is the one
worth protecting. `inference/session.rs` is where a runtime becomes necessary; everything else in the
module stays runnable without one, which is what keeps the whole of the arithmetic above inside CI's
reach.

`inference/upscale.rs` is a file rather than a directory, deliberately against the convention
`models/` follows. That convention exists because a model family's *vocabulary* — its variants, its
parameters, its resolution rules — is several files' worth. A family's *pipeline* is one function and
its tests. The day a family needs more than that, it becomes a directory then; anticipating it now
would be eight directories holding one file each.

`InferenceError` goes in `error.rs` beside `InitError` and `SessionError`, which is where slice 3's
D11 put the layer below it.

### D2. `process` takes a `Picture` and returns one, with the identity composed here

```
Opai::process(
    &self,
    source:     &Picture,
    operations: &[Operation],
    options:    Option<ProcessOptions>,
) -> Result<Picture, InferenceError>

pub struct ProcessOptions {
    pub provider:    ExecutionProvider,    // default Auto
    pub depth:       OutputDepth,          // default Eight — see D3
    pub on_progress: Option<OnInference>,  // default None
    pub cancel:      CancellationToken,    // default a fresh token nobody holds
}
```

**Two required arguments and one optional bundle**, which is the idiom this crate already uses:
`image::save(pixels, destination, format, options: Option<EncodeOptions>)`, where `None` means the
defaults. Only the picture and the operations have no sensible default; the other four do, and a
caller that wants "just enhance this" writes `None` rather than four values it has no opinion about.

This also settles what the first sketch of this signature got wrong, which was to take six positional
parameters — two of them `Option` and one a bare `&CancellationToken`. At that width a call site is
unreadable without naming every argument in a comment, and `ExecutionProvider` beside `OutputDepth`
beside two more is exactly the shape where an argument gets passed in the wrong position and still
compiles.

The bundle is what keeps slice 6 additive, which is the same argument the identity composition below
makes: a cache mode is one more field with a default, not a seventh parameter and a changed signature
at every call site. `..Default::default()` is the documented way to set one field, so a field added
later is source-compatible for every caller using it.

`Picture` is `refactor-image-picture-type`'s rename of `SourceImage`, and **that change is this
slice's prerequisite** — see the Slices section. It matters here rather than being incidental: this
is the change that makes the type a *return*, which is what exposed that its name said input-only,
and it is the change that produces the first identity no file on disk hashes to.

`Picture` in, because it is what `image::load` produces and the pipeline's door should be the
loader's exit. `Picture` out — rather than the bare `DynamicImage` the driver returns — because of
what the reference got wrong here and wrote down:

> *The returned pixels are not the input's, so they must not carry the input's hash. Handing back the
> input hash meant a second Process call over the result looked its operations up under the original
> image's identity and got the first call's output.*

That fix is a one-line composition — the input identity plus every operation's `cache_tag`, hashed —
and doing it here rather than in slice 6 is what makes slice 6 **additive**. A cache slice that had to
change this signature to become correct would be retrofitting correctness onto a function front ends
were already calling.

The composition hashes with `rust-sak`'s `crypto` (XXH3-64), which is already an enabled feature and
is already what a loaded image's identity is. So an identity stays one fixed-width value however long
the chain, and a chained pair of calls agrees with a single call over the concatenated operations —
which is the property the reference's comment is about, and it gets its own test. Not `rust-sak`'s
`memo::KeyFrom`, which the reference uses: `memo` is not an enabled feature and enabling a whole
module for one string join, in the slice *before* the one that adopts it properly, is backwards.

**This produces the first identity that is not the hash of any file**, and the contract has to say so.
`image-io`'s requirement is already scoped correctly — it opens *"A loaded image SHALL carry an
XXH3-64 identity computed over the whole of the file's bytes"* and closes with *"The identity SHALL
describe the bytes that were read, and SHALL NOT be silently carried onto different pixels"* — and a
composed identity **honours** that closing clause rather than straining it: composing is precisely
what stops the input's identity being carried onto computed pixels. So no `image-io` requirement
moves, and this slice adds its own in `image-enhancement` instead.

What did overstate the case was the type's Rust doc comment, which presented file-byte equality as a
property of the type rather than as the loader's guarantee. `refactor-image-picture-type`'s D2
corrects that wording, which is the other half of why that change comes first: without it, the first
`Picture` this slice returns would contradict its own documentation.

An empty chain returns the source unchanged, identity included. Applying nothing does not derive
anything, so there is nothing to name — the same rule the reference states, and it is a property of
the derivation rather than something every caller has to remember.

The return is owned rather than written into a caller's buffer because every enhancement changes the
dimensions or the depth or both; there is no buffer a caller could have prepared.

### D3. `OutputDepth` is three choices, `Eight` is the default, and the third is resolved against the image

```
pub enum OutputDepth { Eight, Sixteen, Source }

impl Default for OutputDepth {
    fn default() -> Self { Self::Eight }   // parity: what the reference always produced
}
```

**`Eight` is the default, so more than 8 bits is always a deliberate request.** The reference has no
depth concept at all — there is no such setting anywhere in the Go or TypeScript source, and its
decode is fixed at 8-bit RGBA — so 8 is what every enhancement this application has ever produced.
A caller that passes `None` therefore gets exactly the Go behaviour, and the two depths slice 4 built
are reached by asking for them.

The alternative was `Source` as the default, which this design originally recommended, and it is worth
recording why it lost rather than leaving the reversal unexplained. It is the *better picture* — it is
the whole reason slice 4's D6 built two decode paths — but as a default it makes the most surprising
outcome the automatic one: a caller that never thought about depth gets a 16-bit result from a RAW,
twice the memory, through encoders this project has barely exercised (see Risks), because of a
property of the input file it did not look at. A default should be the outcome that surprises nobody,
and an opt-in is what makes the better picture a choice someone made.

**What that costs is real and is not hidden: a front end that never thinks about depth silently
narrows a 16-bit RAW**, which is the defect D6 exists to prevent. The answer is that these are two
different defaults at two different layers, and only one of them is this slice's. The library's is
conservative because a library's should be. **A front end's should be `Source`**, for this project's
own reason: `image::load` develops a RAW to 16 bits and `image::save` keeps 16 through PNG, TIFF, AVIF
and HEIF, so a GUI defaulting to `Eight` would throw away depth its own loader and its own exporters
both preserve. That is the GUI slice's decision to make explicitly, and it is written down here so
that slice inherits the reasoning rather than the accident.

`Source` resolves against the loaded `DynamicImage`: the four 16-bit variants and the two float
variants resolve to `ChannelDepth::Sixteen`, the four 8-bit variants to `Eight`. **This is the only
place it can be resolved**, which is why slice 4 built both targets and left the choice to this one:
the driver takes a region and a shape and never sees the file the pixels came from.

**The match cannot be made exhaustive, and what stands in for one is written down rather than
implied.** `DynamicImage` is `#[non_exhaustive]`, so naming every variant this build knows still leaves
a catch-all arm and an `image` bump cannot be turned into a compile error here — which is what the
resolution would want, since the failure it prevents is a format carrying more than eight bits being
narrowed by a default nobody chose. Three things together stand in: the catch-all carries
`debug_assert!(false)`, so a bump fails loudly the first time the suite runs; in a release build it
answers from the variant's own declared bits-per-channel rather than from `Eight`, so the worst case is
a correct answer nobody reviewed instead of a silent narrowing; and the test enumerates every variant a
second time by hand, so the two lists disagreeing is what fails. Worth the three because the alternative
is the one outcome D6 built two decode paths to prevent.

It is resolved **once**, before the chain runs, and every operation is given the resolved depth. A
chain that resolved per operation would work — each operation's input is the previous one's output, and
the previous one's output is already at the resolved depth — but only by accident, and it would put the
depth of a result at the mercy of how many enhancements a user happened to select. Resolving once makes
that impossible rather than merely unlikely.

The divergence slice 4's D6 built for is therefore now user-facing but not automatic: a 16-bit RAW
enhanced by the Go app is 8-bit from the first model run onwards whatever it is exported to, and here
it is 8-bit too **unless someone asks otherwise**. What changed is that asking is now possible, which
it was not before slice 4.

### D4. The tile runs against a borrowed input tensor, and the lock is taken per tile

`inference/session.rs` supplies the `&mut RunTile<'_, ort::Error>` the driver takes:

```
|input: &[f32], output: &mut [f32]| {
    let tensor = TensorRef::from_array_view(([1usize, 3, shape, shape], input))?;
    let mut session = handle.session();                 // the MutexGuard
    let outputs = session.run(ort::inputs![tensor])?;
    let (_, produced) = outputs[0].try_extract_tensor::<f32>()?;
    output.copy_from_slice(produced);
    Ok(())
}
```

Three things in that are decisions.

**The input is borrowed, not copied.** `TensorRef::from_array_view` takes `(shape, &[T])`, so the
driver's input scratch — allocated once per run by slice 4's D7 — becomes the tensor ONNX Runtime reads
with no allocation at all. This is precisely what slice 4's Context checked `ort`'s source for, and it
is the half of D7 that pays off here.

**The output is copied once, and I/O binding is not used.** `try_extract_tensor` borrows the runtime's
own buffer, which is freed when `SessionOutputs` drops, so the data has to leave it. Slice 4's D2
already weighed this: the copy is ~12 MB per tile at a 4x pass against a per-tile inference cost of a
couple of hundred milliseconds — under a percent — and `TensorRefMut::from_array_view_mut` is there if
measurement ever says otherwise. Starting with `run` keeps the first code in this project that touches
a tensor as small as it can be; the binding path is a local change to this one closure.

**The `Mutex` is locked per tile rather than held across the pass.** Slice 3's D4 put the session
behind a `Mutex` because `run` takes `&mut self`, and argued that serializing two runs of one model
against one GPU costs nothing. That argument holds for two *tiles*; it does not hold for two *images*.
Holding the guard for a whole pass means an export queue running two photographs through Kyoto makes
the second wait out the entirety of the first — minutes — where locking per tile interleaves them at
tile granularity. The cost is one uncontended lock per tile, tens of nanoseconds against hundreds of
milliseconds. Recorded here because slice 3 made its call before there was a loop to look at.

The graph's tensor names are not spelled: `Session::run` with a positional `SessionInputs` uses the
session's own declared input names, so a conventionally-exported graph — one tensor in, one out, which
is every fixed-shape model this project ships — needs no name at the call site. The reference has to
name `"input"` and `"output"` because its binding is by name; this does not, and a graph that declared
something else would still run.

`ort::Error` is the `E` the driver is instantiated at, so a per-tile failure arrives as
`TilingError::Tile { index, source: ort::Error }` with the tile number intact.

### D5. The passes run back to back, and exactly one resample happens, at the end

`Upscale::resolve` already returns the sequence: `[2]` for a 2x request, `[4]` for 4x, `[4, 2]` for 8x,
and for a request between two native factors the sequence that first reaches or exceeds it. The
pipeline runs them in order, each over the previous one's result, then resamples once to the requested
scale if the product of the factors overshot.

**Nothing is resampled between passes**, and this is the decision rather than an implementation
detail. A model's input being the previous model's output is what a multi-pass upscale *is*; inserting
a Lanczos resize in between would hand the second pass an interpolation to restore rather than a
photograph, and the result would be visibly softer for no reason a user could see. The reference does
the same and its structure says so — `RunPipeline` loops the passes and calls `resizeToIntendedScale`
after the loop.

The correction is `FilterType::Lanczos3`, matching the reference's `imaging.Lanczos`, and it runs only
when the dimensions actually differ — a 4x request served by the 4x weights is not resampled through an
identity transform. Final dimensions are the input's multiplied by the requested scale and rounded,
which is the reference's arithmetic exactly.

**Applied as `DynamicImage::resize_exact`, not as `imageops::resize` over the image directly**, and the
difference is the depth this series exists to preserve. `imageops::resize` reaches its pixels through
`GenericImageView`, whose `DynamicImage` implementation is typed `Rgba<u8>` — so correcting a 1.5x
request would be exactly where a 16-bit result the caller asked for silently became an 8-bit one, on the
last line of the pipeline, for the one scale class that takes this branch. It is the same defect the
tiling slice wrote `Sampler` to avoid on the way in, arriving on the way out. `resize_exact` dispatches
per variant and keeps what the run produced, and a 16-bit request through a resampled scale is pinned as
a test.

An empty pass sequence is a **refusal**. `Scale`'s validation and the bucket tables make it
unreachable today — slice 1's tests already assert every bucket table reaches 8x with no gap — but the
failure it prevents is one the reference names in a comment: zero passes means the loop runs zero
times and `resizeToIntendedScale` returns a plain Lanczos resize of the input, *presented as a
successful AI upscale, with no error and no log line*. It is the same defect class as slice 4's
zero-tile refusal and it gets the same answer.

### D6. Three weightings, one number, and it only goes up

```
pub struct InferenceProgress {
    pub operation: Operation,
    pub stage: Stage,                  // Installing(Progress) | Running
    pub operation_fraction: f64,       // this operation, 0..=1
    pub chain_fraction: f64,           // the whole run, 0..=1
}
pub enum Stage { Installing(Progress), Running }
pub type OnInference = Arc<dyn Fn(&InferenceProgress) + Send + Sync>;
```

A new type rather than widening the public `Progress`, which is install-shaped: `dependency`, `phase:
Downloading|Extracting`, `bytes`, `total`. A tile loop moves no bytes and extracts nothing, so
widening would mean two fields that are meaningless on most reports and a `dependency` that has to
become optional. `Stage::Installing` **nests** the existing `Progress` rather than flattening it, so a
front end showing "Downloading Kyoto 4x — 41 MB of 128 MB" gets the byte counts unchanged and the
install layer keeps one representation.

`Arc<dyn Fn + Send + Sync>` mirrors `OnProgress` for the reason that one is: it crosses a
`spawn_blocking` boundary (D8).

Three weightings compose into `chain_fraction`, and each exists because of a specific way a bar
misbehaves without it:

- **Across the chain.** Operation *i* of *n* owns `[i/n, (i+1)/n)`. Without it the bar refills from
  zero once per operation. Equal shares, not weighted: what an operation costs is not knowable before
  it runs, and the reference divides equally for the same reason.
- **Inside one operation.** Where a model had to be installed, the install owns the head of that
  operation's range and the run owns the rest; where it was already on disk, the run owns all of it.
  Carried from the reference including its `0.2` split, and including the detail that makes it work:
  the split is decided by whether an install *actually happened*, not by whether one was possible, so
  a model already on disk does not give up a fifth of its range to a phase that never ran.
- **Inside one upscale.** Passes are weighted by **input pixel area**, so an 8x run's 4x pass — which
  covers a sixteenth of the pixels its 2x successor does — takes a proportionate share instead of half
  the bar. Weighted by the *input* rather than the output because the input's area is what decides the
  tile count, and the tile count is what the driver reports against. The reference's arithmetic,
  carried unchanged.

The driver already snaps its own last report to exactly 1 (slice 4's D8); the chain snaps
`chain_fraction` to exactly 1 on its last operation, for the same reason. A caller that passes no
callback is charged nothing at any level — every weighting is inside an `Option` that is `None`.

### D7. Cancellation is a `CancellationToken`, and `enough` was the alternative in the graph

Slice 4's D8 deliberately left this here: *"whether cancellation is a `CancellationToken`, an
`AtomicBool` or something else is a question about how a front end cancels an export queue, and
answering it here would be guessing on behalf of code that does not exist."* The code still does not
exist, but the question it depends on has an answer — an export queue cancels *many* runs from one
gesture, and a Tauri command wants to stop waiting rather than to poll.

`tokio_util::sync::CancellationToken` gives both: a child token per image under one parent for the
queue, and `cancelled()` as a future a command can select on. It costs nothing to compile (see
Context). Inside the run it is used in its polling form — `token.is_cancelled()` as the driver's
`&dyn Fn() -> bool` — because the tile loop is synchronous; the awaitable half is for the caller, which
is exactly the half an `AtomicBool` could not have provided.

**`enough` was the alternative already in the graph and it was rejected on the facts.** It arrives
through `rust-sak`'s `image-raw` feature, so the project's "check `rust-sak` first" rule points at it.
Reading it settles it: it is not `rust-sak`'s vocabulary — `rust-sak` never accepts one, it passes
`enough::Unstoppable` to satisfy `zenraw`'s signature — and it is a `no_std` synchronous trait with no
async half at all. Nothing in `opai` could await it, and a front end cancelling a queue would get
neither the hierarchy nor the future. Adopting it would be reading the rule's letter against its
purpose.

The token is borrowed, not owned, so a caller keeps the one it can cancel. A run stopped this way
returns `InferenceError::Cancelled` and no image, which the driver already guarantees for the tile it
was in the middle of.

### D8. Per operation: acquire on the runtime, run on a blocking thread

Each operation acquires its session `await`-ing `Sessions::session` — which is where the install and
its progress happen — and then hands the synchronous work to `task::spawn_blocking`, the same helper
the archive expansion, the runtime load and every image decode already go through.

This is not optional and it is not about tidiness. In a Tauri process the async runtime *is* the
window's event loop: a minute of tiled inference on it does not merely slow the application down, it
stops the window from drawing — including the progress bar this slice exists to feed. The spec
requires the reports to reach the caller while the run is still going, and this is what makes that
true.

Acquire-then-run per operation, rather than acquiring every model up front, matches the reference and
is what makes D6's second weighting meaningful: "Downloading Kyoto" appears when Kyoto's turn comes,
not as an opening act for the whole chain.

What has to cross the boundary constrains the types, so it is worth stating: the `Picture`'s
pixels are already an `Arc<DynamicImage>` and are cloned in; `SessionHandle` is `Send` because
`ort::Session` is `Send + Sync` (slice 3's Context) and the handle is an `Arc` over it; `OnInference`
and `CancellationToken` are both `Send + Sync + 'static` by construction. Nothing here needs a new
bound on an existing type.

`spawn_blocking` work cannot be aborted, which is why cancellation is cooperative and checked between
tiles rather than by dropping the future. A dropped future leaves the run going until its next tile
boundary; the token is what actually stops it.

### D9. Errors: one public enum the three layers fold into

`pub enum InferenceError` in `error.rs`, with `thiserror` as the project's convention requires:

- `Unsupported { operation: String, reason: &'static str }` — no pipeline for it. Named, so a front end
  can say *which* enhancement it cannot run, and carrying the reason because the four refusals below are
  not one thing: D10's whole point is that Tokyo's gate is a table entry away from lifting and a denoise
  pipeline is a change away, where Osaka needs a different contract. Without the reason a front end can
  only say "not supported" to all of them, which is wrong about three.
- `Install(Arc<InitError>)` — the model could not be put on disk, from `SessionError::Install`.
- `Open { artifact, provider, source }` — on disk and would not open, from `SessionError::Build`. Slice
  3's D11 drew that distinction deliberately and it survives to the public surface intact, because
  the two are acted on differently: one is retried, the other is a broken install.
- `Run { operation, tile, source }` — the model failed on a tile, from `TilingError::Tile`, carrying
  the tile index for the reason slice 4 carried it.
- `Untileable { width, height }` and `NoPasses { operation, scale }` — the two refusals of a request
  that cannot produce a picture.
- `Cancelled` — the caller asked. **Not** the next one.
- `Shutdown` — the async runtime shut down before the blocking work could run, from `task::Cancelled`.

The last two are separate for the reason slice 4's D9 made the same split one layer down, and this is
where it becomes visible to a user: one is somebody closing a dialog and the other is a process on its
way down. A front end that conflated them would tell a user their enhancement failed when they had
just cancelled it themselves.

Not variants on `InitError`. That type is public and documented as what *initialization* reports;
slice 3 already declined to widen it for session failures and named this slice as the one that would
need a public inference error folding both. This is it.

`SessionError` stays `pub(crate)` and is converted at the boundary, so the crate's internal layering
is not published. Its sources are already behind `Arc` (slice 3's D11), so this enum is `Clone` for
free — which slice 6 will want, since a cache single-flighting a run hands one failure to every
waiter, exactly as the session cache does.

### D10. Four refusals, and only one of them is technical

- **The seven non-upscale families.** The series' declared plan. Each lands with its own change.
- **Osaka.** `Upscale::resolve` returns `Resolution::Graphs` for it, which is a different contract
  entirely — a fixed set of named graphs, a latent space, its own accumulator (slice 4's D10). The
  `Resolution` enum exists precisely so that a caller has to say what it does with each branch rather
  than defaulting one to the other's shape, and this slice says: refused.
- **Tokyo and Saitama.** These two *would run unchanged*. They are convolutional, they resolve to
  `Resolution::Passes`, and the pipeline would serve them without a line of new code. They are refused
  anyway, at the user's decision, because their execution-provider profiles are unmeasured —
  `UpscaleVariant::profile` returns `EpProfile::default()` for every arm but Kyoto's FP16 one, and the
  series defers the eleven measured profiles to the changes that can measure them. Shipping them now
  would mean a user running Tokyo on settings nobody checked and concluding the model is slow.

  **This is a gate, not a limitation, and the distinction is the point of writing it down.** Lifting it
  is one table entry per variant once someone has measured it. The refusal names the variant so the
  message can say so.
- **A scale no bucket covers.** D5. Unreachable today and guarded anyway.

Every refusal is checked over the whole chain **before** anything is installed, which the spec
requires: a chain whose second operation cannot be served must not first transfer gigabytes on behalf
of its first. It is a cheap pass over the operations and it runs first.

### D11. Ownership and lifetime: the chain holds every handle to the end

Each operation's `SessionHandle` is pushed into a `Vec` that lives for the whole `process` call and is
dropped when it returns. This is the reference's `leaseSet` and it is kept for the reason its comment
gives: *"releasing per operation would let a concurrent admission evict operation 1's model while
operation 2 is still running, and the rebuild would be charged to this call."*

In Rust that costs nothing to enforce and nothing to get wrong. A `SessionHandle` is an `Arc` over the
resident entry (slice 3's D4), so holding one *is* the guarantee: the sweep's "in use" test is
`Arc::strong_count > 1`, eviction is a `map.remove` that drops only the cache's reference, and the
session is freed by `Drop` when the last handle goes. A model in use cannot be reclaimed by anything —
not by the sweep, not by an explicit clear, not by a provider switch — because reclaiming it is not a
decision any code makes. That is the spec's last scenario, and there is nothing to test about it that
the compiler has not already decided.

Two consequences worth stating rather than discovering. A chain that names one model twice takes two
handles on one entry, which is correct — the count goes up twice and down twice. And a two-hour run
is never swept out from under itself: `last_used` is stamped when a handle is handed out *and* in its
`Drop` (slice 3's D4), and the strong count is above one throughout, so the reference's per-tile
`Touch` and its leaked-lease janitor have nothing to do here. The reference needs them because its
leases are released by hand and one can be forgotten; here there is nothing to forget.

The peak footprint of a run is the source, the current pass's input, its output, and the driver's
scratch — with the previous pass's result dropped as soon as the next pass's begins, since each pass
consumes the last one's image. Nothing budgets it; see Risks.

### D12. Platform and provider validation

Everything in D2, D3, D5, D6, D9 and D10 — the identity composition, the depth resolution, the pass
sequence and its single resample, all three progress weightings, every error mapping and every refusal
— is arithmetic and runs in full on the ubuntu, macOS and Windows CI jobs against a fake per-tile
function, exactly as slice 4's suite does. The refusals and the weightings are where the bugs would
be, and they are all covered there.

What CI cannot cover is `inference/session.rs` and everything downstream of it, for the reason slice
3's D13 established and this slice does not change: the live path needs the pinned ONNX Runtime — ~175
MB — and this repository's suite is hermetic. So the end-to-end test is `#[ignore]`d and run by hand,
as slice 3's four are.

**This slice is the first that can answer the question slice 3 left open**, and that is worth naming
because it is the most valuable test in the series so far. Slice 3 recorded that CoreML's option map
had reached a live runtime and that *"TensorRT's and CUDA's remain unverified"* — nothing had ever run
inference on any of them. A Kyoto run over a real photograph is what verifies, for the first time:
that slice 2's option maps produce a session that actually computes; that the seam blend leaves no
visible edge on a photograph rather than on synthetic gradients (slice 4's D11 called this out as the
one property its proxy assertion could not check, and said *"the seam is the first thing to look at
there"*); that an `Rgb16` result survives the AVIF and HEIF encoders, which slice 4 flagged as a path
`rust-sak` supports and this project had never exercised; and what an 8x 16-bit run actually costs in
memory.

The test is written to be run on hardware and left `#[ignore]`d with that stated as the reason, never
stubbed into passing — the project's conventions forbid fabricating a model to make a test go green.
It will have been run on macOS arm64 (CPU and CoreML) by the time this slice is complete; the NVIDIA
providers stay unverified until somebody with the hardware runs it, and the task says so rather than
implying coverage that does not exist.

## Risks / Trade-offs

**The seam that CI cannot see** → Slice 4 asserted "no visible seam" as "no adjacent pair of columns
differs by more than a small step" over synthetic tiles, and said plainly that it is a proxy and that
this slice is where the real answer comes from. If a real Kyoto pass over a real photograph shows an
edge down the last column, the bug is in slice 4's blend and this slice is what found it. That is the
right order — but it does mean a defect in an archived slice may surface here, and the fix would be a
change against `inference-tiling` rather than something this slice can absorb quietly.

**An 8x 16-bit run of a 24-megapixel photograph holds well over a gigabyte** → Unchanged from slice
4's risk of the same name, and now reachable through a public API that offers it. Nothing budgets it
and the memory budget is deferred past the series. What this slice adds is the first opportunity to
*measure* it, which the live test does; if the number is bad, the honest response is that the public
surface offers 8x and a small machine cannot serve it, and the budget change moves up the queue.

**Refusing Tokyo and Saitama ships less than the code can do** → Deliberate (D10), and the cost is
real: two models a user can see in the catalogue and cannot run, for a reason — an unmeasured EP
profile — that is invisible to them. Mitigated by the error naming the variant, so the message can say
"not yet tuned" rather than "not supported". Lifting it is a table entry each.

**The chain's equal per-operation shares are wrong whenever operations cost differently** → A chain of
a denoise and an 8x upscale gives each half the bar, and the second half takes twenty times as long as
the first. The reference has the same behaviour and the same reason: what an operation will cost is
not knowable before it runs. Nothing here makes it worse, and the per-pass weighting inside an upscale
fixes the case that actually occurs today, since a chain of one operation is what this slice can
produce.

**`InferenceError` publishes a shape three internal layers fold into** → Adding a family later may want
a variant, and `#[non_exhaustive]` is what keeps that from being a breaking change. Applied for that
reason, consistently with `Dependency` and `Phase`.

**One public function does five things** → `process` validates, installs, opens, runs and composes an
identity, and a caller that wants only one of those has no smaller door. That is the intended trade —
slice 3's D10 made the same call for `session` and gave the same reason: *"a model that is named is a
model that can be run" is true at one call site rather than at every caller*. Splitting it would put
the ordering back in every front end.

## Migration Plan

No migration. Nothing on disk is read or written that did not exist before, no format changes, and
every public item in this slice is new — no existing signature moves, so nothing that compiles today
stops compiling.

Four `expect(dead_code)` attributes come off as their subjects gain their first caller:
`mod inference`, `Sessions::session`'s test-only reachability, `Operation::profile` and
`Upscale::profile`. That is the visible sign the slice landed.

Reverting leaves slices 1-4 exactly as they are today: four layers with no caller, each carrying the
attribute that says so.

## Open Questions

- **Whether the per-tile lock (D4) should instead be held across a pass.** Answerable only by
  measuring a real two-image queue, which needs the front end this slice does not build. It changes no
  specified behaviour and is local to one closure, so the day there is an export queue to watch, it is
  a one-line change either way.
- **Whether an upscale's correcting resample should use Lanczos3 or something cheaper.** Carried from
  the reference unexamined, because matching it is the parity bar and the resample runs once per run
  over the final image. If it turns out to cost meaningfully at 8x, a filter choice is a parameter,
  not a redesign.
