## Context

See proposal.md for why. What this slice builds on, as it is today:

- **`frontend/ipc/export.ts`** (slice 1):
  - `exportImage(source, operations, processor, request, crop?)` answers `{ run, done }`, with the run minted
    before the invoke.
  - `done` resolves `{ outcome: "exported", path, bytes }` or `{ outcome: "stopped" }`, and rejects with an
    `ExportError`.
  - `cancelExport(run)` stops an export.
  - `onExportProgress` delivers `enhancing` reports, which are a `RunProgress` under the export's run, and then one
    `writing` report.
- **`gui/src/export/`** (slice 1): the `export` command. A stop takes effect up to Writing and not after. A stopped or
  failed export leaves no file.
- **`hooks/useAutopilot.ts`, `analyse(file, crop)`**:
  - registers the run in `useAutopilotStore` (`begin`/`end`), keyed by path
  - writes the stack through `addEnhancements`
  - on failure, it toasts `errors.autopilotFailed` and declines the current photograph
  - it resolves `void` whatever happened, so a caller cannot tell a written stack from a failure
- **`hooks/useEnhancementRun.ts`, `withFaces(operations, faces)`**:
  - a private helper that puts the chosen faces into every face recovery
  - faces come from `useFacesStore` by `(identity, crop)`
  - the user's skips come from `useSkippedFaces(identity)`, filtered by `lib/faces.ts`'s `enabledFaces`
- **`stores/files.ts`**:
  - `files` (drawer order)
  - `selectedPaths: Set<string>`. Its comment already names the export queue as the one consumer that wants the
    picked records, once, when it opens.
- **`stores/settings.ts`**:
  - `processor`
  - `quality: Record<QualityFormat, number>`, plus `DEFAULT_QUALITY`, `clampQuality` and `QUALITY_FORMATS`
  - written whole through the store's own save path
- **`gui/src/images/files.rs`**:
  - `open_images` uses `tauri-plugin-dialog`'s Rust API from an `async` command
  - `reveal_image` refuses any path the registry did not admit, and reveals through `reveal.rs`
- **`features/sidebar/Sidebar.tsx`** draws the export button permanently `disabled`.
- **i18n**: `sidebar.exportImage` and an `export.*` block ported from the reference. The dialog reuses most of it.
  The table-header keys `export.queue.{image,size,type}` and the three-state keys
  `export.queue.{processing,completed,error,errorTooltip,errorDownloadTooltip}` describe a layout the design replaced.

The reference, read in `../open-photo-ai/cmd/gui/frontend/src`:

- `features/export/*`
- `utils/export.ts`: `getExportEligible`, `groupByChain`, `resolveExt`, `getExportInfo` and
  `resolveQualityFormat`
- `stores/export.ts`: persisted as `export-storage`, with `runState` and `key` excluded

Each decision below says where it follows the reference and where it does not.

## Goals / Non-Goals

**Goals:**

- One place owns the batch. The dialog's close policy, the buttons, the summary and the rows all read one state, so
  they cannot disagree about whether a batch is running. The reference moved `runState` into its store for this
  reason.
- The batch's rules can be tested without a window: which files, in what order, under what name, with what quality,
  and what each outcome does to a row.
- Autopilot at export is the canvas's own analysis, not a second implementation of it.

**Non-Goals:**

- Running files concurrently. Slice 1's D1 is one file per request, and the batch awaits each file in turn.
- Refreshing an opened photograph whose own file an overwrite replaced. That is slice 1's recorded risk, and the
  warning footnote is the mitigation.
- Analytics events. The reference tracks five export events. `analytics` is a stored preference that nothing reads
  yet, by design.
- TIFF compression. That gap is slice 1's, and it belongs in `rust-sak`.
- A stop during face detection. `detectFaces` has no cancel command (the canvas lets detections finish as a warm
  cache). An abort during it waits for it to settle and discards what it found (D3).

## Decisions

### D1. The queue is a snapshot of paths; each row reads its stack live until its turn

The dialog snapshots the queue when it opens:

```ts
files.filter((file) => selectedPaths.has(file.path))
```

