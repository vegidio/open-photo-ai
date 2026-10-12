## Context

See `proposal.md` - Why, and `specs/models/spec.md` for the requirements. What shapes the approach is
what the two previous slices already built and what these two families do that none of the six do.

Already in place: `Precision` / `FloatPrecision`, `Family` with all eight prefixes (`dt` and `fr`
included and unused), `ArtifactId`, `RangeError` with its `parameter: &'static str` field,
`SimpleVariant` and `single_artifact`, the `FamilyEntry` / `VariantEntry` / `ParameterEntry` catalogue
types, six operation types with their identity / cache-tag split, and the cross-family cache-tag
collision tests in `models/mod.rs`.

Two things break the mould the six share, and they are what the decisions below are about:

1. **Detection's result is not an image.** Every other family maps an image to an image. New York maps
   an image to a set of faces.
2. **Face recovery's per-run input is not a scalar.** The four parameterized families carry a bounded
   number. Face recovery carries a variable-length, possibly empty, ordered set of structured values -
   so it cannot be `Copy`, and its cache tag cannot be a formatted number.

The reference implementation is the behavioural source of truth and four of its files were read
against rather than assumed: `models/ids_test.go` pins all six artifact ids as literals,
`models/detection/types.go` carries the `Face`/`PointF`/`RectF` shapes and the five-landmark constant,
`models/detection/variant.go` and `models/facerecovery/variant.go` carry the `Op` shapes and which
fields are and are not in the identity, and `models/facerecovery/common.go` carries `FacesCacheKey`.

## Goals / Non-Goals

**Goals:**

- Six model names that cannot be constructed in a state the published artifacts do not cover, closing
  the vocabulary at all eight families.
- One `Face` value both families speak, owned by neither, with the guarantees the reference leaves to
  convention: exactly five landmarks, and a confidence that is in range because it exists.
- A face-recovery cache tag that distinguishes every selection a user can make, at a length that does
  not depend on how many faces are in the photo.

**Non-Goals:**

- Any inference. No session, no tiling, no detection decode, no alignment, no blending. Named here so
  they are visibly deferred rather than missed, because the reference carries substantial hard-won
  material for each: RetinaFace anchor generation and decode (`detection/anchors.go`, `decode.go`,
  `filter.go`, `input.go`), the ArcFace landmark template and the similarity transform, the feathered
  elliptical blend mask, Athens's fidelity weight and both variants' 512px tile size, the graph input
  and output tensor names, and - the largest body of it - the per-variant `EPProfile` tuning, which is
  measured per graph and per precision and is the most detailed provider material in the reference.
- Downloading model artifacts. This change names them.
- The model registry, leases and memory budgets. No session exists yet.
- Autopilot, crop, per-image face selection UI, and the export queue.
- Any GUI or CLI work.

## Slices

This is the model-naming series: giving the library a complete, typed vocabulary for every model the
application ships, and nothing else.

| # | Slice | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `add-models-1-upscale` | Shared model vocabulary (precision, family prefixes, operation/artifact ids, catalogue, serde) plus the four upscale variants, both contracts, and artifact resolution | initialization slices (done) | **done** (archived `2026-09-11-add-models-1-upscale`) |
| 2 | `add-models-2-enhancements` | Denoise, Sharpen, Colorization, Light Adjustment, Color Balance - 13 variants, one artifact each - plus the `Strength` (0-3) and `Bias` (-1..1) parameter types | 1 | **done** (archived `2026-09-11-add-models-2-enhancements`) |
| 3 | `add-models-3-faces` | Detection (New York) and Face Recovery (Athens, Santorini), the `Face` value and its geometry, and the catalogue at all eight families | 1, 2 | **this change** |

With this slice the series is complete: every family in the domain is nameable and the catalogue is
closed. A later change may still touch this vocabulary - the two things the reference's frontend
registry holds that the catalogue deliberately still does not (a default variant per family, and the
HD/SD quality-tier naming over FP32/FP16) were noted as open in slice 2 and remain open - but neither
is a slice of this series.

Downstream of the series, each its own feature and planned as its own change when reached: downloading
artifacts from HuggingFace, the model registry with its leases and memory budgets, and inference
itself.

