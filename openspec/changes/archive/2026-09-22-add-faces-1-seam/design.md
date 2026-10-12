## Context

See proposal.md — Why for the motivation. What shapes the approach is that almost everything this
slice needs already exists on one side of the gap or the other, and the design is mostly about which
side each piece belongs to.

**What `opai` already publishes.** `Detection::newyork(FloatPrecision)` builds a `Subject::Analysis`;
`Opai::execute(&Picture, &Analysis, Option<ExecuteOptions>)` runs it and answers `Executed<Faces>`,
serving a repeat from the run store **before** transferring or opening anything. `Face` is a `Copy`,
`Eq`, `Hash`, `Serialize`, `Deserialize` value of thirteen coordinates and a confidence, quantized to
a hundredth of a pixel on acceptance. `Faces::write_cache_signature` folds an order-sensitive
signature of the bounding boxes into a face-recovery operation's cache tag.
`VariantEntry::build`'s `FaceRecovery` arm already routes per model: Athens is handed
`values.fidelity`, Santorini is not, and a Santorini operation carries `fidelity: None`.

**What `gui` already has.** `images/crop.rs` holds `Crop` and `cropped`, whose `identity_of` folds
`millidegrees`, the two flips and the rectangle into a framed picture's identity — so a framing is a
value and the same framing twice is one identity. `enhance/run.rs` applies the framing to the loaded
picture *before* `process` sees it, which is what makes every cache key downstream fold from the
framed pixels. `enhance/progress.rs` holds `RunProgress` and the `enhance:progress` event, thinned to
about a hundred reports per run and carrying `run`, `family`, `stage`, `chainFraction` and
`installFraction`. `enhance/operation.rs` holds `Requested`, with one arm.

**What the frontend already has.** `lib/enhancements.ts` holds all seven families in pipeline order,
`RUNNABLE` (one member) with a type-level check that the wire and the menu agree, and `modelChoice`'s
positional HD/SD tier rule. `hooks/useEnhancementRun.ts` is the one caller of `enhance`, holds the
current run's name in a ref so one progress subscription can filter by it, and already re-runs on a
change of stack, image, framing or processor. `stores/files.ts` publishes `registerFileOwner`, which
`stores/crop.ts` and `stores/transform.ts` use to be told when a photograph closes.

**The two caching properties this change was asked for are consequences of the above rather than work
here**, and that is worth stating because it is the reason this slice is thirteen tasks and not twenty:

```
   a face-recovery cache per selection          a New York cache per framing

   Faces -> cache_signature                     Crop -> identity_of
   "12.34,56.78,90.12,34.56;..."                xxh3(source|crop|mdeg|flips|l,t,w,h)
        |                                            |
        v                                            v
   folded into the operation's cache tag        the framed Picture's identity
        |                                            |
        v                                            v
   3 faces  -> 8 distinct entries               90deg -> 50deg -> 90deg
   (one per subset, order-sensitive)            is one identity, twice
```

## Goals / Non-Goals

**Goals:**

- One boundary at which a face crosses from the detector to the recovery model, so that when a user
  can choose among them (slice 2) the choice has one place to be applied.
- A detection that cannot be skipped by a run that needs it, structurally rather than by convention.
- The faces held against the framing they were found at, so a stale answer is a miss rather than a
  wrong set of boxes.
- No second copy of the framing, the family list, the model vocabulary or the cache key.

**Non-Goals:**

- **Choosing among the faces.** No toggling, no dialog, no boxes, and no deselection anywhere —
  slice 2. A store field for a selection nothing can write would be surface describing work in
  progress.
- **Anything in `opai`.** If a task here reaches for a file under `crates/opai/`, something has been
  mis-scoped.
- **A fidelity control.** Fixed at maximum, and deliberately not on the wire — D7.
- **Autopilot suggesting face recovery.** Autopilot is unwired in this window and is its own change.
- **Export taking faces.** There is no export queue yet.

## Slices

