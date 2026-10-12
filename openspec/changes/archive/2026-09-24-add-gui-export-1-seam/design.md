## Context

See proposal.md for why. What this slice builds on, as it is today:

- **`enhance/run.rs`, `enhance_with`**, does the job in this order:
  1. judge the whole chain
  2. resolve the source
  3. write the slot
  4. decode and frame (`load_framed`)
  5. run `Opai::process`

  The slot (`Runs`) holds **one** run and one result, and a new run displaces both. Its comment on `OutputDepth`
  already says: "Export asks for `Source` instead."
- **`enhance/progress.rs`, `Reporting::new(run, send)`**, thins `opai`'s reports and hands each one to a `send`
  closure. Only the free function `reporting()` ties it to `enhance:progress`, so a second destination costs nothing.
- **`autopilot.rs`, `Analyses`**, is a table of cancellation tokens by run name:
  - it remembers up to 16 stops that arrive before their run
  - `Registration` removes its own entry on drop

  Nothing in it is about Autopilot except its name and its doc comments.
- **`images`**:
  - `Opened::resolve(identity)` answers an admitted path
  - `load_framed(path, crop)` decodes and frames it
- **`opai::image::save(pixels, destination, format, options)`** writes and answers the bytes written. It refuses
  options for the wrong format before touching the destination.
- **`frontend/ipc/enhance.ts`**:
  - `mintRun()` is the one counter every run name comes from
  - `Operation` is the chain's wire type, with faces already resolved inside a face recovery

The reference, read in `../open-photo-ai`:

- **`ImageService.ExportImage`** does the following:
  1. emits 0.1
  2. runs inference, sending its progress to the *canvas* event
  3. emits 0.9
  4. claims the output name
  5. encodes
  6. emits `COMPLETED` with the byte count
- **`getOutputPath`** claims `name`, then `name_1` .. `name_999`, each with `O_EXCL`. It falls back to `_999`
  unclaimed.
- **`utils.EncodeImage`** writes:
  - AVIF at speed 6
  - TIFF with Deflate
  - everything else at the encoder's defaults, with the quality on top

## Goals / Non-Goals

**Goals:**

- Exporting is independent of the canvas in both directions: neither stops or drops the other's work.
- An export never leaves a stray file, whether it fails, is stopped or is refused.
- Progress is precise enough for slice 2 to draw Enhancing with a real fraction, the fetch with its own fraction, and
  Writing as its own stage.

**Non-Goals:**

- Composing the destination's name from prefix, suffix, location and "preserve". Slice 2 does that, because the
  dialog shows those names before anything runs.
- Running Autopilot before an export. That happens in the window, in slice 2, through the `suggest` seam that already
  exists.
- Queue order, batching, or an overall fraction. The window asks for one file at a time.
- Reporting an export as a separate kind of failure when a model fails to download. The reference distinguishes
  `ERROR_DOWNLOAD` by searching the message for `[download]`. The library's message is carried whole here, and slice 2
  decides what to show.
- Refreshing an opened photograph whose own file an overwrite replaced. See Risks.
- TIFF compression (see proposal, Impact).
- Analytics events.

## Decisions

### D1. One file per request. The window drives the batch

`export` exports one photograph. The window loops over its queue and calls it once per file.

*Why not a batch command:* Autopilot runs **between** files, in the window, through `suggest`. The suggestions land
in the stack and the chain is built from them there. A batch command would need the window to hand over chains it
does not have yet, or would need Autopilot moved behind the command. Per-file also leaves one failure per answer, and
screen 15e continues past a failure anyway.

### D2. The cancellation table is promoted, not copied

Export is `Analyses`' second caller. By this project's tier rule, the table moves to a shared module **before**
export is written: `gui/src/stops.rs`, holding `Stops` and `Registration`.

- Autopilot manages one instance and export manages another, so a stop by name can only reach the kind of work it was
  meant for.
- The type is unchanged and keeps its tests. Only the prose that says "analysis" becomes "job".
- Tauri keeps one managed value per type, so the two tables cannot be two instances of one type. `Stops<Of>` is
  generic over a marker that is never constructed, and each caller names its own: `Analyses = Stops<Analysis>` and
  `Exports = Stops<Export>`.

*Alternative:* one shared instance for both. Rejected. Run names cannot collide, because `mintRun` is the one
counter. But two instances keep `cancel_suggest` from ever being able to stop an export, and the cost is one more
managed state.

### D3. `export_with`: the order of what happens is the contract

