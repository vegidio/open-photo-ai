## Context

See proposal.md - Why. These facts about the code shape the design:

- **`OperationProgress` already knows the step.** `ChainProgress::operation(index, subject)` builds every reporter
  with its `index` and the chain's `operations`, and `emit` uses both to compute `chain_fraction`. They are dropped
  just before the report is built (`crates/opai/src/inference/progress.rs`). `process` builds `ChainProgress` over
  `planned`, the chain *after* `in_apply_order`, including cache hits (`progress.operation(index, ..).cached()`).
  `execute` builds `ChainProgress::new(on_progress, 1).operation(0, ..)`. So both entry points already hold the right
  numbers.
- **`in_apply_order` is `pub(crate)` in `opai`.** A front end can't call it to work out the position a face recovery
  will run at. It can only sort the chain again by its own copy of the rule, which is the duplication the rule exists
  to prevent.
- **The GUI's `Handover`** (`crates/gui/src/enhance/progress.rs`) depends only on `opai` types: `OnInference`,
  `InferenceProgress` and `Stage`, plus an `AtomicBool`. It has two tests: a detection that worked gives `[20, 20, 60,
  100]`, and a store-served detection gives `[0.0, 0.5, 1.0]`. `prepare_chain` (`enhance/run.rs`) builds it, and both
  the canvas run (`enhance_with`) and the export (`export/run.rs`) use its `chain()`.
- **The GUI shows no counter.** `ProgressBar.tsx` labels the bar from `report.family`, and `progressLabelKey` reads a
  `Detection` report as Face Recovery. Asked how the detection should count, the repository owner said to replicate
  the GUI: the detection is part of the face recovery, not a step of its own.
- **`InferenceProgress` is a plain struct, not `#[non_exhaustive]`.** Test fakes outside `opai` build it with struct
  literals: `gui/src/faces.rs`, `gui/src/export/run.rs`, `gui/src/export/progress.rs`, `gui/src/enhance/progress.rs`,
  and `perf/src/view.rs`.

## Slices

The CLI's new look is delivered as three changes. **This change is slice 2.**

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `cli-look-1-rust-sak-tui` | `rust-sak`'s `tui` feature: `GradientBar`, `Spring`, `ProgressLine`, the ETA text, and the live one-line session that writes to stdout or stderr. A `rust-sak` release (`26.10.6`). Here: the `rust-sak` pins moved to the new tag, and `cli` depending on `rust-sak` with `tui` | - | DONE (archived 2026-10-12) |
| 2 | `cli-look-2-progress-core` | In `opai`: the detection-then-chain `Handover` moved from `gui` into the library, so both front ends draw one bar the same way. The operation's index and the chain's length carried on `InferenceProgress`, so a front end can count `[n/N]` without working it out. The GUI's behaviour doesn't change | - | **CURRENT** |
| 3 | `cli-look-3-terminal` | In `cli`: the new layout: the `💾` dependency line frozen at the last dependency's 100%, the `🖼️` header, the labelled gradient bar using the GUI's labels, and the `✨`/`🧨` result line. Also: `indicatif` removed; notes, face report and provider details taken off the terminal; plain output when stderr is not a terminal; and the `cli-enhance` spec delta | 1, 2 | PENDING |

Slices 1 and 2 are independent. Slice 3 needs both.

## Goals / Non-Goals

**Goals:**

- One bar over a detection and its chain, implemented once, in `opai`, and used by both front ends.
- Every report says which step of the chain as it runs, so `[n/N]` is read off a report and never reconstructed.
- The GUI receives byte-for-byte the same events it receives today.

**Non-Goals:**

- Any change to `cli`. It keeps its two `indicatif` bars until slice 3.
- Sending the counter to the window. `RunProgress` keeps its shape, and the GUI can adopt the counter later as its
  own change.
- Changing the weighting: equal shares per operation, `INSTALL_SHARE` and `DETECTION_SHARE` all stay as they are.
- Merging `find_chain_faces` and `process` into one library call. See Decisions.

## How the work divides

- **`opai`**:
  - the two fields on `InferenceProgress`, filled by `OperationProgress::emit`
  - `Handover`, in a new `inference/handover.rs`, re-exported from `lib.rs`
  - the tests for both, and crate docs ("Running an enhancement" names `Handover` beside `find_chain_faces`)
- **`gui`**: deletes its `Handover` and `DETECTION_SHARE`, and builds `opai::Handover` in `prepare_chain`. Its
  `Thinning`, `describe` and `RunProgress` stay, because they are the window's wire shape and event rate. The test
  fakes name the two new fields.
- **`cli`**: untouched.
- **`perf`**: its test fakes name the two new fields. `perf` doesn't use `Handover`: its `select::with_faces` blocks a
  row rather than running it after a failed detection, and it times the detection on its own.

## `rust-sak` coverage

None needed and none used. This is how the application reports progress over its own chain, not infrastructure.
Slice 1's `ProgressLine` takes `Option<(usize, usize)>` for its counter, and slice 3 feeds that from the two fields
added here. No crate is introduced.

## Decisions

### D1. Two plain fields: `operation_index` and `operation_count`

`InferenceProgress` gains `pub operation_index: usize` (zero-based) and `pub operation_count: usize`. They sit next to
`operation_fraction`, following the existing naming. `emit` fills them from the `index` and `operations` it already
holds. No call site in `process` or `execute` changes.

- **Zero-based**, like every other index in the crate (`select(index, face)`, `ChainProgress::operation(index, ..)`).
  The `+ 1` for display belongs to the front end.
- **The count is carried, not just the index.** The roadmap names only the index. But `N` is just as hard for a front
  end to know: a detection reports `1`, and it is `planned.len()` that the library divides by. Carrying both keeps
  `[n/N]` entirely a matter of reading the report.
