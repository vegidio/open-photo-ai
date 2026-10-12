## Why

The model scaffold covers six of the eight families. The two that were deliberately left out - Detection
and Face Recovery - are the two that do not fit the shape the other six share: Detection returns *faces*
rather than an image, and Face Recovery takes a set of those faces as a per-run input rather than a
scalar. Both are named in `Family` already, with their `dt` and `fr` prefixes reserved, and both are
unreachable: there is no way to name New York, Athens or Santorini, and nothing in the library can
describe a detected face.

This change closes that gap, so the vocabulary covers every family the application ships and the
inference work that follows has one complete set of names to build against rather than six of eight.

## What Changes

- **A `Face` value, and the geometry it is made of.** A detected face is a bounding box, exactly five
  landmark points, and a confidence. New public types: `Point` (x, y), `Rect` (min, max), `Confidence`
  (bounded to 0.0-1.0 on acceptance), and `Face`. They live in one file directly under `models/`,
  because they are cross-family vocabulary: Detection produces them and Face Recovery consumes them,
  and neither family owns them.
- **The Detection family.** One variant - New York - published at FP32 and FP16. A `Detection`
  operation with no per-run parameter at all, resolving to `dt_newyork_fp32` / `dt_newyork_fp16`.
- **Detection is the one family whose result is not an image.** That is stated in the type: the faces a
  run would produce are `Vec<Face>`, and the library says so rather than leaving a caller to discover
  it when inference is wired up.
- **The Face Recovery family.** Two variants - Athens and Santorini - each published at FP32 and FP16,
  resolving to `fr_athens_fp32` and the three siblings. A `FaceRecovery` operation carries the faces it
  is to recover, and an empty set is a legitimate value rather than an error.
- **The faces are outside the model's identity and inside its image cache tag**, exactly as the
  strength and the bias already are for the four parameterized families. Two runs over different face
  selections are one model - so the registry loads Athens once no matter how many faces a user toggles -
  and two different images, so they cannot be served one another's picture.
- **The catalogue publishes all eight families.** Detection is listed with its one variant and no
  parameters; Face Recovery is listed with its two variants and no parameters, because faces are a
  structural input supplied by a Detection run rather than a control a front end draws.

Nothing here runs anything. No session is built, no graph is loaded, no face is detected and no face
is restored - this change names what those runs will be, as the six families before it did.

### Deliberate divergences from the reference implementation

- **Detection appears in the catalogue; the Go frontend has no `dt` row.** Its `ENHANCEMENT_ORDER` is
  the seven user-choosable families, and its `FaceService.DetectFaces` hard-codes `newyork.Op`. Here a
  front end reads the codename and the precisions from the library, so the one place that knows which
  model detects faces is the library. No user-visible chooser gains a row: Detection is published as a
  family that takes no parameters, and a front end draws controls for the seven it always drew.
- **`FaceRecovery` declares only its own graph in `required_artifacts`.** In the reference,
  `facerecovery.GetDtModel` acquires the detection model from inside face recovery, so the two are
  coupled. Here detection is its own operation and reports its own artifact; an operation naming a
  graph it never opens would make "what must be on disk for this run" untrue for every other family.
  Nothing is lost - a caller that detects then recovers asks both operations, which is what it already
  does to get the faces.
- **A confidence outside 0.0-1.0 cannot be constructed.** The reference carries a bare `float32`; the
  value is a softmax output and is naturally in range, so this is a guarantee being written down rather
  than a behaviour change.
The image cache tag is **not** a divergence: it folds the faces in exactly as the reference's
`FacesCacheKey` does, by spelling each bounding box out at two decimals in order. A fixed-length digest
was considered and rejected - see `design.md`.

### rust-sak

Checked before designing anything, as the two previous slices were. **Nothing applies.** This change
writes no infrastructure - no hashing, HTTP, download, extraction, filesystem, caching, telemetry or
hardware probe - so no module is relevant and no feature is added to the existing
`["crypto", "fetch", "fs", "sysinfo"]` set. Nothing here is a candidate for extending `rust-sak`;
model naming is application domain vocabulary and does not belong in a general-purpose crate.

Two modules were read against anyway, because the cache-tag decision turns on what they do: `memo`'s
`key_from` SHA-256s its parts into a fixed-length key, which is what establishes that an operation's
cache tag is a *part* of a cache key rather than a name on disk, and `crypto` was weighed for digesting
a face selection and deliberately not used.

No new third-party crates. `serde` and `thiserror` are already direct dependencies and are all this
needs.

## Capabilities

### New Capabilities

None. This extends the vocabulary the `models` capability already describes.

### Modified Capabilities

- `models`: adds the Detection and Face Recovery families and their variants, the `Face` value and its
  geometry, the rule that a set of faces is outside a model's identity and inside its cache tag, the
  fact that Detection alone produces faces rather than an image, and the catalogue now publishing all
  eight families.

## Impact

**Crates touched: `opai` only.** No `gui` and no `cli` work. These are domain types both front ends
need - the GUI to show a face box overlay and let a user toggle a face, the CLI to report how many
faces were found - so by the layering test they belong in the core library, and neither front end can
consume them until an inference API exists. There is no user-visible surface in this change, and
therefore no frontend component, i18n key or Tauri command to add.

**Code:**

- New: `models/face.rs`, `models/detection/{mod,variant}.rs`, `models/face_recovery/{mod,variant}.rs`.
- Modified: `models/catalogue.rs` (two more entries, and the tests that walk every family),
  `models/mod.rs` (module wiring, re-exports, and the cross-family cache-tag collision tests, which now
  have eight families to keep apart), `crates/opai/src/lib.rs` (re-exports),
  `crates/opai/tests/public_api.rs`.

**Public API:** additive. Nothing existing is renamed, removed or given a different shape - including
`FamilyEntry` and `ParameterEntry`, which describe the two new families unchanged. No persisted value,
artifact name or download URL changes.

**Estimated task count: 11.** Inside the 15-task limit, so this is one change rather than a series -
but it is the third and last slice of the model-naming series `add-models-1-upscale` ->
`add-models-2-enhancements` -> this one, and `design.md` carries that list.
