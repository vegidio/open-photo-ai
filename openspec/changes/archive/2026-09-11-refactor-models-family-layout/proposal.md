## Why

The six model families are organised two different ways. `upscale` is a directory of three modules;
the other five are single flat files sitting beside the shared vocabulary they depend on. Nothing
about a family's nature decides which shape it gets - upscale got a directory because it was built
first and alone, and the five that followed got files because each was small on its own. Two more
families are already named as future work (Face Recovery and Detection), so the question of which
shape a new family takes is about to be asked twice more with no rule to answer it.

## What Changes

- Every model family becomes a **directory** under `crates/opai/src/models/`, with the same two
  modules: `mod.rs` for the operation and `variant.rs` for what can be named. `denoise.rs`,
  `sharpen.rs`, `light_adjustment.rs`, `color_balance.rs` and `colorization.rs` each split into
  `<family>/mod.rs` + `<family>/variant.rs`.
- `UpscaleVariant` moves from `upscale/mod.rs` into `upscale/variant.rs`, where every other family's
  variant enum will live. `upscale/resolve.rs` stays as it is - the third module a family may add
  when its contract needs one.
- Files directly under `models/` are left holding only shared vocabulary and cross-family code:
  `artifact.rs`, `precision.rs`, `scale.rs`, `strength.rs`, `bias.rs`, `simple.rs`, `catalogue.rs`
  and `mod.rs`. None of them moves or is renamed.
- Each family's `mod.rs` re-exports the variant rows the catalogue reads, so `catalogue.rs` names
  every family at one level (`denoise::ALL`, `upscale::KYOTO`) instead of reaching two levels into
  upscale alone as it does today.
- Tests split along the same seam they already imply: variant-level tests move to `variant.rs`,
  operation-level tests stay in `mod.rs`, and `every_variant()` lives in `variant.rs` for all six
  families.
- The resulting rule is recorded in `openspec/config.yaml`'s `context` block, under `# Conventions`,
  so a later change that adds Face Recovery or Detection reads it as a constraint rather than
  inferring a shape from whichever family it happens to open first.

No behaviour changes, no public API changes, and no **BREAKING** changes. Every type this crate
exports is re-exported flat through `models/mod.rs` and then `lib.rs`, so no consumer names a
module path that moves.

## Capabilities

### New Capabilities

None. This change adds no behaviour.

### Modified Capabilities

None. The `models` capability's requirements are unchanged - this moves code between files without
altering what the library accepts, refuses, resolves to or names. Per the project's own `specs`
rule, requirements are written against observable behaviour rather than internal structure, so a
file-layout rule has no place in `openspec/specs/models/spec.md`; it belongs in the project context,
where this change puts it.

`.openspec.yaml` is therefore marked `skip_specs: true`.

## Impact

**Crates touched: `opai` only.** No `gui` or `cli` work. These are domain types both front ends read
through the crate root, and none of them names a submodule path.

**rust-sak:** nothing. This change writes no infrastructure code - no hashing, HTTP, downloads,
extraction, caching, image IO or hardware probing - so no `rust-sak` module is relevant and no
feature is added or enabled.

**Code:** `crates/opai/src/models/` throughout. Five family files split in two, one enum moves
within `upscale/`, and `catalogue.rs` plus `models/mod.rs` follow the moved items. `lib.rs` is
untouched.

**Tests:** `crates/opai/tests/public_api.rs` is untouched and is the check that the public surface
did not move - it imports from the crate root exactly as a consumer does, so an incomplete
re-export fails there.

**Dependencies, systems, platforms:** none affected.

**Task estimate: 11**, under the 15-task limit, so this is one change rather than a series.

**Divergence from the Go implementation:** none in behaviour. This is a structural decision the
project context already reserves to the Rust side ("do not mirror the Go project's file and
directory structure, its module boundaries below the crate level"), so it needs no parity
justification.