- **In apply order.** The position is in the chain *as it runs*, because that is the order a user sees steps happen
  in. A front end that wants the position in its own listed order doesn't exist today.

**Alternative rejected: a `Step { index, count }` struct, or `Option<(usize, usize)>`.** A struct is one more public
type for two integers. An `Option` suggests a report can lack a position, and none can.

**Alternative rejected: making `InferenceProgress` `#[non_exhaustive]` at the same time.** That would stop the `gui`
and `perf` fakes from building reports at all. They would need a public constructor added only for tests. Naming two
fields in five fakes is cheaper and keeps the type as it is.

### D2. `Handover` moves to `opai::inference::handover`, and takes the chain it covers

```rust
pub struct Handover { /* inner: OnInference, detected: Arc<AtomicBool>, attributed: (usize, usize) */ }

impl Handover {
    pub fn new(on_progress: &OnInference, operations: &[Operation]) -> Self;
    pub fn detection(&self) -> OnInference;   // head 0.2 of the bar, a Cached detection dropped
    pub fn chain(&self) -> OnInference;       // the rest, or the whole bar if the detection reported nothing
}
```

The fraction mapping and the "store-served detection owns nothing" rule move exactly as they are. `DETECTION_SHARE`
(`0.2`) moves with them and stays private. Both GUI tests move beside it and keep their expected values.

What is new is `operations`. `new` runs `in_apply_order` over it once and records the position of the first face
recovery and the chain's length. `detection()` then rewrites the detection's `operation_index`/`operation_count` (its
own `0`/`1`) to those values, along with `chain_fraction`. `subject` and `stage` are left alone, so a front end still
sees a `Detection` subject and can label it as the GUI does. `chain()` forwards both fields unchanged. A chain with no
face recovery records `(0, len)`, which nothing ever reads: `find_chain_faces` makes no detection for such a chain.

Why the first face recovery in apply order: the detection is made for the face recoveries, and the GUI already reads
it as Face Recovery. Counting it as that step makes the label and the counter agree. The detection belongs to that
step and isn't a step of its own (the owner's direction: replicate the GUI).

**Alternative rejected: count the detection as its own step (`N + 1`).** The GUI doesn't treat it as one. Also, `N`
would then depend on whether the detection did any work, which is only known after it ends, so the count could change
mid-run.

**Alternative rejected: forward the detection's own `0`/`1`.** Every front end would then have to reconstruct the
face recovery's position, which needs `in_apply_order`, which isn't public. That is the very problem this slice
removes.

**Alternative rejected: one library call that finds faces and processes, with the handover inside.** That would hide
`Handover` entirely. But the GUI's `Enhancer` seam deliberately keeps `find_chain_faces` and `process` separate, so
its fakes can answer each half, and export reports the detection into a different row's bar than the canvas does.
Folding them together would be a larger, behaviour-touching change for a slice that promises none.

### D3. Ownership and lifetime

`Handover` owns a clone of the caller's `OnInference` (an `Arc`) and an `Arc<AtomicBool>`. `detection()` and `chain()`
hand out new `Arc<dyn Fn>` closures that share both. They are `Send + Sync + 'static`, because the detection's
callback is called from the `spawn_blocking` thread the tile loop runs on.

`chain()` must be asked for *after* the detection has ended. That is when whether it reported work is known. The
`find_chain_faces(..).await` completing makes every store the detection's callbacks made happen-before the load in
`chain()`, so `Relaxed` stays sufficient, as it is today. This contract is stated on `chain()`'s rustdoc, as it is in
the GUI's version. No model session, memory budget or execution provider is touched.

### D4. The GUI's wiring changes by one argument

`prepare_chain` calls `Handover::new(on_progress, &operations)` *before* `operations` is moved into
`find_chain_faces`. A face selection never changes which families are in the chain, so the positions recorded match
the chain `process` later runs. `Prepared.handover` keeps its type, now `opai::Handover`, and `enhance/mod.rs` stops
re-exporting `Handover`. `RunProgress` and `describe` don't read the new fields, so the window's events don't change.

## Risks / Trade-offs

- **[A chain listing denoise before face recovery shows the detection as "step 2" before step 1 runs.]** Apply order
  puts denoise first, then face recovery, but the detection runs before the whole chain. → Accepted. The label
  (Face Recovery) and counter (`[2/3]`) agree with each other and with the GUI's labelling, and the counter is the
  face recovery's real position. The CLI, the counter's only consumer in slice 3, offers no denoise, so today
  `--fr --up` reads `[1/2]` → `[1/2]` → `[2/2]`.
- **[Adding fields breaks struct-literal construction outside `opai`.]** → Every such site is in this workspace and
  named in Context. `opai` has no external consumers. The tasks include updating each fake.
- **[`Handover::new` must see the same chain `process` runs.]** If a caller passes a different chain, the detection's
  counter would be wrong. → Documented on `new`. Both callers (`gui` now, `cli` in slice 3) pass the chain they then
  give to `find_chain_faces`, and `in_apply_order` is the same function `process` uses.

## Validation

- Platforms: pure Rust over `opai` types, with no platform-specific code. Unit tests run on macOS locally, and the
  workspace CI runs them on macOS, Windows and Linux.
- Execution providers: not applicable to the logic. The GUI regression check (tasks) runs face recovery plus upscale
  on macOS with CoreML and CPU. CUDA and TensorRT are **untested** by hand, but nothing in this change depends on the
  provider.

## Migration Plan

The struct gains two fields, and a type moves from one crate to another in the same workspace. They land in one
commit, so the workspace compiles at every point. Rollback is reverting that commit. No data, settings, cache keys or
wire shapes change.
