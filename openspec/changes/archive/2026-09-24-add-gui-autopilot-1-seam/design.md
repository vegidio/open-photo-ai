## Context

`opai` already analyses a picture: `Opai::suggest(&Picture, Option<&[Family]>, Option<ExecuteOptions>)` returns
`Suggestions { suggested, incomplete }`, narrowed to the families asked for, in `Family::ALL` order, and fails with
`InferenceError::Cancelled` or `Shutdown` when its token is cancelled. Its face signal runs
`Detection::newyork(Fp32)` through `execute`, which is the same analysis at the same precision that `gui`'s
`detect_faces` runs (`faces.rs`, `DETECTION_PRECISION`). So when both run on the same `Picture` identity, the second
is served from the run store.

`gui` already has everything the seam needs except the command itself:

- `Opened::resolve` admits an identity.
- `load_framed(path, crop)` decodes an image and applies a crop off the async runtime.
- `Processor` is the wire spelling of an execution provider.
- `Setup::peek` hands out an `Opai` clone once start-up has finished.
- `faces.rs` shows the shape to copy: a seam trait so the order of events can be tested without a model, a
  `Request` struct, and a `*_with` function the command delegates to.

See proposal.md for why this is needed now.

## Goals / Non-Goals

**Goals:**

- One command that answers a photograph's suggestions at its framing, and one that stops it by name.
- Analysing the framed picture, so the face detection it makes is the one `detect_faces` will be served.
- A stop that races ahead of its own analysis still lands.

**Non-Goals:**

- Anything a user can see: the trigger, the spinner row, applying suggestions, the 512x512 face rule. All of it is
  slice 2.
- Progress reporting (D5).
- Changing `opai`.

## Decisions

### D1. Its own module, `gui/src/autopilot.rs`, not part of `enhance` or `faces`

It holds no result, touches neither `Runs` nor the `opai://` scheme, and runs no chain. It shares the order of
operations with `faces.rs` (resolve, decode and frame, run), and that order is shared as code through `load_framed`,
the same way `faces.rs` shares it with `enhance/run.rs`.

*Alternative:* a second arm on `detect_faces`. Rejected, because the two answer different questions and fail
differently: a detection cannot be stopped (faces D11), and an analysis must be.

### D2. Analyse the framed picture

The command takes `crop: Option<Crop>` exactly as `detect_faces` does, and calls `load_framed(path, crop)`. That makes
the picture's identity the one `cropped()` derives. Because the face signal and `detect_faces` both run
`Detection::newyork(Fp32)` with the cache on, slice 2's follow-up `detect_faces` call, which it needs in order to size
the faces, is a run-store hit rather than a second inference.

*Alternative:* analyse the file as it is on disk, as the reference does. On a photograph nobody has cropped this is
the same picture. On a cropped one it would suggest an upscale from the wrong pixel count, and find faces in
coordinates the window cannot draw or size against. It would also leave the follow-up detection with nothing cached.

### D3. Cancellation is a table of runs keyed by the window's name for each, not the enhancement slot

Managed state: `Analyses { held: Mutex<Held> }`, where `Held` holds:

- `running: HashMap<String, CancellationToken>`
- `stopped`: a small bounded set of names whose stop arrived before their analysis

It is a `std::sync::Mutex`, and no guard is held across an `await`. That is the same reasoning as `Runs` and `Opened`,
and poisoning is treated as usable for the same reason.

- `suggest(run, ..)` registers its token under `run` before it decodes anything. If `run` is already in `stopped`, it
  removes the name and answers `Stopped` at once. That is what makes "the stop arrives first" read and fetch nothing.
- `cancel_suggest(run)` cancels and removes the token if `run` is running. Otherwise it records `run` in `stopped`.
  It answers nothing and fails at nothing, like `cancel_enhance`.
- The analysis removes its own entry when it finishes, whatever the outcome, through a drop guard so that a panic
  cannot leave a token behind.
- `stopped` is bounded (a handful of names, oldest evicted). A stop for an analysis that already finished lands there
  and is never matched, so without a bound the set would grow for the whole session.

**Why not the slot:** `Runs` holds exactly one run and supersedes it on the next write. Analyses must not supersede
one another. Slice 2 lets an analysis for a photograph the user moved off go on and apply to that photograph, so
several can be in flight. They also must not disturb the enhancement run in flight.

**Why keyed by run name rather than by image identity:** the identity is a content identity, so two open files with
the same bytes share one. Stopping "the analysis of this identity" when one of them is closed would stop the other's.
The run name is the window's own and is unique per request, which is the same arrangement `cancel_enhance` uses.

### D4. The wire

The answer is `Vec<SuggestionWire>`, where `SuggestionWire` is `gui`'s own serialize-only type tagged by `family`
with `rename_all = "snake_case"`, matching how `Requested` spells operations:

```text
[{ "family": "colorization" }, { "family": "upscale", "scale": 4.0 }]
```

`Suggestion` has no `Serialize`, and giving it one in `opai` would fix a wire shape in the library for one front end.
The conversion is a match with one arm per `Suggestion` arm, so a new arm fails to compile here rather than being
dropped silently.

The request's `families: Vec<Family>` uses `Family`'s existing `Deserialize`, and is passed through as `Some(&families)`.
The interface always names a set, because the settings store always has one, so the "unset = every family" form of
`opai`'s API is not exposed. An empty vec is a real answer ("check nothing"), not a missing one.

`incomplete` is **not** on the wire. The user's decision for Autopilot is "apply what came back, and log the rest",
and `opai` already records the unreadable signal and its reason at a verbosity that reaches the log file
(`autopilot`: "An analysis says what it suggested"). The command adds one `warn!` naming the run, so the log line can
be tied to the photograph. A field no window reads would be surface with no consumer.

