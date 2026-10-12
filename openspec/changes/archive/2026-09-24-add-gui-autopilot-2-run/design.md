## Context

Slice 1 (`add-gui-autopilot-1-seam`) left the window everything it needs to run Autopilot without any Rust change:

- `ipc/autopilot.ts`: `suggest(source, processor, families, crop?)` answers `{ run, done }`, and `cancelSuggest(run)`
  stops an analysis by name. A stopped analysis rejects with `{ kind: "stopped" }`, including one whose answer had
  already been determined. Suggestions arrive in pipeline order. Only an upscale carries a value, its `scale`, and
  that scale is read off the **framed** pixels by the same ladder `defaultScale` uses.
- The analysis's face signal runs the same detection `detect_faces` runs, at the same precision and on the same
  framed identity. So a `detectFaces` call made right after it is a run-store hit.

The frontend pieces this slice builds on, as they are today:

- `stores/enhancements.ts`: a stack per **path**. A stack emptied by removals keeps its key, holding `[]`. That makes
  `enhancements.has(path)` exactly the reference's "this file has had a list" test.
- `stores/faces.ts`: `setFaces(identity, crop, faces)` records a detection and seeds the size rule (`decidedSkips`)
  in one `set`. `useImageFaces` matches the framing **by reference**.
- `hooks/useEnhancementRun.ts`: detects before running only when `useImageFaces` misses. It draws progress only for
  the run it started (`mine.current`).
- `stores/files.ts`: `registerFileOwner` tells every holder of per-file state when a file closes.
- `lib/enhancements.ts`: `newOperation(family, entry, stored, file)` builds what the add menu adds, and
  `familiesWhere` orders families by `ENHANCEMENTS`.
- `stores/settings.ts`: `autopilotExcluded`, `models` and `processor`.
- The i18n keys `sidebar.analysing` and `errors.autopilotFailed` already exist in every locale. The theme already
  carries `--success-bright` and `--activity`, the pair of colours design screen 04 gives the analysing spinner.

The reference's own trigger is `SidebarEnhancements`'s effect, read in `../open-photo-ai`. It fires on
`[autopilot, hasEnhancement, file]`, applies unconditionally, and cancels nothing. Its comments warn against one
thing specifically: tying the spinner's reset to the effect's cleanup, because writing the stack re-runs the effect.

## Goals / Non-Goals

**Goals:**

- Autopilot runs exactly as `gui-autopilot` describes, and the only state it adds is the analyses in flight.
- A stack written by Autopilot starts its run with the faces already known, so no second detection and no detection
  sweep appears on the canvas.
- Every stop (close, switch-off, reframe) is explicit and named, never a side effect of a React cleanup.

**Non-Goals:**

- Any Rust change.
- Restarting an analysis when the processor or the Autopilot family choice changes mid-flight. The spec keeps the
  choice in force at the moment of asking.
- Analytics events. The reference sends `autopilot_run` and `autopilot_failed`, but this application has no analytics
  pipeline yet. The slice that builds one adds them.
- Reporting an incomplete analysis to the user. As slice 1 D4 decided, it is logged only.

## Decisions

### D1. The analyses in flight are a store of their own, keyed by path

`stores/autopilot.ts` holds these fields:

- `analysing: Map<string, { run: string; crop?: CropInfo }>`: the analysis in flight for each photograph.
- `declined?: string`: explained in D4.

Its actions:

- `begin(path, run, crop)`
- `end(path, run)`: removes the entry only if it still names that `run`.
- `stop(path)`: calls `cancelSuggest`, then removes the entry.
- `stopAll()`
- `decline(path)` and `clearDeclined()`

It registers as a file owner:

- `forget(path)` → `stop(path)`
- `forgetAll()` → `stopAll()`

That registration is the whole of "closing a photograph stops its analysis", and it needs nothing added to `closeFile`.

**Keyed by path, not identity.** The answer is applied to a stack, and stacks are keyed by path. Two open files with
the same bytes are two stacks, and each gets its own analysis. Slice 1 keyed the backend's table by run name for the
same reason.

**A store rather than component state.** The spinner row (`EnhancementList`) and the trigger (`useAutopilot`) are in
different components, and the file-owner registry is a module-level hook. The reference kept `analysingPath` in
component state and could then only show one photograph's spinner. Here several analyses can be in flight at once.

`useAnalysing(path)` answers whether that photograph has an entry, and is what the list reads.

### D2. The trigger is `useAutopilot()`, mounted once in `Sidebar`

Its effect reads these values and decides from them:

- the current file's `path` and `identity`
- `autopilot`
- `hasStack`, from `enhancements.has(path)`
- the crop in force (`useImageCrop(identity)`)
- the current file's entry in `analysing`
- `declined`

In order:

1. **Autopilot off.** Call `stopAll()` if anything is in flight, then return. This is the switch-off stop.
2. **Nothing to analyse.** No file, or no identity (the bytes could not be read, and nothing can be analysed or
   enhanced): return.
3. **In flight.** If an entry exists for this path at the same crop reference, return: the analysis is already
   running. If the crop reference differs, call `stop(path)`. That is the reframe stop, and the effect goes on to
   step 4.