```text
  1. judge the whole chain            --> refused: nothing fetched, nothing read
  2. resolve the source               --> refused: unknown source
  3. register in Stops                --> None: stopped before it started (answer Stopped)
  4. load_framed                      --> refused: unreadable source
  5. Opai::process, depth Source,     --> Cancelled or Shutdown: answer Stopped
     reporting to export:progress
  6. release the held-back report; check the token one last time
  7. report Writing
  8. claim the destination (D4)       --> refused: unwritable, naming it
  9. save (D5)                        --> failed: claim released, unwritable
 10. commit the claim                --> failed: destination as it was, unwritable
 11. answer { path, bytes }
```

- Steps 1 and 2 come first so a refusal costs nothing, as in `enhance_with`.
- There is **no slot write**, so a canvas run can neither be displaced by an export nor displace one. `Runs` is not
  touched at all, which is what the "does not disturb the enhanced view" requirement asks for.
- A chain ended by `Shutdown` answers `Stopped`, as Autopilot's analyses do: the application is going down, so there
  is no failure for the window to show, and nothing has been written yet.
- Step 6 is the **last point a stop takes effect**. After Writing is reported, the write finishes. The encode and the
  write are short compared with a chain, and a half-written file that a stop then has to clean up is the worse
  outcome. The spec states this.
- The `Enhancer` trait from `enhance/run.rs` is reused as the seam, so the tests run without a model on disk.
- The claim comes **after** the chain (step 8, not step 3). A long export holds no placeholder that a user could see
  in Finder and mistake for a finished file, and a stop during the chain has nothing to clean up.

### D4. The destination claim is a guard, not a release function

`destination.rs` has `claim(requested, overwrite) -> Result<Claim, io::Error>`.

- **Overwrite on:** the destination is not touched until commit. The claim creates a hidden name beside it
  (`.name.ext.opai-1`, the same `create_new` loop) and the export is written there. `commit()` renames it over the
  destination, carrying over the replaced file's permissions.
  - `save` truncates before it encodes, so writing into the destination directly would let a failed encode or a full
    disk destroy the file being replaced. That file can be the photograph itself.
  - Not a `rust_sak::fs` temporary file. Those are created readable by their owner alone, and the export would keep
    that mode after the rename.
- **Overwrite off:** it tries `requested`, then `stem_1.ext` .. `stem_999.ext`, each with
  `OpenOptions::new().write(true).create_new(true)`. The first one that succeeds wins, and the filesystem decides any
  race, as in the reference.
  - `AlreadyExists` moves on to the next name.
  - Any other error is returned at once. A directory that does not exist should not be probed 1,000 times.
  - Running out of names returns an error naming `requested`. This is the proposal's divergence.
- **`Claim` removes its placeholder on drop unless `commit()` succeeded.** Every early exit after step 8 releases the
  name, a panic included. That is the same shape `Registration` uses for the table.

`save` then opens the claimed path with `File::create`, which truncates the empty placeholder, so `opai::image` needs
no change. `commit()` can fail only when overwriting, if the rename is refused, and that is answered as unwritable.

### D5. Format and quality become `EncodeOptions` in one place

The wire format is `"bmp" | "gif" | "jpeg" | "png" | "tiff" | "avif" | "heic" | "webp"`.

- The four lossy names match `QUALITY_FORMATS` in `stores/settings.ts`.
- `heic` is the window's word, mapped to `ImageFormat::Heif`.

`encode_options(format, quality)` does this:

1. starts from `EncodeOptions::default_for(format)`
2. sets the quality on the four lossy arms, clamped to 1..=100
3. sets AVIF's speed to 6, as the reference does

The lossless arms ignore the quality, and the spec says so.

*Why here and not in `opai`:* one caller, and the reference keeps its encoder choices in its GUI service. The
quality is what the settings store holds, and its range is a window's decision.

### D6. `export:progress` is its own event, with its own wire

```text
ExportProgress =
  | { run, phase: "enhancing", family, stage?, chainFraction, installFraction? }
  | { run, phase: "writing" }
```

- The export builds a `Reporting` with `Reporting::new(run, send)`. `send` wraps each `RunProgress` as
  `phase: "enhancing"` and emits it on `export:progress`.
  - Thinning, the rule that a cached operation has no stage, and the held-back final report all come for free.
  - The enhance wire's `Stage` gains no `writing` arm it would never send.
- `writing` is emitted by `export_with` itself at step 7, once and without a fraction, because the encoder has no
  progress to report.

*Why not `enhance:progress`:* `useEnhancementRun` filters by run, so a stray report would be ignored. But the canvas
chip is not the only listener, and faces already had to prove "not reported on that indicator" by construction. A
separate event makes it true by construction here too.

### D7. The answer and the errors

```text
Exported  = { outcome: "exported", path, bytes } | { outcome: "stopped" }
ExportError (tag "kind") =
    notReady | unknownSource { identity } | unknownOperation { index, reason }
  | unreadableSource { identity, message } | enhance { message }
  | write { path, message }
```

