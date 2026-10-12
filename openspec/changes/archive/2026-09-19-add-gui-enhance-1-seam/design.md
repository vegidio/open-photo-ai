## Slices

The series that takes the application from a sidebar whose Add enhancement button is greyed out to
one that runs an upscale and draws the result. **This is a new series**; the `add-gui-images-1-ipc`
.. `add-gui-images-4-zoom` series closed on the zoom slice and is not continued here.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-enhance-1-seam` | The structured operation on the wire; the single-run slot; the run and stop commands; the progress event and its throttle; the enhanced result served over the existing `opai://` scheme; `ipc/enhance.ts`. No screen | - | **CURRENT** |
| 2 | `add-gui-enhance-2-sidebar` | The per-file enhancement store; screen 10 (add menu, upscale only); the sidebar rows with their info line and hover-remove (screens 05 and 14); screen 11 (the model tray and the scale control); the run-and-cancel effect; screen 06 (the progress chip); the enhanced pane drawing the result | 1 | PENDING |

**Slice 2 is the tight one.** It estimates at fifteen tasks exactly - store, menu, row, popover
frame, model tray, scale control, the Add button made live, the run effect, the chip, the enhanced
pane, i18n, tests, lint - which is the hard limit with nothing spare. If it overflows when its
`tasks.md` is written, the cut is to split the canvas half off as a third slice: the run effect, the
chip and the enhanced pane are one coherent piece and leave the sidebar whole behind them. That is a
fallback, not the plan; it is written down here so the next person does not have to rediscover it.

**Crates.** `gui` alone. `opai` is not edited: `Opai::process`, `ProcessOptions`,
`InferenceProgress`, `Scale` and `Enhanced` are consumed exactly as they stand, which is the test
this slice passes rather than the exception it takes. `cli` is untouched. Within `gui` the split is
the one every previous slice used - the Rust side owns the conversion from the wire shape into
`opai::Operation`, because that is the only place the two vocabularies meet; the frontend side owns
the spelling of the command names and nothing else. See proposal.md - Impact.

**rust-sak: nothing used directly, and nothing missing.** No download, hash, extraction, temp file,
codec, cache or hardware probe is written here. The run reaches `fetch` and `crypto` when a model
must be installed and `memo` when the run cache is read, all through `opai`; the enhanced result is
encoded through the rendition path `add-gui-images-1-ipc` built, which reaches rust-sak's `image`
through `opai`. There is no gap to fill in either direction.

**New crates: none.** No new npm package either - D6.

**Platforms.** Written for all three, validated on macOS. One thing here is platform-shaped and is
named in Risks: whether a webview caches a response for an identity that later stops being served.

**Execution providers.** The provider is whatever the window passes through; nothing here selects,
builds or falls back. Validated on CoreML and CPU, which is what the development machine offers.
CUDA and TensorRT are untested by this slice and untouched by it - a run's provider reaches `opai`
unchanged and the fallback behaviour is `add-inference-2-providers`'.

## Context

Everything the run itself needs already exists and is reachable from `gui` today. See proposal.md -
Why. What shapes this design is a short list of facts about the two sides of the boundary:

```
  opai                                        gui                          webview
  ----                                        ---                          -------
  process(&Picture, &[Operation], opts)  <--  enhance command         <--  invoke(...)
    ProcessOptions.cancel: Token         <--  cancel command          <--  invoke(...)
    ProcessOptions.on_progress           -->  event emit              -->  listen(...)
  Enhanced { picture, provider_report }  -->  ??? held where ???
                                              opai:// serve           <--  <img src>
```

- **`opai` is ready and is not the constraint.** Cancellation is already cooperative and checked
  between tiles, already reports a distinct `InferenceError::Cancelled`, and is already a field on
  `ProcessOptions` rather than something a caller arranges.
- **Tauri offers no cancellable call.** Wails' `CancellablePromise` cancelled the Go `context` for
  free, and the reference's whole cancellation story rests on it. There is nothing equivalent, so the
  stop has to be its own command, which makes ordering the interface's problem as well as ours.
- **`Picture` holds `Arc<DynamicImage>`.** Sharing a result between the slot that owns it and the
  handler that serves it costs a refcount, not a copy. This is what makes the resident-result
  decision affordable.
- **The `opai://` scheme already resolves an identity and caches encoded renditions.** It resolves
  through `Opened`, which maps identity to a path. An enhanced result has no path, which is the one
  place the existing machinery genuinely does not fit.
