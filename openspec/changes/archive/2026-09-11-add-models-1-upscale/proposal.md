## Why

`opai` can install the ONNX Runtime and report which execution providers a machine supports, but it
has no vocabulary for the thing all of that exists to run: a model. There is no way to say "Kyoto at
2x in FP16" in Rust, and so nothing for the inference API to take as a parameter when it is built.

This slice adds that vocabulary for the upscale family, and the shared types every other family will
reuse. It is deliberately the hardest family first: upscale is the only one that puts a parameter in
the model's identity, the only one that resolves to more than one artifact per operation, and the
only one with two structurally different contracts. A vocabulary designed against the simpler
families would have to be reworked when Osaka arrived.

No inference, no session loading, no downloads. The deliverable is a set of values that name a model,
validate their own parameters, and answer which files on disk they need.

## What Changes

- New shared model vocabulary in `opai`: the eight family prefixes, precision, upscale scale, and the
  two distinct id namespaces described below.
- The four upscale variants as data: Tokyo, Kyoto, Saitama (convolutional) and Osaka (diffusion).
- Artifact resolution: given an operation, the list of `.onnx` files it needs. For a convolutional
  variant that is the native-pass sequence from its scale bucket table; for Osaka it is three graphs,
  two of which pin FP16 regardless of what the operation asked for.
- A published catalogue: which variants exist, which precisions each admits, and each parameter's
  permitted range - so a front end reads them instead of hardcoding them.
- `serde` on the public model types, since the GUI carries operations across Tauri's `invoke`
  boundary and settings persist them.

Deliberate divergences from the Go implementation's *behaviour*, each fixing a real defect:

- **Out-of-range parameters are rejectable.** Go clamps the scale silently and does not bound the
  intensity at all - the ranges live only in React props (`min={0} max={300}` in `OptionsDenoise`).
  A slider cannot exceed them; a CLI flag trivially can. Ranged types get both `new` (returns
  `Result`) and `clamped` (infallible), so the GUI keeps its forgiving behaviour and the CLI gets a
  real error, with in-range guaranteed by the type either way.
- **An operation's identity is the operation itself, not a string.** Go gives every operation a string
  id (`up_kyoto_2.5x_fp32`) and keys the model registry, the image cache, the log lines and the
  progress events on it. That is a workaround for Go being unable to derive equality or hashing on a
  sum type, not a design: the registry needs a comparable key, which Rust derives directly on the
  operation. Flattening the value into a string also has a consumer that parses it back apart - the
  frontend recovers the family with `opId.slice(0, 2)` and the precision with `id.split('_')[2]`, the
  latter positionally correct only for a three-segment id and guarded by a `startsWith('fr_')` that
  keeps it away from `up_kyoto_2.5x_fp32`, where index 2 is the scale. The string id is dropped.
  Operations compare and hash as values, travel as structured data, and show a display label.
- **The one string that survives is the image cache tag**, because that key persists to disk and has to
  outlive the process. It is written deliberately and covered by tests rather than derived from the
  serialized form, so renaming a field cannot silently invalidate every cached image. Nothing else
  keys on it.
- **Scales compare exactly.** Go renders the scale into its id with `%.4g`, so `1.66661x` and
  `1.66669x` produce one string and collide in the image cache - the second request is served the
  first one's image. A scale is quantized to three decimal places and stored as an integer instead,
  which makes it exactly comparable and soundly hashable (`f64` is neither) and makes the cache tag
  lossless.
- **Per-run parameters are typed.** Go passes them as `map[string]any` under stringly-typed keys, and
  `IntensityFromParams` silently substitutes `1.0` on a missing key or a wrong type - a typo is a
  wrong image, not an error.
- **Cache-key contribution is total.** Go makes it an optional interface (`CacheKeyer`) returning `""`
  for "nothing to contribute". A family that carries a per-run parameter and forgets to implement it
  silently collides in the image cache. Exhaustive matching removes the possibility.

**Not** divergences, and kept verbatim: every artifact filename and every display label. The artifact
names name files already published on HuggingFace, and the labels are what users have already seen.

Every model carries two names, and this slice keeps them strictly apart:

- A **codename** - the developer-facing identifier, and the only one the library itself uses. It
  appears in artifact filenames, the image cache tag, the serialized form and the Rust API, so it SHALL be
  lower-case ASCII with no accents, spaces or punctuation: `malmo`, `saopaulo`, `petersburg`. This is
  not a style preference - a codename ends up in a filename, a download URL and a cache directory
  name, and anything else would have to be escaped differently in each of those places.
- A **label** - display text, shown to a user and used nowhere else. It is free to carry accents,
  spaces and punctuation, because nothing is ever composed from it: `Malmö`, `São Paulo`,
  `St. Petersburg`.

For the four upscale variants in this slice the two differ only in capitalisation - `tokyo` and
`Tokyo`, `osaka` and `Osaka` - so nothing here exercises the distinction; it is written down because
the types that enforce it are defined here and every later family inherits them.

Where it does start to matter is the following slice, and the reference implementation is inconsistent
there: `São Paulo` carries its accent as a label, while Malmö and St. Petersburg are spelled `"Malmo"`
and `"Petersburg"` even as labels, where the ASCII restriction does not apply and only their codenames
needed it. Giving those two their proper display spelling is a user-visible change and belongs to the
slice that introduces those families, not to this one.

## Capabilities

### New Capabilities
- `models`: naming an AI model and its parameters, validating those parameters, and resolving an
  operation to the artifacts it requires. This slice populates it with the upscale family and the
  shared vocabulary; a following slice extends the same capability with the remaining families.

### Modified Capabilities
<!-- None. `initialization` is untouched: this slice adds no dependency, no download and no
     runtime interaction. -->

## Impact

- **Crates touched: `opai` only.** No `gui` or `cli` work. These are domain types both front ends
  will need, so they belong in the core library by the layering test; neither front end consumes them
  until the inference API exists.
- **`rust-sak`: nothing applies, and nothing should.** Checked before proposing. This slice writes no
  infrastructure - no hashing, HTTP, downloads, extraction, filesystem work, caching or hardware
  probes - so no `rust-sak` module is relevant and no feature is added to the dependency. The module
  that *will* matter is `fetch` plus `crypto`, when model downloading lands in a later slice.
- **New third-party crates: none beyond `serde`**, which the workspace's Tauri side already requires.
- **Public API: additive only.** Existing exports (`Opai`, `InitError`, `SupportedProviders`,
  `prepare_library_path`, the progress types) are unchanged.
- **Task count: 13**, within the 15 limit. The full six-family scaffold was estimated at roughly 24
  and is split into two slices; see `design.md` for the series.