It then orders the snapshot with `groupByChain` (D5). The snapshot is **paths**, fixed for the dialog's life. The spec
says the queue does not change while the dialog is open.

What each row *shows* is read live from the stores:

- the name
- the dimensions after its enhancements
- the types

So a settings change while idle updates them.

What each row *runs* is read at the moment its turn comes:

- its stack
- its crop
- its chosen faces
- the processor

That is what lets an Autopilot suggestion written into the stack mid-batch (D3) be the chain that runs.

- *Why not re-derive the queue from the stores, as the reference's memo does:* here Autopilot's answer is written into
  the stack, so a re-derived queue would regroup, and reorder the rows, in the middle of a batch.
- *Order within the snapshot:* drawer order, grouped. The reference iterates its `selectedFiles` array. This store
  keeps a Set, and the drawer order is the stable order the user can see.

### D2. `stores/exportBatch.ts` owns the batch; `features/export/batch.ts` drives it

A store that is not persisted holds:

```text
phase:   "idle" | "running" | "ended"
queue:   string[]                                  (paths, D1)
rows:    Map<path, Row>
current: { path, run?, analysing? } | undefined    (what Abort has to stop)
aborted: boolean
```

```text
Row =
    { stage: "queued" }
  | { stage: "analysing" }
  | { stage: "enhancing", fraction }
  | { stage: "writing" }
  | { stage: "done", path, bytes }
  | { stage: "failed", reason }
```

- **The actions:**
  - `open(paths)` snapshots the queue and marks every row Queued. The dialog calls it on mount.
  - `reset()` does the same for Export again.
  - `setStage(path, row)`.
  - `abort()`.
- **The summary and the overall fraction are selectors**, not stored values:
  - written = Done rows
  - failed = Failed rows
  - remaining = Queued rows after the current row
  - overall = (Done + Failed + the current fraction) / the number of rows

**`runBatch(settings, quality)`** is a plain async function in `features/export/batch.ts`, not a hook. It is written
against the stores through `getState()` at each step, as `analyse` is. It is started by Save, with the settings and
the committed quality it was saved with. For each queued path, in order:

```text
  aborted?                                       --> stop; the rest stay Queued
  record = files.find(path); identity absent?    --> Failed (unreadable, D6)
  Autopilot on and no stack for path?            --> Analysing; awaitAnalysis (D3)
        failed                                   --> Failed (D6), next
        aborted meanwhile                        --> Queued, stop
  operations = stack ?? []
  a face recovery among them, faces unknown?     --> Analysing; detectFaces; setFaces
        rejects                                  --> setFaces(.., []), and exported anyway
        aborted meanwhile                        --> Queued, stop
  operations = withFaces(operations, enabledFaces(faces, skipped))
  { run, done } = exportImage(identity, operations, processor, request(D4), crop)
        --> Enhancing (fraction 0) at once; the progress listener updates it
  done:
        exported  --> Done { path, bytes }; recorded for reveal by the backend (D7)
        stopped   --> Queued (only Abort stops an export), stop
        rejects   --> Failed (D6), next
```

When the loop ends, the phase becomes `ended`, whether the batch finished, failed in part or was aborted.

- **A face detection that fails** is recorded as no faces at that framing, and the file is exported anyway. This is
  what the canvas's run does: a failed detection must not cost the photograph the rest of its chain. Unlike the canvas,
  the batch shows no notice (the dialog covers the window, and its rows report only what stops a file) and the row
  reads Done. The face recovery in the chain then restores no face. The failure is logged.

- **Progress.** One `onExportProgress` subscription is held by the dialog for its life. It matches `report.run`
  against `current.run`:
  - `enhancing` sets the row's fraction from `chainFraction`
  - `writing` sets Writing

  A report for any other run is dropped.
- **Abort** sets `aborted` and stops what is in flight:
  - `cancelExport(current.run)` while exporting
  - `useAutopilotStore.stop(path)` while an Autopilot analysis runs
  - nothing while a face detection runs, which is D3's non-goal

  The loop observes `aborted` after every await. So the file in progress becomes Queued, and no new file starts.
