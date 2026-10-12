## Why

Slice 1 built the model vocabulary against the hardest family and populated only that one. `opai` can
name the four upscale models and resolve them to artifacts; it still cannot name the thirteen models
that make up the other five enhancement families, so neither front end can offer them and the
inference pipeline has nothing to be handed.

This slice finishes the scaffold the series set out to build. The thirteen are structurally uniform -
one artifact each, one precision pair, a per-run parameter that is never part of the model's identity
- which is why they come second: the vocabulary is now settled and they populate it rather than shape
it.

## What Changes

- **Five new families**, thirteen variants in total, each resolving to one artifact:
  - `dn` denoise - Stockholm, Gothenburg, Malmo
  - `sh` sharpen - Moscow, Petersburg, Novgorod
  - `la` light adjustment - Paris, Lyon
  - `cb` colour balance - Rio, São Paulo
  - `cl` colorization - Delhi, Mumbai, Jaipur
- **Two ranged parameter types**, following the `Scale` precedent (bounded, quantized on acceptance,
  with both a rejecting and a clamping constructor):
  - `Strength`, 0.0 to 3.0, carried by denoise and sharpen. 1.0 is the model's own output; below it
    softens the effect, above it amplifies.
  - `Bias`, -1.0 to 1.0, carried by light adjustment and colour balance. 0.0 leaves the image alone
    and a negative value inverts the adjustment.
  - Colorization takes no parameter at all, and carries no empty parameter slot to say so.
- Each family's operation is its own type, mirroring `Upscale`: a variant enum carrying the precision,
  a value that compares and hashes as the model's identity, a display name, an image cache tag, and
  resolution to its single artifact.
- The parameter is deliberately **outside** the identity for all four parameterized families, so a
  registry keyed on the operation loads one session across every value a user drags through - and the
  cache tag carries it, so two values are not served one another's image.
- **BREAKING** (internal, pre-release): slice 1's `ConvPrecision` is renamed `FloatPrecision`. Its
  domain - published at FP32 and FP16 - is exactly what all thirteen of these variants need, and the
  name "convolutional" means nothing on a colorization or colour-balance model. The serialized
  spelling (`"fp32"`, `"fp16"`) is unchanged, so nothing persisted or published is affected; only the
  Rust type name moves. Sharing it is what keeps `dn_stockholm_int8` unnameable.
- The catalogue grows five entries, so a front end can populate all six model choosers and configure
  every parameter control from the library alone.

## Capabilities

### New Capabilities

None. These are further requirements of the capability slice 1 introduced.

### Modified Capabilities

- `models`: adds requirements for the five enhancement families - which variants and precisions each
  publishes, the two parameter types and their ranges, that a parameter is excluded from the model's
  identity but included in its cache tag, that colorization carries none, and that the catalogue
  publishes all six families. The existing upscale requirements are unchanged in behaviour; the
  precision rename is a Rust type name and not an observable behaviour, so it adds no requirement.

## Impact

- **Crates: `opai` only.** No `gui` or `cli` work. These are domain types both front ends need - the
  GUI to populate its choosers and sliders, the CLI to parse and validate flags - so by the layering
  test they belong in the core library. Nothing consumes them until the inference API exists, so there
  is still nothing to wire up, exactly as in slice 1.
- **`rust-sak`: nothing applies.** Checked before proposing. This slice writes no infrastructure - no
  hashing, HTTP, download, extraction, filesystem, caching, telemetry or hardware probe - so no
  `rust-sak` module is relevant and no feature is added to the existing
  `["crypto", "fetch", "fs", "sysinfo"]` set. Model naming is application domain vocabulary, specific
  to this app, and does not belong in a general-purpose crate.
- **No new third-party crates.** `serde` and `thiserror`, both already direct `opai` dependencies, are
  all this needs.
- **Code touched:** new modules under `crates/opai/src/models/`; `precision.rs` for the rename;
  `catalogue.rs` for five entries; `models/mod.rs` and `lib.rs` for re-exports; `upscale/` and
  `tests/public_api.rs` follow the rename.
- **Task estimate: 13**, within the 15 limit, so this stays one change.

### Deliberate divergences from the reference implementation

- **The parameter ranges become library facts rather than UI literals.** In the reference, `0-300` and
  `-100..100` are literals in React components with nothing connecting them to the Go code, which
  never bounds an intensity at all. Here `Strength` and `Bias` enforce the same bounds the catalogue
  publishes, so a value the library would refuse cannot be offered and a value that reaches inference
  is already in range. The user-visible behaviour of the sliders is unchanged.
- **A quantized parameter, as with `Scale`.** The reference builds its cache key with `"i=%.3g"`, so
  two intensities that round alike collide and the second run is served the first one's image.
  Quantizing on acceptance makes the two the same value before anything is rendered, so they are one
  run rather than two sharing a key. The divergence is only reachable for inputs finer than the UI can
  produce - it steps in whole percent - and it replaces a wrong image with a rounded parameter.
- **Colorization's absent parameter is a shape, not a missing implementation.** The reference expresses
  "no per-run input" by an operation declining to implement two optional interfaces, so a family that
  acquires a parameter and forgets one of them collides silently in the cache. Here it is the absence
  of a field, and every family's cache-tag contribution is total.
- **Label spellings are taken from the reference's own variant tables**, not from the prose in this
  project's context: the sharpen variant is `Petersburg` (not "St. Petersburg"), the denoise variant is
  `Malmo` (no diaeresis) and the colour-balance variant is `São Paulo`. These are user-facing and
  match what the Go application shows today.