## Crate split

**`opai` only.** No `gui`, no `cli`. These are domain types both front ends need - the GUI to draw a
face-box overlay and let a user toggle individual faces, the CLI to report a face count - so by the
layering test they belong in the core library, and neither front end can consume them until an
inference API exists. There is no user-visible surface in this change, and therefore no React
component, i18n key or Tauri command to add.

## Dependencies

**`rust-sak`: nothing applies**, as in slices 1 and 2. This change writes no infrastructure - no
hashing, HTTP, download, extraction, filesystem, image codec, memoization, telemetry or hardware probe -
so no module is relevant and no feature is added to the existing
`["crypto", "fetch", "fs", "sysinfo"]` set. Model naming is this application's domain vocabulary and is
not a candidate for extending `rust-sak` in either direction.

Two of its modules were read against anyway, because the cache-tag decision turns on what they do.
`memo::key_from` SHA-256s its parts into a 64-character key, which is what makes a `cache_tag` a *part*
of a key rather than a name on disk - that is the finding the decision below rests on. `crypto` was
considered for digesting a face selection and is deliberately **not** used: the length it would buy is
already provided one layer up, and hashing here would only hash something `key_from` is about to hash
again.

**No new third-party crates**, and none was weighed, because nothing new is being chosen.

`serde` (with `derive`) and `thiserror` are already direct `opai` dependencies and are all this needs.

## Decisions

### `Face` and its geometry are a file under `models/`, not a member of either family

`models/face.rs`, alongside `precision.rs`, `strength.rs` and `bias.rs`. The project convention is
explicit that a file directly under `models/` is shared or cross-family vocabulary and a directory is
one family, and this is the textbook case: Detection produces faces, Face Recovery consumes them, and
the value is not a fact about either graph.

Putting it inside `detection/` - which is what the reference does, with `facerecovery` importing
`detection.Face` - would make the family that happens to produce the value its owner, so face recovery
would depend on the detection *module* while deliberately not depending on the detection *artifact*.
The reference lives with that because Go packages gave it nowhere better; there is somewhere better
here.

Four types, all public:

```rust
pub struct Point { pub x: f32, pub y: f32 }
pub struct Rect  { pub min: Point, pub max: Point }
pub struct Confidence(f32);                                  // 0.0..=1.0, validated
pub struct Face  { bounding_box: Rect, landmarks: [Point; 5], confidence: Confidence }
```

`f32`, not `f64`: these are model outputs and the graphs emit `float32`. Widening would invent
precision the detector never had, and the reference carries `float32` throughout.

`[Point; 5]`, not `Vec<Point>` or a slice. The count is a property of the RetinaFace graph, the five
are read positionally by whatever aligns a face, and an array makes a four- or six-landmark face
unrepresentable rather than a runtime check - the same move the vocabulary already makes everywhere
else. The reference reached the same conclusion from the other direction: its `ArcfaceTemplate` was
changed from a slice to an array precisely because an exported mutable slice let any importer reorder
the reference points and break alignment process-wide.

The five are documented in their fixed order - left eye, right eye, nose, left mouth corner, right
mouth corner - transcribed from `detection/types.go`.

### `Confidence` is validated but not quantized

`Confidence::new(f32) -> Result<Confidence, RangeError>`, accepting `0.0..=1.0` and rejecting anything
outside it including `NaN`, with `NAME`/`MIN`/`MAX` constants like `Strength` and `Bias`. `Face::new`
takes an already-built `Confidence`, so it is itself infallible - a face that exists is a face whose
confidence is meaningful, with no second validation path.

It is **not** quantized, and that is the one place it departs from `Scale`, `Strength` and `Bias`.
Those three quantize because they sit in a `Hash`/`Eq` identity that `f64` cannot soundly provide, and
because two user-supplied values that would collide in the cache must become one value first. Neither
applies here: a confidence is a model observation, it is in no identity, and it is in no cache tag.
Quantizing it would discard resolution for nothing.