- **`images.rs` already argues at length against base64 over IPC**, and names the 150 lines of
  `utils/image.ts` the reference spends on the consequences. That argument decides D4 before this
  design starts; it is not reopened here.

## Goals / Non-Goals

**Goals:**

- Settle the three things a later screen cannot take back: how an operation is spelled, how a run is
  stopped, and where the result lives.
- Leave `opai` untouched, and prove the seam needed nothing added to it.
- Make "one run at a time" a property the backend holds rather than one the interface promises.

**Non-Goals:**

- **No screen, and no store.** Both are slice 2. The Add enhancement button stays disabled, which is
  what `Sidebar.tsx`'s own note asks for.
- **No export.** The export queue runs its own chains at its own depth over many files at once, and
  the single-run slot here is explicitly not it - D2.
- **No autopilot.** The toggle stays a standing preference that nothing reads.
- **No provider fallback notice.** `Enhanced` carries a `ProviderReport` and the reference raises a
  snackbar from it. Nothing reads it this slice; it is answered and ignored rather than dropped, so
  the slice that draws it has it already.
- **No face recovery.** `opai` can run it, but it needs a detection run and a face set the window has
  no wiring for. It is a family this seam will carry unchanged when that wiring lands - D1.

## Decisions

### D1. An operation crosses the boundary as a tagged variant, not an encoded string

The reference composes `up_kyoto_2x_fp32` in TypeScript and parses it back in Go, and its own comment
records the cost: the string is *"a contract in three places at once"* - the Go parser, the frontend
image cache, and the library's on-disk cache key - so *"changing a template silently invalidates every
cached result and breaks parsing on the Go side"*.

None of those three forces apply here. The run cache key is derived inside `opai` from the
`Operation` itself; the frontend needs no cache of its own because the pixels travel over a URI
scheme; and `catalogue` already publishes the codename, the precisions and the parameters the string
encodes. So the window names them:

```
  { family: "upscale", codename: "kyoto", precision: "fp32", scale: 2.0 }
      |
      +-- one arm, in Rust --> Upscale::kyoto(FloatPrecision::Fp32, Scale::clamped(2.0))
```

**Tagged by family, with one variant today.** The other six families take different values - an
intensity, a face set, nothing at all - so a flat record with an optional `scale` would be a shape
that is wrong for six of seven. A tagged union means denoise is a variant and an arm, not a
reshaping of what upscale already sends.

**Alternatives.** Carrying the reference's string would have meant writing a grammar in TypeScript
and a parser in Rust for vocabulary the catalogue already publishes - the exact duplication
`catalogue()` was built to remove. Sending an opaque index into the catalogue would have coupled the
wire to the order a presentation decision happens to publish.

### D2. One run, one result, and the slot that holds both

The application holds a single slot: the run in flight and the most recent completed result. Asking
for a run writes the slot and cancels what it displaces.

```
   enhance(runId, source, ops)
        |
        v
   +--------------------------------------------------+
   |  RUN SLOT                                         |
   |    id:     RunId (the window's)                   |
   |    cancel: CancellationToken   --> cancel() the   |
   |    result: Option<Picture>         displaced one  |
   +--------------------------------------------------+
        ^                                   |
        |                                   v
   cancel_enhance(runId)              opai:// serve
   (no-op unless id matches)          (Arc clone, no copy)
```

**Why the backend supersedes rather than trusting the window.** The slot has to exist anyway - D4
needs somewhere for the result to live, and there is one of it. Given that, cancelling on write is a
few lines, and it turns an invariant the interface could break into one it cannot. The reference
relies wholly on its frontend calling `p.cancel()` in an effect cleanup; an unmount or a fault there
leaves a run holding sessions and occupying a GPU with nobody waiting for it.

**Why an explicit stop as well.** Superseding alone leaves a run going in exactly the cases the
feature was asked for: removing the only enhancement, and moving to a photograph with an empty stack.
Neither starts a successor, so neither displaces anything.

**A displaced run answers `stopped`, whatever it returned.** Cancellation is cooperative and checked
between tiles, so a run that has been displaced can still return successfully without ever having
noticed - a chain served entirely from the run cache returns almost at once and checks nothing at
all. Its pixels were dropped the moment the successor took the slot, so reporting it as an
enhancement would hand the window an address the scheme answers `GONE` for. The slot therefore
answers whether it kept what it was given, and a run it did not keep reports as stopped. Slice 2's
run effect can branch on `outcome` alone and never has to ask whether its run is still the current
one.