Face recovery in the window, in order. Only the current slice is proposed; the rest are proposed when
reached.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-faces-1-seam` | The `face_recovery` wire arm; the `detect_faces` command; `ipc/faces.ts` and `stores/faces.ts`; Face Recovery in the add menu; the detect-then-enhance run path; the `2 Faces` info line; detection reported on the existing indicator. Restores **every** face found | - | **current** |
| 2 | `add-faces-2-select` | Screen 13's Faces block in the options panel; screen 13b's Select faces dialog with its boxes, footer hint and Apply; the deselection; the `1/2 Faces` info line; the re-run on a changed selection | 1 | pending |

**This is a new series.** The `add-gui-enhance-1-seam` .. `add-gui-enhance-2-sidebar` series closed on
its second slice, which named this work among what it left: *"the six other families (each arrives
with its own wire variant, its own options panel, and for face recovery a detection run the window has
no wiring for)"*.

**The seam lands ahead of the screen**, which is this project's established order and is what lets
slice 2 ship a dialog whose Apply is a single store write against a run path exercised since slice 1.

### Decisions settled for the series, acted on where noted

Settled with the repository owner during this slice's exploration. Repeated here so slice 2 inherits
them from an archived document rather than from a conversation.

- **Face boxes are drawn in the Select faces dialog only**, not over the Original pane. Screen 13's
  note says otherwise, but no screen in the design file renders them and the reference draws them only
  in its dialog. *Slice 2.*
- **The detector runs at FP32 always**, not at the recovery model's tier. *Acted on: D2.*
- **A deselection is stored as the `Face` value, not as an index into the detected array.** A
  re-detection then needs no reset rule at all: a face that comes back keeps its state, one that the
  framing removed matches nothing, one newly found is on. *Slice 2, and the reason D9 keys the store
  the way it does.*
- **Detection is reported under the Face Recovery name**, where the reference is silent. *Acted on:
  D8.*
- **Fidelity is fixed at 1.0 and is not on the wire.** *Acted on: D7.*
- **The Select faces dialog is fitted to the photograph's aspect, and Apply commits while the close
  box and Escape discard** — the mockup's own 634px is 628px square plus chrome, so it already agrees
  with the reference's aspect fit, and an explicit Apply is what makes dismissal a discard. *Slice 2.*

## Decisions

### D1 — Detection is a command of its own, driven from the window

`detect_faces(source, processor, crop, run)` in a new `gui/src/faces.rs`, answering the faces. It is
**not** folded into `enhance`.

The window has to hold the faces whatever else is true: slice 2 draws a box per face and reports a
count per photograph, and neither can be done with a set of faces that only ever existed inside one
`enhance` call. Once they are on this side, handing them back with the operation is free and is what
makes the selection expressible at all.

*Alternative considered:* `enhance` detects internally when the chain carries a face recovery, and the
window never sees a face. It is fewer round trips and is wrong for the feature: the whole point of
screens 13 and 13b is that the window decides which faces the run is about, and a window that cannot
name a face cannot deselect one. It would also put two model runs behind one cancellation token and
one progress range, which `data-inference` deliberately keeps apart from the chain.

The command lives in `faces.rs` rather than in `enhance/`: it runs no chain, holds no result, and
touches neither `Runs` nor the `opai://` scheme. Its one shared dependency is `images::{Crop, Opened,
framed}`, which `enhance/run.rs` also takes.

### D2 — New York at FP32, always

The detector's precision is a constant in `faces.rs`, not a function of the recovery model's tier.

`face-detection` already requires that the two precisions agree: *"the same faces are found in the
same order, differing at most in sub-pixel placement and comparing equal wherever that difference
falls below a hundredth of a pixel"* — and a `Face` is quantized to a hundredth of a pixel on
acceptance, so in the overwhelming majority of cases the two are the *same value*. The reference's own
comment records the measurement behind that: 25 variants of the sample image, identical results.

So the tier buys nothing and costs two things. On disk: a second detector build, transferred the first
time a user tries the other tier. In the store: two entries per photograph per framing, where one
would serve both — and since the entry is what makes a return to a previous framing free, halving the
entries doubles how often that lands.

*Alternative considered:* the reference's rule, tying the detector to the recovery model. Its stated
reason is that the precision *"selects the model file, not because the two disagree about what they
find"* — which is a description of the mechanism, not an argument for it, and the same comment then
records the measurement that makes the distinction empty.

**Recorded as a parity divergence** in proposal.md. What it would take to revisit: a photograph where
the two builds disagree about how many faces there are.

### D3 — The framing is applied before detection, exactly as it is before a chain

`detect_faces` resolves the identity to a path, decodes, applies the framing through `framed` - which
is `cropped` on a blocking thread, and is extracted from `enhance/run.rs` so the two share the call
rather than each making it - and hands the framed picture to `execute`. That is `enhance/run.rs`'s own order, and
it is reused rather than re-derived because every property this change needs follows from it:

```
   load(path) -> Picture{ identity: hash(bytes) }
        |
   framed(picture, crop)            <- the SAME call enhance/run.rs makes
        |
   Picture{ identity: xxh3(source|DISCRIMINATOR|mdeg|flips|l|t|w|h) }
        |
   execute(picture, Detection::newyork(Fp32))
        |
   run store keyed on that identity + the operation's cache tag
