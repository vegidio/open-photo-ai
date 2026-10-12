## Why

The CLI's new look (slice 3) draws one bar for the whole run, labelled `<Step> [n/N]`. Two pieces it needs live in
the wrong place or don't exist yet:

- **The single bar over a detection and its chain is GUI code.** A chain with a face recovery is two runs underneath
  (the detection `Opai::find_chain_faces` makes, then the chain), and each reports its own `0..1`. The GUI's `Handover`
  (`crates/gui/src/enhance/progress.rs`) maps the detection onto the first fifth of the bar and the chain onto the
  rest, so the bar moves forward only once. The CLI needs exactly the same mapping. Under the project's layering rule,
  code both front ends need belongs in `opai`. Today the CLI draws two bars instead.
- **A report doesn't say which step of the chain it is.** `InferenceProgress` carries the subject and two fractions,
  but not the operation's position or the chain's length. A front end can't reliably work out `[n/N]` itself:
  `Opai::process` sorts the chain into apply order, and a detection reports as a one-operation run of its own.

This slice moves the first into the library and adds the second. Nothing changes for the user in either front end.

## What Changes

Task estimate: **9**. This is slice 2 of the `cli-look` series (see design.md - Slices).

- **`InferenceProgress` gains `operation_index` and `operation_count`.** The index is the operation's position in the
  chain *as it runs*, after apply-order sorting, counted from zero. The count is the chain's length. Every report
  carries both:
  - every stage, including `Cached`
  - from `process` and from `execute`, where a one-operation run reports `0` of `1`
- **`opai` gains a public `Handover`**, moved from `gui` with its behaviour unchanged:
  - The detection owns the first `0.2` of the bar, but only when it reported work.
  - A detection served from the store reports nothing, and the chain then takes the whole bar.
  - The chain's reports are mapped onto whatever the detection left.

  It is built from the chain it covers. A detection report going through it then carries the face recovery's position
  and the chain's length, not the detection's own `0` of `1`. The detection is drawn as the start of the face recovery,
  which is how the GUI already labels it ("Face Recovery").
- **`gui` switches to `opai::Handover`** and deletes its own copy and `DETECTION_SHARE`. The window receives exactly the
  same reports as before: the wire shape `RunProgress` doesn't gain the new fields.
- **BREAKING (inside the workspace only):** `InferenceProgress` is a plain public struct, so every struct-literal
  construction must name the two new fields. These are only the test fakes in `gui` and `perf`, which this change
  updates. `opai` has no consumers outside this workspace.
- `cli` isn't touched. It keeps its two `indicatif` bars until slice 3 adopts `Handover` and the counter.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `image-enhancement`: "A run reports what it is doing and how far it has got" gains the requirement that each
  report says which operation of the chain it is (position in apply order) and how many operations the chain has.
- `face-recovery`: adds a requirement that the library can report a chain's detection and the chain on one bar that
  moves forward once. The detection's reports are attributed to the face recovery they were made for.

`gui-enhance` doesn't change. Its requirements describe what the window receives, and that stays the same.

## Impact

- **Crates touched:**
  - `opai`: `inference/progress.rs` for the two fields, plus a new `inference/handover.rs`, re-exports and crate docs.
  - `gui`: `enhance/progress.rs`, `enhance/run.rs`, `enhance/mod.rs`, `export/run.rs` and test fakes in `faces.rs`
    and `export/`.
  - `perf`: test fakes in `view.rs` only.

  `cli` isn't touched.
- **Logic in a front end:** this change takes logic *out of* a front end. Only the window's thinning and wire shape
  stay in `gui`.
- **rust-sak:** nothing there covers this, and nothing should. It is how this application reports progress over its
  own models, not infrastructure. The `tui` widgets from slice 1 only draw a counter they are given, so this slice is
  what supplies it.
- **Dependencies:** none added.
- **Parity:** the reference implementation has no step counter. The detection's share (`0.2`) is the reference's
  `progressAfterDetect`, unchanged.