- The first five variants have the same shapes as `EnhanceError`, so the window can share one formatter.
- `write` carries the destination the window asked for, plus the OS's sentence, untranslated. That is what the
  screen-15e tooltip copies to the clipboard.
- `path` in the answer is the path actually written, which is what the reveal button needs after numbering.

### D8. `frontend/ipc/export.ts`

- `exportImage(source, operations, processor, request, crop?)` answers `{ run, done }`. It mints the run before the
  invoke. The request is `{ destination, format, quality, overwrite }`.
- `cancelExport(run)`.
- `onExportProgress(handler)`.

The command and event names are written once here and pinned by a test, as in `ipc/enhance.ts`. The function is
named `exportImage` because `export` is a reserved word.

### D9. Model sessions and execution providers

Nothing new is owned.

- An export's chain takes its leases inside `Opai::process`, exactly as a canvas run does. The registry already allows
  concurrent leases within its memory budgets, so an export and a canvas run can overlap. The budget decides whether
  a model is shared or rebuilt, not this module.
- A stop cancels the token handed to `ProcessOptions::cancel`, so leases are released on the same path a displaced
  canvas run takes.
- `processor` is passed through as `enhance` passes it.

### Crate division

| Crate | What it gets in this slice |
|---|---|
| `opai` | Nothing. |
| `gui` | Everything: the command, the claim, the encoder choices, the event, the promoted table and the IPC wrapper. Each one has a single caller, which is a window. |
| `cli` | Nothing. |

### rust-sak

- `image` (through `opai`): `EncodeOptions::default_for` and the encoders.
- `fs` has no free-name claim, and none is added. It is a ten-line `create_new` loop with one caller. If a second
  project needs "claim a sibling name", that is when it becomes a rust-sak helper.
- TIFF compression is missing from `EncodeOptions::Tiff`. That gap belongs in rust-sak and is left for its own change.

## Risks / Trade-offs

- **[Risk] An overwrite replaces an opened photograph's own file.** The window still holds that file under its old
  identity. Renditions read from the path would then draw the new bytes under the old identity. → The reference has
  the same exposure. Slice 2's dialog shows the warning-coloured note for exactly this case. Refreshing the record is
  left for a change that needs it.
- **[Trade-off] Export does not reuse the canvas's result.** The canvas runs at 8 bits and export at the source's
  depth, and the run cache keys on depth (`run-cache`). So exporting the photograph on screen runs its chain again. →
  This is the price of keeping 16 bits. For an 8-bit source the two depths produce the same pixels, but the cache key
  still differs. A measurement can argue for folding them later.
- **[Risk] An export and a canvas run compete for the processor.** → The registry's budgets bound memory, and slice 2
  shows the dialog as modal, so the only canvas runs during an export are the ones Autopilot's stack writes trigger.
- **[Trade-off] A stop during Writing is not honoured.** → Bounded by one encode. The spec says so, so the dialog can
  show Abort as disabled at that point if it wants to.

## Validation

- **Vitest** for `ipc/export.ts`, with `invoke` and `listen` mocked:
  - the run is minted before the invoke
  - the command and event names are right
  - the request's shape on the wire is right
- **Rust unit tests**, with the `Enhancer` fake from `enhance/run.rs`:
  - every refusal, and that nothing is left behind
  - the order in D3
  - the stop cases
  - that `Runs` is untouched
- **Rust unit tests** for the claim, in a temp directory: numbering, a race between two claims, release on drop, and
  refusal after `_999`.
- **An `#[ignore]`d integration test** that exports `fixtures/test.dat` through the real `Opai` with a 2x Kyoto
  upscale, as PNG and as JPEG. It checks the dimensions and the byte count. It is run by hand on macOS with the CPU
  provider and with CoreML.

CUDA and TensorRT on Windows and Linux are not tested. The slice adds nothing provider-specific.

## Slices

| # | Change | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `add-gui-export-1-seam` | The `export` and `cancel_export` commands, the destination claim with `_N` numbering, the format and quality mapping, the `export:progress` event, the cancellation table promoted out of `autopilot.rs`, and `frontend/ipc/export.ts`. Nothing a user sees. | nothing | **current** |
| 2 | `add-gui-export-2-dialog` | The sidebar button, enabled on ≥1 selected image and reading "Export image" or "Export images". The export dialog in every state from screens 15 to 15e. Every selected image queued, with one that has nothing to do exported as it is. Autopilot run before exporting a never-visited image, with its suggestions written into the stack. Row stages Queued, Analysing, Enhancing, Writing, Done and Failed, with new i18n keys. The batch continues past a failure. | slice 1 | pending |