**This slot is not the export queue.** Export runs a chain per file over a selection, at a different
depth, and reports per file. It will own its own structure, and the child-token arrangement
`ProcessOptions::cancel` documents - *"a child token under one parent is how a front end stops every
image of an export queue from one gesture"* - is what it will use. Nothing here forecloses that; this
slot is the preview's, and naming it so is what stops a later slice from widening it into something
that serves neither.

### D3. The window generates the run identifier

`invoke` calls race. A cleanup that stops the old run and a body that starts the new one cross the
boundary independently, and either may land first:

```
   cleanup: cancel_enhance()  --\
                                 >--- order not guaranteed
   body:    enhance(...)      --/

   if cancel lands second and names "whatever is running",
   it kills the run that was just started.
```

So the stop names a run, and it is a no-op unless the slot still holds that one. The identifier is
minted by the window rather than returned by `enhance`, because a cancellation can arrive before
there is anything to cancel - the case the reference's `utils/image.ts` handles with a `cancelled`
flag and a comment about it. Minting it up front means the window always has the name of the thing it
wants stopped, and the flag is unnecessary rather than reimplemented.

**Alternative.** A monotonic counter held in Rust and returned from `enhance` would work for the
ordinary path and fail for exactly that one, since the window would have nothing to name until the
promise resolved.

### D4. The enhanced result is served over `opai://`, from one resident `Picture`

The result is held in the slot as an `opai::Picture` and resolved by the scheme handler alongside
`Opened`:

```
   opai://<identity>?size=N
        |
        +--> Opened:   identity -> path      --> decode file, bound, encode
        +--> run slot: identity -> Picture   --> bound, encode      (no decode, no disk)
                 |
                 v
          Renditions LRU (unchanged, shared by both)
```

**Full resolution, and therefore expensive.** The zoom this application already has draws the
enhanced pane at the photograph's own size, so a bounded result would be unusable. A fourfold
enlargement of a 24 MP photograph is roughly 384 MP; at eight bits per channel (D7) that is about
1.5 GB resident in the worst case. Holding **one** is what bounds it: every previous result is
dropped the moment a new run is asked for, which is the same act that cancels the old run.

**Alternatives.** Writing the result to a rust-sak temp file and admitting it to `Opened` would have
reused the existing path wholesale and freed the pixels, at the cost of a full-resolution encode and
a write of hundreds of megabytes on every preview run - paid on every slider drag. Holding a
full-resolution JPEG instead of pixels would cut the memory to tens of megabytes and re-decode on
every zoom-driven fetch, and would lose fidelity before the user ever reached export. Base64 over IPC
is the reference's answer and is ruled out by `images.rs`'s own header.

**What this does to the entitlement.** The set the window may read widens from admitted files to
admitted files plus results produced from them - see the `gui-images` delta. It widens what may be
*drawn*, not what may be *reached*: a result is pixels this application computed, never a file it
went and read.

### D5. What a report carries, and where it is thinned

`InferenceProgress` already carries everything needed: `subject` names the operation, `stage` is
`Installing(Progress) | Running | Cached`, `chain_fraction` drives a bar and `operation_fraction`
scopes it. The event carries the run's identifier too, so a report from a run the window has
abandoned is discardable rather than confusing.

**`Cached` advances and never labels.** It is a stage the reference has no equivalent for. An
operation served from the run cache did neither of the two things a label can name, and reporting it
as `Running` at completion would jump the bar its whole width with no explanation - which is why
`opai` made it a separate variant. With one operation in the chain today, a fully cached run
therefore reports completion and nothing else, and the interface draws no indicator at all.

**Thinned at the emitter, not the listener.** A tiled run reports per tile - thousands of times for a
large upscale - and an indicator has about a hundred positions. The reference throttles in React, and
its component carries a comment about the two numbers that must both be compared or the download
percentage freezes between bar movements. Doing it in Rust means the messages are never serialised or
put on the channel at all, and the rule lives in one place instead of in every listener. The test is
the same one the reference arrived at: emit when the rounded whole-chain percentage changes **or**
the rounded install percentage does, plus the first and last report of each stage.

### D6. The store slice 2 unblocks does not need `immer`

`stores/enhancements.ts` parks this explicitly today: *"No immer. The Wails store needs it for the
Maps; one boolean does not, and pulling the dependency in now would be adding it for code that has
not been written."* Slice 2 writes those Maps, so the question is live and is answered here because it
is a series-level decision rather than a screen's.