- **An export answered `exported` after Abort** keeps Done. That happens when the stop arrived during Writing, which
  slice 1's spec lets finish. The file exists, and the row says so.
- **Unmount.** The dialog cannot close while `running` (D8). The unmount cleanup still calls `abort()`, for the case
  the window itself goes away. The reference's comment records the cost of a batch writing on behind an unmounted
  dialog.

*Why a store, not component state:* the title bar, the rows, the buttons and the close policy are four components.
The reference kept `runState` in its store for this reason, and here the rows' stages join it for the same one.

*Why the loop is not in a hook:* it outlives any single render and awaits across steps. The canvas's `analyse` is
written the same way for the same reason.

`withFaces` moves from `hooks/useEnhancementRun.ts` to `lib/faces.ts`. This slice gives it a second caller, and by
the project's tier rule a second caller promotes the function before it is used. It is not copied.

### D3. `analyse` becomes awaitable; the export waits on an analysis already in flight

`analyse(file, crop, { notify })` answers a `Promise<AnalysisOutcome>`:

```text
AnalysisOutcome = "added" | "stopped" | { failed: unknown }
```

- **The canvas trigger** calls it with `notify: true` and ignores the answer. Its behaviour does not change: the
  toast, the decline and the `finally: end` stay as they are.
- **The batch** calls it with `notify: false`:
  - no toast
  - no decline, because the photograph may not be current, and a decline is about the trigger
  - a failure comes back as `{ failed }`, for the row

`awaitAnalysis(file, crop)` is the batch's entry point:

1. If `useAutopilotStore.analysing` already holds the path, it subscribes to the store until that entry is removed,
   then answers `"added"` if the stack now exists and `{ failed }` otherwise. No second analysis is asked for,
   because the spec says an export waits.
   - The canvas trigger reported its own failure already, and the row carries a generic reason (D6).
2. Otherwise it calls `analyse` with `notify: false`. Because `analyse` registers in the same store, the canvas
   trigger then sees the entry and does not start a second analysis of the current photograph.

An analysis that suggests nothing still writes an empty stack (`addEnhancements(path, [])`). That is today's
behaviour, and it is what marks the photograph as having had a list. The export then runs an empty chain, which
writes the photograph as it is.

*Alternative:* call `suggest` directly from the batch, as the reference does. Rejected. It would be a second copy of:

- the family filter
- the face follow-up with `noFaceToRestore`
- the catalogue lookup
- `suggestedOperation`

And the suggestions would have to be written into the stack by a path the canvas does not share. The spec says they
are added "by the same rules as any other analysis".

### D4. The export settings: a persisted store, plus a quality draft in the dialog

**`stores/exportSettings.ts`** is persisted under `export-storage`. That is the reference's key, as `settings-storage`
is shared with it. It holds:

```text
prefix:    string              ("")
suffix:    string              ("")
location?: string              (absent = the original directory)
overwrite: boolean             (false)
format:    "preserve" | ExportFormat   ("png")
```

It has a rehydration repair in the shape of `stores/settings.ts`: a stored `format` outside the list becomes `png`,
and a non-string `location` is dropped.

- *The reference's own spellings:* it stores `jpg` where the wire says `jpeg`. The repair maps `jpg` to `jpeg`, so a
  Wails user's choice survives.

**The quality draft** is `useState`, in the export settings panel, seeded from `useSettingsStore.getState().quality`.
Seeding through `getState()` rather than a selector follows the reference's reasoning: the Settings dialog must not
yank a half-made edit.

- **Save commits only the format the slider stands for** (`queueQualityFormat`), through the settings store's save
  path, and then reads the record back. The bytes are written with exactly the persisted values, clamping included.
  A draft left on another format, by moving the slider and then changing the format, is discarded, since it was never
  shown as what this batch writes. Where the slider is hidden, nothing is committed.
- **Per-file quality:**

  ```text
  quality = qualityFormat(file) ? committed[qualityFormat(file)] : DEFAULT_QUALITY.jpeg
  ```

  A lossless format ignores the quality, as slice 1's D5 says. The number sent is irrelevant there, and it is sent
  only because the wire takes one.