The same reasoning settles the coordinates: carried exactly as reported. The reference's own comparison
of the FP16 and FP32 detectors found they return the same faces in the same order, differing only in
sub-pixel positions - so sub-pixel positions are the whole of what distinguishes two runs, and rounding
them on acceptance would be discarding the only thing there is to discard.

*Alternative considered:* a bare `pub f32`, as the reference has. Rejected - it costs one newtype to
make "a confidence is a probability" a fact rather than a comment, and the spec now asks for it.

### `Faces` is a newtype over `Arc<[Face]>`, and `FaceRecovery` is `Clone` rather than `Copy`

The five other single-graph operations are `Copy`. A variable-length set cannot be, so something has to
give, and the choice is *what* is given up:

```rust
pub struct Faces(Arc<[Face]>);        // empty is a legitimate value
pub type  DetectionOutput = Faces;    // the one family whose result is not an image
```

`Arc<[Face]>` rather than `Vec<Face>`: a face-recovery operation is cloned freely - held by a front
end, pushed onto an operation list, handed to a cache lookup - and with a `Vec` each clone copies every
face. With an `Arc` a clone is a refcount bump, the contents are immutable by construction, and the
"order preserved, a face handed in is the face handed back" guarantee needs no defensive copy.

This is the first `Arc` in the model vocabulary, and it is worth being explicit that it does not
weaken the property the whole vocabulary rests on: it holds plain data with no `Drop`, no handle and no
native resource. An operation remains a description of *what* to run that a front end can hold and
persist independently of any session. What is given up is `Copy` alone, on one of eight types.

`Faces` is also where the "empty is legitimate" and "order is preserved" guarantees live, so both
families get them from the same place, and it is what carries the cache-tag fragment below.

`DetectionOutput` is a public alias rather than a trait or an associated type. A trait describing what
an operation produces would be inference API designed in advance of the inference API; an alias states
the fact the spec asks for - detection's result is a set of faces, not an image - additively and with
nothing to unpick when the real signature arrives.

### The faces are outside the identity and inside the cache tag

This is the rule the other seven families already follow, applied to a non-scalar. It is also what the
reference does: `facerecovery.Op.Id()` omits the faces and its `CacheKey()` folds them in, with the
comment naming the reason - the registry must reuse one session across every selection a user makes.

`PartialEq` and `Hash` are hand-written over the variant alone, in the same shape as `Denoise`'s, and
`Eq` is a manual `impl Eq for FaceRecovery {}` rather than a derive, because a derive would demand
`Faces: Eq` and `f32` cannot give it. That is a mechanical consequence worth writing down, since it is
the one line in this change that would otherwise look like an oversight.

`Detection` needs none of this: it has one field, so its `PartialEq`, `Eq` and `Hash` are derived, like
`Colorization`'s and for the same reason - writing them out would suggest there was something to
exclude.

### The cache tag spells the faces out, as the reference's does

Format, extending the established `{prefix}-{codename}-{precision}` shape:

```
dt-newyork-fp32                                      detection: nothing to fold in
fr-athens-fp32-f0                                    no faces selected
fr-athens-fp32-f12.34,56.78,90.12,34.56;             one face
fr-athens-fp32-f12.34,56.78,90.12,34.56;100.00,...;  two, in order
```

The signature after `-f` is the reference's `FacesCacheKey` verbatim: each face's bounding box at two
decimals, in order, semicolon-terminated. Two decimals and bounding-boxes-only are both inherited
deliberately. The reference's reasoning is that bounding boxes uniquely identify the deterministically
detected faces within an image, and two decimals is far finer than any difference a re-detection of the
same image produces; landmarks and confidence move with the box and add nothing a box does not already
distinguish.

**This tag is not a filename, and that is what makes spelling it out the right choice.** The reference
composes its cache key in `internal/cache.go`: every operation's id, plus this contribution, joined and
handed to `memo.KeyFrom`, which SHA-256s the lot into 64 hex characters. That hash is the key that
reaches the store. `rust-sak`'s `memo::key_from` is the same function with the same guarantee, so when
the image cache is built here a `cache_tag` will be one *part* handed to it, never a name on disk.