4. **Has a stack.** If `hasStack`, return. This comes after step 3 so that a stack the user added to by hand during
   the analysis does not exempt it from the reframe stop, which would let an answer measured on a framing that is
   gone land in the stack and record faces at a crop no longer in force. A photograph stopped this way has had a
   list, so it is not asked about again.
5. **Declined.** If `declined === path`, return (D4).
6. **Analyse.** Call `analyse(file, crop)` (D3).

**Nothing is stopped from a cleanup.** Every re-run of this effect happens because a dependency changed, and most of
those changes must not stop anything: writing the stack, moving off, or the analysing entry itself appearing. The
reference's comment records the failure mode this avoids, where the stack write re-runs the effect, the cleanup
cancels the analysis in flight, and the spinner is left up forever. The three real stops are D1's `forget`, step 1
and step 3, each an explicit call.

*Alternative:* one effect per photograph, mounted in the drawer's items. Rejected: only the current photograph
triggers, so a per-item effect would be N subscriptions deciding one thing.

### D3. `analyse` is a plain async function in `hooks/useAutopilot.ts`, reading stores at each step

It reads the stores through `getState()` at the moment each step needs them, not through values captured at render.
Each step below either continues or ends the attempt; any "discard" ends it without writing anything.

1. **Ask.**
   - Compute `families = familiesWhere((family) => !autopilotExcluded.includes(family))` and read `processor`.
   - Call `suggest(identity, processor, families, crop)`, then `begin(path, run, crop)`.
2. **Await `done`.**
   - On rejection with `kind: "stopped"`: end silently.
   - On any other rejection: `console.error`, show the `errors.autopilotFailed` toast, and apply D4's decline.
3. **Check the answer is still wanted.** This is `ours()`: the path's entry still names `run`. If not, discard. A
   close, a switch-off or a reframe removed the entry, even if the backend's answer beat the stop across the
   boundary.
4. **Check face recovery** (D6), only if it is among the suggestions.
   - Call `detectFaces(identity, processor, crop)`.
   - On answer: call `ours()` again, then `setFaces(identity, crop, faces)`. If `noFaceToRestore(faces)`, drop the
     face-recovery suggestion.
   - On rejection: keep the suggestion and record nothing. The run path then detects when it runs, and reports its
     own failure through `gui-faces`'s existing rule.
5. **Build.**
   - `await catalogue()` (the `once` promise, already settled in practice).
   - Read `models`.
   - Map each suggestion through `suggestedOperation` (D5). A family with no catalogue model builds nothing and is
     dropped.
6. **Apply.** Check `ours()` again, then call `addEnhancements(path, operations)` (D7) and `end(path, run)`, in that
   order and in the same tick.

**The order in step 6 is load-bearing.** Ending first would leave one render in which the photograph has neither a
stack nor an analysis, and D2's step 6 would start a second one. Writing the stack first, synchronously, means the
effect's next run sees `hasStack`. The failure path in step 2 calls `end` too; D4 is what stops that from
re-triggering.

**`end` runs on every exit path**, including discards, through a `finally`. It is a no-op where a stop already
removed the entry, because it matches on `run`.

**The spinner covers step 4.** The entry is still present during the follow-up detection, so the row stays up. The
detection's `enhance:progress` reports carry a run name `useEnhancementRun` never minted, so the canvas indicator
ignores them. That is `gui-faces`' "not reported on that indicator" requirement, and it holds by construction.

### D4. A failed analysis is retried the next time its photograph becomes current, via `declined`

Once a failure has ended its entry, every condition of the trigger still holds, so the effect would immediately ask
again, fail again, and loop.

`declined` is a single path: the current photograph, set only if the failed analysis was for the photograph that is
current at the moment it failed. The effect clears it whenever the current path changes or Autopilot is switched on.

A failure for a photograph the user had moved off sets nothing, because coming back to it is already "becoming
current". One value is enough, because only one photograph is current, and it cannot leak.

*Alternative:* a `failed: Set<path>` cleared on close. Rejected: the spec retries on the next visit, not only after a
reopen, and a set needs its own pruning rule.

### D5. `suggestedOperation` in `lib/enhancements.ts`

`suggestedOperation(suggestion, entry, stored, file)` calls `newOperation(suggestion.family, entry, stored, file)`.
For an upscale it then replaces `scale` with `suggestion.scale`.

**What a suggestion becomes is by construction what the menu adds**: default model, starting bias of 50%, starting
strength of 100%, and a face recovery with no faces. A future change to the menu's defaults moves both. The upscale's
scale is the one exception, and the spec names it. The two scales agree on an uncropped photograph, and the analysis's
is correct on a cropped one.

The switch over `Suggestion` is exhaustive over its union, so a family added to the wire fails the typecheck here.

### D6. The size rule: `noFaceToRestore(faces)` in `lib/faces.ts`

The check is `faces.every((face) => faceArea(face) > RESTORED_FACE_AREA)`. It sits beside `decidedSkips`, reuses the
same private `faceArea` and the same constant, so "too large to restore" has one definition.