**The request** for one file is:

```text
{ destination: destinationFor(file, settings), format: formatFor(file, settings.format),
  quality, overwrite: settings.overwrite }
```

### D5. `lib/export.ts`: the pure rules

- **`formatFor(file, choice) -> ExportFormat`** and **`extensionFor(file, choice) -> string`**:
  - Preserve keeps the source's extension when it maps to a writable format:
    - `jpg`/`jpeg` → `jpeg`
    - `tif`/`tiff` → `tiff`
    - `heic`/`heif` → `heic`
    - `png`, `bmp`, `gif`, `webp` and `avif` → themselves
  - Anything else writes `tiff`.
  - A chosen format writes its canonical extension (`jpeg` → `jpg`, the rest their own name). This is the reference's
    `resolveExt`, with `jpg` as the extension the reference's `jpg` value produced.
- **`destinationFor(file, settings)`**:
  - the folder is `settings.location`, or the source's own directory
  - the file name is `prefix + stem + suffix + "." + extension`
  - Both separators are accepted, as `lib/paths.ts`'s `fileName` accepts them. It joins with the separator the folder
    is already spelled with.
  - `fileName` and a new `directoryOf` live together in `lib/paths.ts`.
- **`groupByChain(paths, stackOf)`**:
  - the signature is the operation families in order, joined, so options such as scale or intensity do not split a
    group
  - Groups keep the drawer order of their first member, as the reference does.
  - A path with no stack has the empty signature.
- **`queueQualityFormat(files, choice)`**: the one lossy format every file resolves to, or `undefined` (the slider is
  hidden). This is the reference's `resolveQualityFormat`.
- **`describeExportError(t, error, destination)`**: see D6.

### D6. What a Failed row says

The reason is composed once, when the row fails, and stored on the row. The tooltip draws it, and the click copies
the same string. It is composed like this:

| Cause | Reason |
|---|---|
| `write { path, message }` | `export.queue.writeFailed`: the file name and the folder of the destination asked for, then the OS message, untranslated. |
| `enhance { message }`, `unreadableSource { message }` | `export.queue.enhanceFailed`, then the message. |
| `notReady`, `unknownSource`, `unknownOperation` | `export.queue.enhanceFailed` with the kind. Programming errors, not user ones. |
| No identity on the record | `export.queue.unreadable`. |
| Autopilot analysis failed | `export.queue.analysisFailed`, plus its message where the rejection carries one. |

A failed model download reaches this table as `enhance` with the library's message, which names the download. The
reference's separate `ERROR_DOWNLOAD` tag, found by searching the message for `[download]`, is dropped (slice 1,
Non-Goals).

**Copying** is `navigator.clipboard.writeText`, from the click itself. The webview's origin is a secure context, and
a click is a user gesture. The tooltip is a controlled Radix `Tooltip`:

- open on hover
- its text swaps to `export.queue.copied` after a successful copy
- it resets on leave

A copy that the browser refuses leaves the tooltip showing the reason, and the refusal is logged.

### D7. Two Rust commands

**`pick_directory(title: String) -> Option<String>`**, in a new `gui/src/export/directory.rs`:

- It is an `async` command, for the threading reason `open_images` records.
- It calls `app.dialog().file().set_title(&title).blocking_pick_folder()`.
- `None` is a dismissal and is not an error. A path that is not UTF-8 is answered lossily, as `ImageRecord.path` is.

**`reveal_export(path: String)`**:

- **The record.** A new managed `Written` (a `Mutex<HashSet<PathBuf>>`) records every path an `export` answered as
  exported. The `export` command in `export/mod.rs` inserts the path after `export_with` answers.
  - It is recorded in the command, not in `export_with`, so the seam's tests stay free of Tauri state.
  - The path recorded is the one actually written, numbered or not.
- **The check.** `reveal_export` refuses any path not in `Written` with `ExportError`-style
  `{ kind: "revealExport", message }` and opens nothing. Otherwise it reveals through `reveal::reveal`.
- The frontend shows `errors.revealFailed` on any rejection. That string exists already, and it says what this
  failure means.