```

- The faces come back in the framed photograph's pixels, which is what the window draws on and what
  the recovery model is applied to — `gui-faces` requires exactly this, and it needs no code because
  the detector was handed the framed picture in the first place.
- A framing returned to is one identity returned to, so the store serves it. 90° → 50° → 90° is two
  identities and three requests, and the third is served.
- `changes_nothing` means an unframed photograph is the source `Picture` untouched, identity and all —
  so a photograph that is never framed has exactly one detection entry for its whole life.

**No new URL grammar, no new identity derivation and no second copy of the framing.** If this slice
found itself writing a hash, that would be the signal it had gone wrong.

### D4 — The run path owns the detection, so a run cannot start without faces

`useEnhancementRun`'s effect becomes: if the stack carries a face recovery and the faces for
`(source, crop)` are not known, detect first, then enhance.

```
   useEnhancementRun(file, crop)
        |
        +-- stack has no face recovery ------------------> enhance(...)
        |
        +-- stack has one
              |
              +-- faces known for (source, crop) --------> enhance(... with faces)
              |
              +-- not known
                    |
                    detectFaces(source, processor, crop)
                    |        mine.current = its run name
                    v
                    setFaces(source, crop, faces)
                    |        mine.current = the enhance run's name
                    v
                    enhance(... with faces)