Length control therefore belongs at the cache layer, which already has it and cannot avoid having it -
the full key also carries the input image's hash and every operation applied before this one, so it has
to be hashed whatever one operation contributes. Digesting the faces here would hash something that is
about to be hashed again, and would pay for it twice over:

- A 64-bit digest distinguishes selections only probabilistically. Spelling them out distinguishes them
  exactly, and the SHA-256 at the cache layer is where collision resistance is actually wanted.
- A tag stays legible - in a log line, a debug print, a failing test's message - which is the whole
  reason the six existing families format theirs rather than hashing them.
- All eight `cache_tag` methods stay pure formatting, with no hashing dependency in the model
  vocabulary.

*Alternative considered and rejected:* an XXH3-64 digest of the same signature, for a fixed-length tag.
The length it buys is not needed, for the reason above, and it costs all three points.

`-f0` for the empty selection. The reference returns `""` there and `cacheKey` then omits the
contribution entirely, which is correct but leaves an empty selection's tag indistinguishable in shape
from a family that has no parameter at all. `cache_tag` is a total method here, and an explicit `f0`
says "this operation carries faces, and there were none" at a glance. No non-empty signature can
produce it: a real box at the origin renders `0.00,...`.

The signature uses only digits, `.`, `,`, `;` and `-`, so it contains no underscore and cannot be
mistaken for a published artifact name - the invariant `models/mod.rs` already tests across families,
now with eight families to keep apart instead of six.

### Face recovery declares only its own artifact

Settled with the project owner. `FaceRecovery::required_artifacts()` returns exactly one id, its own
graph, and never `dt_newyork_*`.

In the reference, `facerecovery.GetDtModel` acquires the detection model from inside face recovery, at
the recovery's own precision. Here detection is an operation in its own right, run by the caller, and
it reports its own artifact. Making face recovery report a graph it never opens would break "what must
be on disk before this operation can run" for every other family, and would pin New York as the
detector inside face recovery - a coupling there is now no reason to have.

The precision pairing the reference's comment argues for (a recovery at FP16 detects at FP16, so a user
on the SD tier does not pay for the 88 MB FP32 detector to feed a 44 MB FP16 graph) is preserved by the
caller choosing the detection precision, which is exactly what the reference's own `FaceService` does.
It is behaviour for whichever change wires the pipeline up, and is recorded here so it is not lost.

### Detection is published in the catalogue; both families report no parameters

Settled with the project owner. `catalogue()` gains two entries built through the existing
`simple_family` helper, both with `Vec::new()` for parameters, and `FamilyEntry` / `ParameterEntry` are
unchanged.

Detection is listed although the reference's frontend registry has no `dt` row and its `FaceService`
hard-codes `newyork.Op`: the catalogue's job is that a front end restates no model vocabulary of its
own, and "which model detects faces" is model vocabulary. No chooser gains a row - a family with no
parameters is one a front end draws no control for, which is already how colorization is read.

Face recovery reports no parameters for a different reason from colorization's, and the distinction is
worth keeping straight: colorization has nothing to supply, while face recovery has something to supply
that is not a control. `ParameterEntry` describes a numeric range with a min and a max; a set of faces
has neither, and widening the type to express it would change `FamilyEntry`'s shape for all eight
families to carry a variant only one of them uses. Empty is the honest answer for now.

Both are appended after the existing six rather than inserted in `Family` declaration order. The
existing order is pinned by a test and by nothing else - it is not a presentation order, and the
reference's frontend order (`dn fr cl la cb sh up`) differs from it already - so reordering published
entries would churn a list for no gain.

### Module layout follows the established seam

```
models/face.rs                  Point, Rect, Confidence, Face, Faces, DetectionOutput
models/detection/variant.rs     NEW_YORK row, DetectionVariant, every_variant()
models/detection/mod.rs         Detection
models/face_recovery/variant.rs ATHENS and SANTORINI rows, FaceRecoveryVariant, every_variant()
models/face_recovery/mod.rs     FaceRecovery, FaceRecoveryParams
```

A family is a directory with `mod.rs` and `variant.rs`; variant-level tests live in `variant.rs`,
operation-level tests in `mod.rs`, and each family publishes `every_variant()` for the cross-family
tests to walk. Neither family needs a third module: nothing here is the kind of per-family contract
that earned `upscale/resolve.rs`.