*Why a record and not `reveal_image`:* `reveal_image` checks the images registry. An export's output is not an opened
image, and admitting it there would make it servable as one. *Why not reveal any path the webview sends:* that is the
reference's `RevealInFileManager`, and `images/files.rs` records why this application refuses to reveal unvetted
paths.

**Wrappers.** `ipc/export.ts` gains `pickDirectory(title)` and `revealExport(path)`. The command names are pinned by
the existing `export.test.ts`, alongside `export` and `cancel_export`.

### D8. The dialog's components

`features/export/`:

```text
ExportDialog     shadcn Dialog, 1120 x 672 (the design's; the reference's 70rem x 42rem)
 |- title bar    "Export", the summary (D2 selector), close box only when phase != "running"
 |- overall bar  Progress, 3px
 |- ExportQueue  "Queue (n)" + "in pipeline order"; one ExportRow per queued path
 |    '- ExportRow   thumbnail (renditionFor at a small bound, framed), name, dims, size, types, stage pill,
 |                   Failed tooltip (D6), reveal button once Done, own 2px bar only while it is current
 '- ExportSettingsPanel   Filename (two inputs), Save to (Select: original / Browse… / the browsed path) with the
                          switch and footnote, Format (Select), Quality (Slider + value, D4), and the two buttons
```

- **The close policy.** `onOpenChange` ignores every close while `running`, and `onInteractOutside` is always
  prevented. This is the reference's rule: backdrop never, Escape and X not while busy. It is held by the component
  that reads the phase.
- **The buttons:**

  | Phase | Left | Right |
  |---|---|---|
  | idle | Cancel, which closes | Save (primary), which runs |
  | running | Abort | Save, disabled at 50% |
  | ended | Close | Export again (secondary), which calls `reset()` |

  Save and Export again leave the settings as they are.
- **The Save-to Select.** It follows the reference's hidden-item trick: the chosen folder is the displayed value, and
  choosing Browse… opens the picker. A dismissed picker keeps the previous value, including the original directory.
- **Settings while running** are disabled. The reference leaves them live. `runBatch` runs with the settings it was
  saved with, while the rows' names follow the store, so a live column would let the rows show names other than the
  ones being written.
- **The row's dimensions after** are `framedDimensions(file, crop)` × `upscaleFactor(stack)`, as `NavbarDimensions`
  computes them.
- **The row's sizes** use the same byte formatter the rest of the window uses (`lib/utils.ts` gains `formatBytes` if
  no such formatter exists).
- **The row's types** are the source's extension and the extension it will write, upper case.

### D9. The sidebar button and i18n

- **`Sidebar.tsx`.** The button is enabled on `selectedPaths.size > 0`. Its label is `t("sidebar.exportImage",
  { count })`, with the key split into `exportImage_one` and `exportImage_other`, which the parity test normalises.
  It mounts `ExportDialog` while open.
  - The sidebar test that pins the button as always disabled is replaced.
  - `gui-shell`'s "unavailable while none is open" still holds: nothing can be picked with nothing open.
- **New keys under `export.queue`:**
  - `pipelineOrder`
  - the six stages: `queued`, `analysing`, `enhancing`, `writing`, `done`, `failed`
  - `writeFailed`, `enhanceFailed`, `unreadable`, `analysisFailed`, `copied`
- **Summary keys under `export.summary`, pluralised:**
  - `ready`
  - `running`
  - `ended`
  - `endedFailed`
- **Removed:** the header and three-state keys listed in Context.
- **All thirteen locales** get every addition and removal.

### Crate division

| Crate | What it gets in this slice |
|---|---|
| `opai` | Nothing. |
| `gui` | Rust: `pick_directory`, `Written` and `reveal_export`, and their registration. Frontend: everything else. Each piece has one caller, which is this dialog, and the reference keeps all of it in its frontend. |
| `cli` | Nothing. |

### rust-sak

None is used directly. The folder picker is `tauri-plugin-dialog` and the reveal is `tauri-plugin-opener`. Both are
existing dependencies, reached through their Rust APIs as this crate already does. Neither is infrastructure that
`rust-sak` covers.