The answer is no. The store has three writers - append to a file's list, filter one out, replace one -
all one level deep over a `Map<path, Operation[]>`:

```
  set(state => ({ enhancements: new Map(state.enhancements).set(path, next) }))
```

`immer` earns its place on deeply nested state, and this is not that. The reference needs it because
its store grew three per-file Maps with more shape inside them; that is a reason that applies to its
store, not to a Map of flat arrays. If a later slice's face selection or crop state turns out to be
genuinely nested, that slice should pay for the dependency and say so. Slice 2 rewrites the parked
comment to record that the Maps landed and still did not need it, so it is not re-litigated a third
time.

### D7. Depth is eight bits, and the scale is clamped rather than refused

**Depth.** `ProcessOptions::depth` defaults to `Eight` and its own documentation says a front end's
default should be `Source`. That is right for export and wrong here: this result exists to be drawn
in a webview, which cannot draw more than eight bits per channel, and asking for sixteen would double
the resident cost of D4 to produce pixels nothing can show. The export slice will ask for `Source`,
which is the caller-by-caller decision `ProcessOptions` is shaped for.

**Scale.** `Scale` offers both `new` (refusing out of range) and `clamped`. The seam uses `clamped`.
The control the user drives is bounded to the same range, so a value outside it can only arrive
through a fault, and the honest question is what a user should see when one does: a refused
enhancement, or a picture. `Scale::clamped`'s own documentation states the case it exists for - *"a
caller that cannot act on a refusal"* - and a slider drag is exactly that. The fault stays visible in
the log.

### D8. Sessions, memory and lifetimes

Nothing here owns a session. A chain acquires its own and holds them for its whole duration - `opai`
pins every model a chain touches until the chain returns, so a concurrent admission cannot evict
operation one's model while operation two is running - and lets go when `process` returns. The slot
holds a `CancellationToken` and an `Option<Picture>`, neither of which is a session handle.

That is what makes superseding safe rather than something to arrange. Cancelling the displaced run
does not free its models; it makes the run return, and the run returning drops the last handle. A
stop therefore cannot race a session's destruction, because the stop does not touch sessions at all.
This is the compile-time guarantee `openspec/config.yaml` asks the Rust design to reach for where the
Go original had a locking discipline, and this slice inherits it rather than adding to it.

The one lifetime this slice does own is the result's. It is `Arc<DynamicImage>` inside `Picture`, so
the scheme handler serving a rendition holds it alive for the duration of that response even if a new
run displaces the slot mid-flight. A request that arrives after the displacement finds nothing and is
refused; a request already in progress completes against the pixels it started on.

## Risks / Trade-offs

**A fourfold upscale of a very large photograph is over a gigabyte resident** → Bounded to one at a
time by D2, and dropped at the moment the next run is asked for rather than when it completes. The
alternatives in D4 each trade it for a worse cost. Worth revisiting if a machine with less memory
than the models already demand turns out to exist; a model that needs 8 GB of GPU memory sets the
floor well above this.

**A webview may cache a response for an identity that later stops being served** → The rendition
response is served immutable, which `ipc/images.ts` records as true and acted on only by WebView2. An
enhanced identity covers the source and every operation, so the pixels behind it can never change -
a cached response is therefore correct, not stale, and the refusal after displacement only affects a
window that did not cache it. Untested on Windows; named here because that is the one platform where
the cache exists.

**A stop that arrives while a model is downloading may not feel immediate** → Cancellation is checked
between tiles and at the install boundary, not inside a multi-gigabyte transfer. A user who adds
Osaka and immediately removes it may wait. Not addressed here: it is `opai`'s cancellation
granularity, it affects the CLI equally, and inventing a second mechanism in `gui` would be the
front-end-trapped logic the architecture rules forbid. Recorded so the slice that hits it knows where
it lives.

**The window can leak a run if it never stops one and never starts another** → It cannot leak more
than one, by D2. The run finishes, writes the slot, and holds no sessions afterwards; the cost is one
completed run's pixels held until the next request, which is the same cost the feature has anyway.

**Slice 2 is at the fifteen-task limit** → Named in Slices above, with the cut to make if it
overflows. The risk is that the limit is met by compressing several steps into one task, which the
rules forbid for exactly this reason.

**The emitter-side throttle could hide a stall** → Emitting only on change means a run that stops
advancing stops reporting, and an indicator would sit still rather than say so. That is the same
behaviour the reference has, and the honest one: the bar not moving is what a stall looks like. The
first and last report of each stage are always emitted so a stage transition is never the thing that
goes missing.