The codename/label split already carries accented and multi-word labels (`saopaulo` / `São Paulo`), and
New York is the third case of it: codename `newyork`, label `New York`. Both are transcribed -
`ids_test.go` pins `dt_newyork_fp32`, `detection/newyork/newyork.go` pins the label.

## Ownership and lifetimes

Unchanged in substance from slices 1 and 2, and restated because this slice introduces the first
reference-counted value in the vocabulary and the project's sharpest convention is about ownership
around sessions.

Nothing here owns a native resource. Every operation is a plain value describing *what* to run, with no
handle, no `Drop` and no reference to a loaded session. `Arc<[Face]>` is shared ownership of immutable
plain data - the thing it makes cheap is cloning a description, not sharing a resource - so the
description a front end holds and serializes still outlives, and stays independent of, any session
built from it.

The use-after-free hazard the project conventions call out is a property of sessions, which still do
not exist. When they do, face recovery is the family where it will bite hardest: it is the one that
will hold two models at once (a detector and a restorer) in the reference, and the reference's own
`GetDtModel` comment says as much - "holding the model without the lease is a use-after-free waiting to
happen". This change removes half of that hazard structurally by the decision above: the two operations
are independent, so nothing here forces one to hold a lease on the other's behalf.

## Platforms and execution providers

Platform-independent and provider-independent by construction: no artifact name, label, precision or
bound varies by operating system, architecture or execution provider, and the one hash used has a
byte-stable, endianness-defined output that `rust-sak` documents as matching `xxhsum`. Nothing needs
per-platform validation, and CI's existing matrix covers it. **No execution provider was exercised,
because nothing in this change builds a session.**

That is exactly where the deferred `EPProfile` material becomes real. The reference's tuning for these
three graphs is the most thoroughly measured in its catalogue and is per graph *and* per precision -
New York prefers NHWC on CUDA at both precisions and runs sequentially; Athens sets NHWC for FP16 only
because FP32 measures the opposite way, and keeps itself off the Neural Engine; Santorini does the
reverse of Athens on CoreML and cannot use NHWC on CUDA at all, because 23 of its convolutions take a
reshaped weight. Carrying any one of those over to another graph by analogy is precisely what the
reference's comments warn against. Whoever builds sessions must read those three files rather than
generalising, and must state which platforms and providers were validated at that point.

## Risks / Trade-offs

- **An id or label transcribed wrong names an artifact that does not exist** → All six ids were read
  from `models/ids_test.go`, which pins them as literals because they key the on-disk cache and name
  the downloaded files; the labels were read from each variant's own literal. Tests assert the full
  expected set per family.
- **`FaceRecovery` losing `Copy` ripples into call sites later** → It has no call sites yet, which is
  the cheapest possible moment for this. The cost is bounded to a `.clone()` that is a refcount bump,
  and the alternative - a fixed-capacity inline array of faces - would cap how many faces an image may
  have, which is a far worse trade for a group photo.
- **A face-recovery cache tag grows with the face count, so a forty-face photo builds a ~1 KB string
  per lookup** → Accepted, and the same as the reference. It is a transient string handed to
  `key_from`, not stored and not a filename; the cache key it contributes to is 64 characters whatever
  it carries. Bounding it here would trade an exact signature and a legible one for a saving at the
  wrong layer - see the decision above.
- **Two families named but not runnable could read as complete** → The same risk slices 1 and 2 carried
  and the same mitigation: the Non-Goals above enumerate every piece of detection and face-recovery
  behaviour left in the reference, by file, so the next change inherits a list rather than a search.
- **Sub-pixel coordinates make two detections of the same image differ, producing a cache miss** →
  Inherent, and inherited: the tag rounds to two decimals exactly as the reference does, and detection
  is deterministic for a given image, crop and precision, so a repeated run reproduces the same
  coordinates. The reference's front end additionally caches detection results by file hash plus crop
  plus precision, which is the layer that keeps a re-detection from happening at all - noted for
  whichever change wires that up.