### Model sessions and execution providers

Nothing new is owned:

- Each file's chain takes its leases inside `Opai::process`, through slice 1's command.
- The batch runs one export at a time, so a batch of one chain loads each model once. The grouping in D5 keeps that
  true for mixed batches within the registry's budgets, as the reference's comment argues.
- An Autopilot analysis and a face detection at export are the same commands the canvas uses, with the same leases.
- The processor is read from Settings at each file's turn.

## Risks / Trade-offs

- **[Risk] An abort during a face detection waits for it.** → Bounded by one detection, which is short next to a
  chain. The row reads Analysing until it settles, then Queued. A cancel for detections belongs to the change that
  needs it.
- **[Risk] A canvas-started analysis that fails gives the export row a generic reason.** The failure was already
  shown by the canvas's own notice. → Acceptable. The alternative is threading the canvas's error through the store
  for this one case.
- **[Trade-off] The settings column is disabled while running.** The reference leaves it live. → Otherwise the rows'
  names would change under a batch that is writing the old ones. Only one batch runs at a time, so nothing is lost.
- **[Risk] `navigator.clipboard` refused by a webview.** → Logged, and the tooltip keeps showing the reason, so the
  user can still read it. Verified by hand on macOS. WebView2 and WebKitGTK are untested.
- **[Trade-off] Writing Autopilot's suggestions into the stack changes photographs the user never looked at.** →
  This is the slice table's decision, and it is what makes the canvas show afterwards what was exported. The spec
  records it in `gui-autopilot`.

## Validation

- **Vitest, pure** (`lib/export.test.ts`):
  - extension and format for Preserve (JPEG, PNG, HEIF, RAW) and for each chosen format
  - destination with each separator and with a browsed folder
  - grouping and its stability
  - the queue's quality format
  - each error's reason
- **Vitest, stores:**
  - `exportSettings`: its defaults, its persistence, and the repair of `jpg` and of a bad format
  - `exportBatch`: `open`, `reset`, the summary selectors and the overall fraction
- **Vitest, `runBatch`**, with `ipc/export`, `ipc/autopilot`, `ipc/faces` and `ipc/catalogue` mocked:
  - the order
  - continuing past a failure
  - Abort at each stage
  - an `exported` answered after Abort
  - Autopilot on and off
  - an analysis already in flight
  - an empty suggestion
  - a face recovery with and without known faces, and a face detection that fails
  - the committed quality per file
- **Vitest, components:**
  - the button's availability and plural
  - each phase's buttons and close policy
  - the quality slider's visibility
  - the footnote swap
  - a Failed pill's tooltip and copy
  - the reveal call
- **Rust unit tests:**
  - `Written` records only exported paths
  - `reveal_export` refuses an unrecorded path without opening anything
  - `pick_directory` is registered (build)
- **By hand, on macOS** (Apple silicon, CPU and CoreML), handed to the user to run:
  - a four-file mixed batch
  - one file made to fail with a read-only folder
  - Abort mid-batch
  - Export again
  - a numbered name revealed

Windows and Linux (the folder picker, the reveal, the clipboard) are not tested. The slice adds nothing provider
specific, and CUDA and TensorRT are untested.

## Slices

| # | Change | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `add-gui-export-1-seam` | The `export` and `cancel_export` commands, the destination claim with `_N` numbering, the format and quality mapping, the `export:progress` event, the cancellation table promoted out of `autopilot.rs`, and `frontend/ipc/export.ts`. Nothing a user sees. | nothing | done |
| 2 | `add-gui-export-2-dialog` | The sidebar button, enabled on ≥1 picked image and reading "Export image" or "Export images". The export dialog in every state from screens 15 to 15e. Every picked image queued, with one that has nothing to do exported as it is. Autopilot run before exporting a never-visited image, with its suggestions written into the stack. Row stages Queued, Analysing, Enhancing, Writing, Done and Failed, with new i18n keys. The batch continues past a failure, and Abort ends it. `pick_directory` and `reveal_export`. | slice 1 | **current** |