Errors are `SuggestError`, tagged by `kind` in camelCase like `DetectError`, with these arms: `notReady`,
`unknownSource`, `unreadableSource`, `stopped`, and `analyse { message }`.

- `InferenceError::Cancelled` and `InferenceError::Shutdown` both map to `stopped`. Either way the question was
  withdrawn, and slice 2 must stay silent for both.
- Everything else maps to `analyse`, carrying the library's own sentence untranslated.

### D5. No progress reporting

The only step of an analysis that can take noticeable time is a first-use transfer of the face detector. Reporting it
on `enhance:progress` would drive the canvas chip, which names an enhancement being run. That would show "Face
Recovery" over a photograph the user never asked to restore, while slice 2's spinner row already says the photograph
is being analysed. So `on_progress` is left at `None`.

*Trade-off:* a first analysis on a fresh install waits on the detector download with only a spinner. The detector is
small, and the same transfer is reported normally the first time a face recovery is added by hand.

### D6. Order of events, and where the token is checked

```text
suggest(run, source, processor, crop, families)
  1. Setup::peek                  --> notReady
  2. register run (or: stop arrived first) --> stopped, nothing read
  3. Opened::resolve              --> unknownSource, nothing read
  4. load_framed                  --> unreadableSource
  5. token cancelled?             --> stopped  (a stop during the decode)
  6. Opai::suggest(.., Some(&families), ExecuteOptions { provider, cancel, ..Default })
  7. token cancelled?             --> stopped  (even if an answer came back)
  8. deregister (drop guard)
```

Step 7 is there because cancellation is cooperative. An analysis whose every signal was already in the run store can
answer `Ok` without ever looking at its token. The window withdrew the question either way, and an answer to a
withdrawn question must not be applied.

The token is `ExecuteOptions::cancel`, which `opai` already observes before and after the pixel passes and throughout
the detection.

### D7. Model sessions: nothing new is owned here

The command never holds a session. `opai`'s `execute` acquires the detector's lease for the length of the detection
and releases it when the call returns, including a cancelled return. So stopping an analysis releases its hold on the
detector at the same point a stopped enhancement run does. The only thing `gui` owns is the `CancellationToken`, and
the drop guard in D3 bounds its lifetime to the command's.

### Crate division

| Crate | What it gets in this slice |
|---|---|
| `opai` | Nothing. It already owns the analysis, the narrowing, the ordering, the incompleteness record and cancellation. |
| `gui` | The command, the cancellation table and the wire type. These concern a window: which open photograph it analyses, and under what name it withdraws one. |
| `cli` | Nothing. |

### rust-sak

No `rust-sak` feature is used directly. The transfer (`fetch`), the run store (`memo`) and the framed identity
(`crypto`) are all reached through `opai` and `gui/src/images/crop.rs` as they are today. Nothing is missing.
`tokio_util::sync::CancellationToken` is already a dependency through `opai`'s public `ExecuteOptions`, so no crate is
added.

## Risks / Trade-offs

- **[Risk] A stop for a run the backend never sees leaves a name in `stopped`.** The bounded set (D3) caps it. An
  evicted early stop can only matter if a stop overtakes its own analysis by more than the bound's worth of other
  stops. The window never produces that, because it stops one analysis per closed photograph.
- **[Risk] Divergent framings share nothing.** An analysis at one crop does not pre-fill detection at another. This
  is the correct behaviour (different pixels), and on the path slice 2 builds, the analysis and the follow-up
  detection are made at the same framing.
- **[Trade-off] `incomplete` stays out of the answer.** If a later slice wants to tell the user an analysis was
  incomplete, it adds the field then. Nothing in this slice has to be undone for that.

## Validation

Tested with unit tests behind the seam trait, needing no model. The follow-up detection being served from the run
store is checked by hand on macOS (CPU and CoreML) in slice 2's manual check, where there is a window to drive. The
command itself adds no execution-provider-specific behaviour: `processor` is passed through as `detect_faces` passes
it. CUDA and TensorRT on Windows and Linux are not tested by this slice.

## Slices

| # | Change | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `add-gui-autopilot-1-seam` | The `suggest` and `cancel_suggest` commands, the suggestion wire, and `frontend/ipc/autopilot.ts`. Nothing a user sees. | nothing | **current** |
| 2 | `add-gui-autopilot-2-run` | The trigger, the spinner row, applying the suggestions, face recovery dropped when every face is larger than 512x512, cancelling on close, and the spec changes to `gui-enhance`, `gui-faces` and `gui-settings`. | slice 1 | pending |

The trigger is: the current image, Autopilot on, no stack yet, and the families the settings allow.

Applying the suggestions uses a single batch add in pipeline order, uses the models the settings name, and skips
families already in the stack.

Slice 2 also has to amend `gui-faces`'s "Faces are found only for a photograph whose enhancements need them". Its
line "SHALL NOT detect faces in a photograph merely because it is open, has been selected, or is being looked at" is
exactly what an Autopilot analysis with face recovery allowed does. Similarly, the `gui-settings` sentence "The
application does not run an Autopilot analysis yet" has to be replaced.

Slice 2 carries forward these decisions made while exploring this feature:

- Analysis runs once per photograph, only when it is current, as the reference does.
- Turning the switch on over a current photograph that has no stack analyses it.
- An incomplete analysis is applied and logged only.
- An outright failure shows an error toast, writes no stack, and retries the next time that photograph becomes
  current.
- Closing a photograph mid-analysis discards the answer and stops the analysis.
- The 512x512 rule is in the frontend and uses `RESTORED_FACE_AREA`, applied only to Autopilot's suggestion.
