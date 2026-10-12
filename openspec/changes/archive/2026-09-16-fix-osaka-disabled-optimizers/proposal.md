## Why

`add-upscale-osaka-1-region` concluded that ONNX Runtime 1.26 *"takes
`optimization.disable_specified_optimizers` as a single transformer name rather than as a list"*,
narrowed `EpProfile::disabled_optimizers` from a `Vec<String>` to a single `Option<String>`, dropped
`ReshapeFusion` from Osaka's declaration, and wrote the finding into `execution-providers` as a
requirement — *A model disables at most one graph optimizer* — that every model measured after Osaka
now has to obey.

**The conclusion does not follow from the measurement, and the measurement could not have found it.**
The reference joins that setting's entries with a **semicolon**, and says so at the key's own
declaration: *"The entries are semicolon-separated, and an unrecognised name is ignored rather than
reported - so a typo here is silent, and the only way to know the setting took effect is that the
model loads."* This project's port joined them with a **comma** instead — `settings.disabled_optimizers.join(",")`,
introduced in `add-inference-2-providers` — so the separator was hardcoded and slice 1's sweep varied
only the order and the spacing around it:

```text
  (unset)                                            FAIL
  "SimplifiedLayerNormFusion"                        LOAD
  "ReshapeFusion,SimplifiedLayerNormFusion"          FAIL
  "SimplifiedLayerNormFusion,ReshapeFusion"          FAIL
  "ReshapeFusion, SimplifiedLayerNormFusion"         FAIL
  "SimplifiedLayerNormFusion,NotATransformerAtAll"   FAIL
  "ReshapeFusion"                                    FAIL
```

Every one of those results is exactly what a semicolon-separated setting produces: a comma-joined
value is one unrecognised name, so nothing is disabled and the graph fails precisely as it does with
the setting unset — which is the silent-failure mode the reference's comment warns about. The data
cannot tell "the runtime takes one name" from "the separator is a semicolon", and slice 1 read it as
the first. `ort`'s own doc comment on `with_disabled_optimizers` says *"Accepts a comma-separated
list"*, which is a plausible-sounding claim from the binding rather than from the runtime — the
function passes the string through verbatim.

So a one-character port defect was diagnosed as a runtime limitation, and then written into the
specification as a constraint on every future model. That is the part worth fixing promptly: Osaka
itself loads either way, because `SimplifiedLayerNormFusion` alone is what it needs, but the next
model that genuinely needs two optimizers disabled will be told by this project's own spec that the
runtime cannot do it.

## What Changes

- **The separator is measured before anything is written against it**, on the real diffusion
  transformer under the pinned runtime through this project's own binding — the same discipline slice
  1 applied and the same reason it gives for it. A semicolon-joined pair must load where the
  comma-joined pair fails. Everything below is conditional on that result, and if the measurement
  comes back the other way the finding stands and this change becomes a documentation fix instead.
  **Measured, and it does** — the separator is a semicolon, order does not matter, an unrecognised
  entry is dropped on its own, and the entries are not trimmed. The table is in design.md D1; the
  table above is slice 1's, kept for what it could not have shown.
- **`EpProfile::disabled_optimizer` widens back to a list**, joined with the separator the measurement
  proves, restoring the field the port originally had and the shape the reference has.
- **Osaka declares both names again** — `ReshapeFusion` and `SimplifiedLayerNormFusion` — matching the
  reference exactly. With this, every field of Osaka's profile is the reference's: sequential
  execution, the memory planner off, both optimizers disabled, CoreML on the CPU and GPU, TensorRT's
  builder optimization level at 3, and `cuda_prefer_nhwc` set for the variant and taken back off for
  the transformer alone.
- **The spec requirement is withdrawn rather than softened.** *A model disables at most one graph
  optimizer* describes a runtime that does not exist; it is removed and replaced by one saying a model
  may disable several, with the separator named as the thing that has to be right.
- **The doc comments that carry the wrong finding are rewritten.** `BROKEN_OPTIMIZER`'s long
  explanation, `EpProfile::disabled_optimizer`'s, and `build.rs`'s all currently assert the runtime
  limitation as fact, and are the sort of confident, well-evidenced-looking comment that stops the
  next person checking.

### Deliberate divergence from the reference's behaviour

**None. This change removes one.** Slice 1 recorded its narrowing as a correction *of* the reference —
*"This corrects the reference implementation rather than transcribing it, and it is the one figure in
Osaka's profile measured here rather than inherited"* — and that is what is being withdrawn. After
this change Osaka's execution-provider configuration is the Go application's, field for field.

Whether `ReshapeFusion` is load-bearing for this graph is a separate question and is deliberately not
re-litigated: slice 1 measured that `SimplifiedLayerNormFusion` alone suffices to load it. It is
declared because the reference declares it, the reference's comment describes both as miscompiling
this graph, and a transformer that is merely unnecessary costs nothing but a fusion this graph was not
relying on.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `execution-providers`: *A model disables at most one graph optimizer* is removed — it describes a
  limitation that does not exist — and replaced by a requirement that a model may disable several,
  with the entries separated the way the runtime actually reads them. *A model's measured settings
  include how the graph itself is run* loses the singular that slice 1 put into it, and *Osaka's
  measured settings* names both optimizers again.

## Impact

**Crates**: `opai` only. Nothing here is user-visible and no public API is touched — `EpProfile` is
`pub(crate)` and no consumer configures a session.

**rust-sak**: nothing to check. This is a session-configuration defect in this project's own porting
of the reference, not infrastructure, and no `rust-sak` module is near it. No feature changes and no
dependency is added.

**Files**: `providers/profile.rs` (the field and its comment), `providers/options.rs` and
`sessions/build.rs` (carrying the list through and joining it), `models/upscale/variant.rs` (Osaka's
declaration and `BROKEN_OPTIMIZER`'s comment).

**Ordering**: this should land **before** `add-upscale-osaka-2-pipeline`, which is proposed and not
yet applied. Slice 2 does not touch `EpProfile` and is not blocked by this, so the two are independent
and could archive in either order — but taking this one first means slice 2's end-to-end run on the
real weights exercises the profile the application will actually ship, rather than one known to be
wrong.

**Risk to what already works**: low and worth naming. Osaka loads today, so the failure mode of
getting the separator wrong in the other direction is a graph that stops loading on the CPU provider —
which is exactly what task 1 measures before task 2 writes anything, and what the existing live region
test from slice 1 would catch.

**Task count**: 6, estimated before writing this.