```

`gui-enhance` requires that a chain carrying a face recovery is not run before its faces are known,
and putting the detection **in** the run path is what makes that structural rather than a guard two
effects have to honour. A run started early would restore nothing, would be stored under the empty
selection's signature, and would have to be run again the moment the faces landed — a visible flash of
an unrestored result and a wasted chain.

**One ref, two run names.** `mine.current` already exists so that one progress subscription can filter
reports by run; it is set to the detection's name and then to the enhancement's. Nothing else changes
about the listener, and the indicator is continuous because both report the same label (D8).

**A second ref holds the detection in flight**, because the store cannot: its entry appears when the
answer *lands*, so between asking and landing the question "are this photograph's faces known?" reads
no, and every re-render that reaches the effect — a second enhancement added, a model changed, the
processor changed — would ask for the same faces again. Two detectors would then run over the same
pixels at once, and on a first use transfer the model twice. So the effect asks its question of the
store *and* of the detection under way, keyed the same way the store is: the identity and the framing.
A pass that finds one in flight picks its run name back up and draws it, which is what keeps the chip
lit across a change that asked nothing.

**A late answer is written only while the photograph is still open.** The last thing the detection does
is check the file store, because closing a photograph has already told every owner to forget it (D9) —
and a write after that would put an entry back for a photograph nobody can see and nothing will close
again. Moving *off* a photograph is not closing it, and that answer is still written: see Risks.

*Alternative considered:* the reference's arrangement — a `useSyncFaces` callback invoked from
"enhancement added" and from "crop applied", writing the store, with the run path reading whatever is
there. It is parity, and it is two independent effects racing to decide whether a run may start; the
reference answers the race by letting the run proceed with whatever the store holds, which is the
early-run problem above. Here the store is a **cache of the run path's own step**, and the only thing
that writes it is the thing that needs it.

### D5 — The stack holds what the user chose; the faces are resolved at run time

`stores/enhancements.ts` keeps holding `Operation` values, and a face-recovery entry in it carries a
codename and a precision and **no faces**. The faces are added by `useEnhancementRun` on the way to
`enhance`.

The stack is the user's choice and is keyed per photograph; the faces are a property of the pixels and
change when the framing does. Putting them in the stack would mean every framing change rewrote every
photograph's stack, and a stack entry would stop being comparable to the one the menu created. The
reference reaches the same arrangement from the other direction — its `getEnabledFaces` runs inside
the image pipeline, not inside the store.

This also means the wire type and the store type stop being the same shape for this family. `Operation`
gains `{ family: "face_recovery"; codename; precision; faces: Face[] }`, and what the store holds is
that with `faces: []`. Spelled as one type with the faces filled in at the boundary rather than two
types with a conversion, which is the duplication slice 1 of the enhance series removed from the wire.

### D6 — The faces cross the wire in `opai`'s own shape, snake_case included

```ts
type Face = {
    bounding_box: { min: { x: number; y: number }; max: { x: number; y: number } };
    landmarks: [Point, Point, Point, Point, Point];   // eyes, nose, mouth corners
    confidence: number;
};
```

`Face`'s `Deserialize` accepts exactly what its `Serialize` produces, so a face the window hands back
is byte-for-byte the face it was given — which is what makes `Faces`' cache signature reliable across
the round trip, and what will make a deselection stored by value (slice 2) compare correctly.

This is the one place in this window's wire that is not camelCase, because `Face` is `opai`'s type and
this crate does not own its shape. The alternative is a `gui`-side mirror with a rename and a
conversion in both directions — a second spelling of thirteen coordinates whose only job is casing,
and a place for a landmark order to get transposed silently. The casing is named in `ipc/faces.ts`'s
own documentation so the next reader does not "fix" it.

The five landmarks are typed as a fixed-length tuple, matching `Face::LANDMARKS` and the reference's
own conclusion that an array rather than a slice is what stops a consumer reordering the reference
points.

### D7 — The fidelity is fixed in `resolve`, and is not on the wire

`Requested::FaceRecovery` carries `codename`, `precision` and `faces`. `resolve` builds
`ParameterValues::new().with_faces(Faces::new(faces)).with_fidelity(Fidelity::MAXIMUM)` and hands it
to the same `VariantEntry::published(..).build(..)` call the upscale arm uses.

**No per-model branch here**, because `build`'s own `FaceRecovery` arm is the branch: Athens reads
`values.fidelity`, Santorini does not, and a Santorini operation carries `fidelity: None`. So handing
a fidelity to Santorini is not a value being ignored — it is a value that model's constructor has no
parameter for. Its cache tag is unaffected, which is what keeps `face-recovery`'s "one selection twice
through a model that accepts no fidelity is one result" true. This is the same property that makes
`operation.rs` name no model: the library resolves the codename against the catalogue the chooser was
built from.

**`with_fidelity` is written explicitly although `ParameterValues::new()` already defaults it to
`MAXIMUM`.** One line, and it pins the *window's* intent to fix the value at 1.0 rather than
inheriting a library default that is free to change. `Fidelity::MAX` is 1.0 and `Fidelity::MAXIMUM` is
that value.

**Not on the wire**, because a field the window always sets to one value describes a control that does
not exist. When a fidelity control is wanted it is added to this arm then, and the default it arrives
at is the value this slice fixes.

### D8 — Detection reports under `Family::Detection`, and the frontend names it Face Recovery

The command emits `RunProgress` on the existing `enhance:progress` event, carrying the detection run's
own name and `family: Family::Detection`. The **frontend** maps that family to
`preview.progress.faceRecovery` when it draws the chip.

Reusing the event rather than adding one: the window draws one indicator, every report already names
its run, and `useEnhancementRun` already filters on that. A second event would be a second
subscription and a second thing to unregister for a chip that can only ever show one thing.

**Honest in Rust, named in TypeScript.** The wire field means "which family this operation belongs
to", and the operation is a detection; saying `face_recovery` there would make the `operation`
diagnostic (`New York (FP32)`) disagree with its own family. What the *user* should read is the
enhancement they asked for, and which enhancements this application presents and what it calls them is
already documented in `lib/enhancements.ts` as a presentation decision that belongs in a front end —
the same file that explains why `detection` is named nowhere in `ENHANCEMENTS`. So the mapping is one
entry beside that list, not a lie on the wire.

`Stage::Installing` and `installFraction` carry the detector's transfer unchanged, which is the whole
reason this decision exists: on a first run the window otherwise sits still through a download.

*Alternative considered:* a `preview.progress.detection` key in all thirteen locales. It names a model
the user never chose and that the settings dialog deliberately never lists.

### D9 — `stores/faces.ts` is keyed by identity and holds the framing it was found at

```ts
faces: Map<string, { crop?: CropInfo; faces: Face[] }>
```

**Keyed by identity**, as `stores/crop.ts` and `stores/transform.ts` are and for their stated reason:
identity is the key everything about a photograph's pixels hangs off, and a file whose bytes could not
be read has no faces to find in the first place. Two copies of one photograph in two folders are one
thing to look at and share one answer.

**The framing travels in the value rather than in the key**, which is `useEnhancementRun`'s own
arrangement for the enhanced result and is there for the same reason: a render at a different framing
simply does not match, so there is no window in which one framing's boxes are drawn over another's.
The comparison is by reference — the crop store replaces the whole value on a write and hands back the
same object until one happens — which is the trick that file already documents.

*Alternative considered:* keying by `identity + cropToken`, as the reference keys its detection cache.
That is a cache of every framing a user has passed through, which the reference needed because its
cache is the only thing standing between a crop drag and an inference call. Here the run store below
is that cache, and it is bounded, shared and outlives the process — so a second unbounded one in the
webview would buy nothing and would have to grow an eviction policy, which is what the reference's own
comment records having to add.

**Registered with `registerFileOwner`**, so closing a photograph releases its faces and closing every
photograph releases all of them, on the same terms as its framing and its transform.

**No deselection field.** Nothing in this slice can write one.

### D10 — A failed detection is an empty answer, and the chain still runs

A detection that fails raises the catalogue's own `errors.faceDetectFailed` and records the photograph
as having no faces. The chain then runs with a face recovery carrying no faces, which
`face-recovery` specifies as restoring nothing while still producing the photograph at the depth
asked for.

The alternative — refusing to run the chain — would mean a failed detection also cancelled an upscale
the user asked for in the same list, which is a second failure caused by the first. The reference
makes the same call, and `face-recovery` already has the requirement that makes it safe: an empty
selection is a request, not an error.

**The empty answer is recorded rather than left absent**, so D4's "faces known for this framing?"
question has an answer and the run is not held forever waiting for one.

### D11 — Detection is not separately cancellable in this slice

There is no `cancel_detect`, and `faces.rs` holds no slot. A window that has moved on discards the
answer by run name, exactly as it already discards a progress report about a run it abandoned.

An abandoned detection costs one bounded run — the detector sees one fixed square whatever the
photograph's size, so it does not scale with megapixels — and its result is written to the run store,
where the next request for that framing is served by it. The transfer is resumable and is work the
next detection would have to do anyway.

What this avoids is a second copy of `enhance/slot.rs`'s displacement half. `Runs` does two things —
displace-and-cancel, and hold the produced pixels — and detection needs only the first; reusing `Runs`
itself would make a detection displace a held enhanced result, which `gui-enhance` requires stays on
screen until a newer one lands. If a third caller for the displacement half ever appears, that is the
change in which it is promoted out of `slot.rs`, before its second caller is written.

*The trade-off is named in Risks.*

### D12 — What this is validated against

**Crates: `gui` only.** `opai` is not edited; `cli` is untouched.

**`rust-sak`: nothing used directly, and nothing missing.** The detector's transfer reaches `fetch`,
its result reaches `memo`, the framed identity reaches `crypto` — all three through `opai` and through
`images/crop.rs`, both of which already exist. No downloader, retry loop, extractor, hasher, codec,
cache or hardware probe is written here. **No new crates. No new npm packages.**

**Model sessions, budgets and providers.** This slice opens no session and holds no handle:
`Opai::execute` acquires a lease for the length of the call and drops it before returning, which its
own documentation states and which is why `suggest` is documented as leaving nothing outstanding for
`release_sessions`. The processor is whatever the settings store holds and reaches `execute` through
the same `Processor -> ExecutionProvider` mapping `enhance/run.rs` already owns; this slice selects,
builds and falls back on nothing of its own, and a provider that cannot open the detector is answered
by `opai`'s own fallback and reported in `Executed`'s provider report.

**Platforms.** Written for all three, validated by hand on macOS; Windows and Linux are covered by
CI's build and test run, as every previous GUI slice has been. Nothing here is platform-shaped: no new
path handling, no new URL scheme, no new window behaviour.

**Execution providers.** Validated on CoreML and CPU, which is what the development machine offers.
CUDA and TensorRT are untested by this slice and untouched by it.

## Risks / Trade-offs

- **The first use of Face Recovery is a download, and the window has never shown one outside setup.**
  → D8 puts it on the indicator that is already on screen, under a name the user recognises. The
  failure mode if that mapping is wrong is a chip with an empty label, which the unit test for the
  family-to-label mapping catches.

- **An abandoned detection keeps running.** D11's trade. A user who adds Face Recovery, waits through
  a transfer and then removes it has paid for a detector they are not using — though it is on disk for
  the next time, and the detection's own run is bounded by the detector's fixed input size rather than
  by the photograph's. → Named rather than mitigated; the mitigation is a second slot, and building
  one before there is a second caller for it is what this project's layering rule explicitly refuses.

- **The run path grows a second await, and it is the one that can be slow.** `useEnhancementRun`'s
  effect already has a cleanup that cancels the enhancement; a detection resolving after that cleanup
  now writes to the faces store for a photograph the window may have moved off. → Harmless by
  construction for a photograph that is still open: the store is keyed by identity and holds the
  framing, so a late write lands in the entry it belongs to and is read by nobody until that
  photograph and framing are current again. It is a warm cache, not a stale render. **Closing is the
  one case that is not harmless** — the forget has already run, so a later write would outlive the
  only thing that empties the store — and it is the one case the write guards against, by asking the
  file store whether the photograph is still open. The notice for a *failed* detection is raised
  either way: what it reports is that work this window asked for failed, which a close does not make
  untrue.

- **Two families in `RUNNABLE` is the first time the type-level check earns its keep.** `lib/enhancements.ts`
  types `RUNNABLE` as the wire's own family union, and a family on the wire and not in the list is a
  compile error. → That is the mechanism working; the risk is the reverse, a menu entry whose
  `newOperation` arm was forgotten, which `noImplicitReturns` turns into a compile error in the same
  file.

- **A face-recovery row's count is read from a store the run path fills**, so between adding the
  enhancement and the detection landing the row has no count to report. → The row reports the count it
  has and the detection is the first thing the run does, so the gap is the length of one detection.
  What it must not do is read `0 Faces` and look like an answer; the info line distinguishes "not yet
  known" from "none found", and that distinction is what the unit test pins.

- **Detection and the chain are two runs, so the indicator's fraction restarts between them.**
  `chainFraction` reaches 1 at the end of the detection and begins again at 0 for the enhancement. →
  Accepted for this slice: they are two runs of two models and `data-inference` deliberately keeps an
  analysis off the chain's range. The label is continuous, which is what a user reads; a single
  fraction across both would mean the window inventing a weighting between a detection and an
  arbitrary chain.

- **`Face`'s snake_case on a camelCase wire looks like a bug.** → D6 names it in `ipc/faces.ts`'s own
  documentation, and the round-trip test — faces out of `detect_faces`, back into `enhance`, compared
  against what `FaceRecovery::athens` builds directly — is what would fail if anyone renamed them.

- **The slice leaves a feature that runs over faces nobody can choose**, which is a state the user can
  see and could mistake for finished. → It is the honest intermediate state: `face-recovery` specifies
  restoring the faces the operation carries, and carrying all of them is a correct selection. Slice 2
  is named in the Slices table above, in this document, which is archived with the change.

## Migration Plan

Nothing to migrate: no stored state, no persisted shape, no URL grammar, no existing command
signature changes. `Requested` gains an arm, which is additive on the wire — an older window sending
only upscales is unaffected.

Reverting is removing `gui/src/faces.rs` and its registration, the `FaceRecovery` arm,
`ipc/faces.ts`, `stores/faces.ts`, `FaceRecoveryOptions.tsx`, the `face_recovery` entry in `RUNNABLE`
and the two arms in `EnhancementRow` — and the application returns to an add menu offering one
family, which is where `add-gui-enhance-2-sidebar` left it.

## Open Questions

None. The six that were open when this slice was planned — where the boxes are drawn, the detector's
precision, how a deselection survives a re-detection, the dialog's dismissal semantics, whether
detection reports progress, and how the work is sliced — were settled with the repository owner and
are recorded above under Slices and in D2, D8 and D11.