**An empty detection counts as "none to restore"** (`every` over nothing is true). The analysis said there were faces,
so this only happens if the pixels changed underneath, and a face recovery with nothing to restore is exactly the row
the rule exists to keep out.

**The faces are recorded whether or not the suggestion survives.**

- Where it survives: the first run of the new stack finds `useImageFaces` hitting, starts at `WHOLE_BAR`, and never
  detects.
- Where it is dropped: a face recovery the user later adds at that framing is served the same way. The large faces
  are already skipped by `setFaces`' own size rule, which is what the user would have got anyway.

`stores/faces.ts`' comment calling the run path "the faces' only writer" becomes "the run path and Autopilot". Both
write the same value through the same `setFaces`.

**The recorded crop is the reference `begin` captured.** `ours()` in step 4 guarantees no reframe happened since
(D2's step 3 would have removed the entry, whether or not the photograph has a stack by then), so that reference
is still the crop store's current one, and `useImageFaces` matches it.

### D7. A batch add on the enhancement store

`addEnhancements(path, operations)` does four things in **one `set`**:

1. drops every operation whose family is already in the stack
2. appends the rest
3. sorts by `pipelineOrder`
4. writes the key **even when nothing is added**

That last point is what makes an empty answer mean "analysed" (`has(path)` becomes true).

One write rather than N `addEnhancement` calls, because `useEnhancementRun` keys on the stack array. N writes would
start a run on the first suggestion, cancel it on the second, and so on. The existing `addEnhancement` is left alone
for the menu.

### D8. The analysing row

`EnhancementList` renders the row when `useAnalysing(file.path)` is true, in place of the rows. It returns early
before the `stack.length === 0` check, so a photograph with nothing yet still shows it.

Design screen 04 gives these metrics:

- padding `12px 16px 12px 40px`, with a 16px gap
- a 22px ring with a 2px `--border` track, whose top and right edges are `--success-bright` and `--activity`
- the default `animate-spin`
- `sidebar.analysing` in 14px bold `--primary`

It carries `role="status"`, so the announcement reaches a screen reader.

### D9. Model sessions and execution providers

Nothing new is owned. The analysis holds the detector's lease only inside `opai`'s `execute`, and slice 1's D7 already
covers its release on a stop. The follow-up `detectFaces` cannot be stopped (faces D11). It is a run-store hit that
opens no session, and if the store was evicted it is one bounded detection whose answer is discarded by `ours()`.
`processor` is passed through as the run path passes it.

### Crate division

| Crate | What it gets in this slice |
|---|---|
| `opai` | Nothing. |
| `gui` | Frontend only: the trigger, the in-flight store, the batch add, the size check and the row. Every one of these is a question about a window: which photograph is current, and what the sidebar shows. |
| `cli` | Nothing. |

### rust-sak

Nothing is used, and nothing is missing. No infrastructure is involved.

## Risks / Trade-offs

- **[Risk] A stop and an answer cross.** An answer can arrive after the user closed the photograph, switched off, or
  reframed. → `ours()` is checked after every await (D3), and the backend already answers `stopped` for a token
  cancelled before its answer (slice 1 D6 step 7). Either check alone is enough.
- **[Risk] The follow-up detection is not a cache hit** (the run store was evicted or is disabled). → It runs once
  more, under the spinner. Correctness does not depend on the hit, only the cost does.
- **[Trade-off] The answer applies to a photograph the user moved off.** This is the reference's behaviour and the
  explored decision. A user browsing quickly through a folder accumulates analyses in flight, one per photograph
  visited, and all of them go on. → Each is bounded, and closing or switching off stops them. A cap on concurrent
  analyses is left out until a measurement asks for one.
- **[Trade-off] The analysing row hides a list the user added to by hand during the analysis.** This matches design
  screen 04 and the reference. The row lasts for one analysis, and the hand-added entry survives the merge (D7).

## Validation

Vitest, with `ipc/autopilot`, `ipc/faces` and `ipc/catalogue` mocked, covers every trigger, stop and apply scenario
in the `gui-autopilot` delta.

A manual check on macOS, on CPU and on CoreML, covers the paths that need models:

- open `fixtures/test.dat` with Autopilot on
- see the analysing row, then the suggestions
- confirm the face recovery's run reports no detection stage
- close a photograph mid-analysis
- switch Autopilot off mid-analysis

This is also the check slice 1 deferred to this slice: the follow-up detection being served from the run store. CUDA
and TensorRT on Windows and Linux are not tested. The slice adds nothing provider-specific.

## Slices

| # | Change | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `add-gui-autopilot-1-seam` | The `suggest` and `cancel_suggest` commands, the suggestion wire, and `frontend/ipc/autopilot.ts`. Nothing a user sees. | nothing | done |
| 2 | `add-gui-autopilot-2-run` | The trigger, the analysing row, applying the suggestions in one batch, face recovery dropped when every face is larger than 512x512, stopping on close, switch-off and reframe, and the spec changes to `gui-autopilot`, `gui-enhance`, `gui-faces` and `gui-settings`. | slice 1 | **current** |

This slice finishes the series.
