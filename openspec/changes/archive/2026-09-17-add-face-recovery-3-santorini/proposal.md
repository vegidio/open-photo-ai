## Why

Santorini is the last operation in the catalogue that is fully described and still refused. Slice 1 made it
expressible, cacheable and catalogued; slice 2 built the whole of the geometry a face-recovery run is made of —
the ArcFace template and the similarity fit, the warp straight into the graph's tensor, the feathered elliptical
mask, the forward-mapped composite — and parked all four files at Athens' model tier because they had exactly one
caller. This is the slice that adds the second one.

It is therefore **a promotion before it is a model**. The project's rule is that a model-tier file a second model in
the family needs moves to family tier *in the change that adds that caller and before that caller is written*, and
`models/mod.rs` already names the four files and says which change moves them: *"Santorini promotes Athens' four"*.
Writing Santorini's pipeline first and moving them afterwards is exactly how a "temporary" copy becomes permanent.

What is left after the promotion is small, and that is the point. Santorini's graph differs from Athens' in one
respect — **it takes the image alone** — so the shared pipeline is one contract with an `Option` weight, matched at
the point the tensor is built, which is where the reference's `if fidelity >= 0` branch already sits. Everything
else it needs is already written and already tested.

## What Changes

**The four geometry files and the pipeline move to family tier, before Santorini is written.**

- `models/face_recovery/athens/{transform,warp,mask,composite}.rs` become
  `models/face_recovery/{transform,warp,mask,composite}.rs`. A file move and an import change, which is what slice 2
  wrote them to be: none names a variant and each takes the tile size as a parameter. `models/mod.rs`'s list of
  candidates parked at model tier loses these four and keeps Osaka's five.
- Athens' pipeline body — the `Model`/`ImagePipeline` impls, the per-face loop, the depth dispatch and the
  full-frame carry — becomes `models/face_recovery/restore.rs`, the one restoration contract both variants run.
  `models/face_recovery/athens/mod.rs` is left holding what is genuinely Athens': its measured profile and the prose
  behind it.
- The tile size and the graph's `[-1, 1]` normalisation move with it, as family-tier constants. Both graphs are
  exported at 512 and both standardise, so they are one value each today; the geometry takes the square as a
  parameter, so a future variant at another size is a per-variant value **at that point** and not before.

**Santorini runs.**

- `models/face_recovery/santorini/mod.rs`: its measured provider profile and the prose behind it, and the
  `FaceRecoveryVariant::profile` arm that reaches it in place of `EpProfile::default()`.
- **The weight is bound or it is not, decided by the variant row.** `FaceRecoveryVariant` answers what weight a run
  binds: Athens' carried fidelity, or `Fidelity::MAX` where the operation carries none, and `None` for Santorini.
  The pipeline takes the resolved `Option<f32>` and matches on it once — `Backend::run_weighted` with a value,
  `Backend::run_graph` without. This is the reference's `variant.Fidelity` of `1.0` or `-1`, without the sentinel.
  No session call is added: the fourth was slice 2's and the single-input declared-shape call has been there since
  Osaka.
- **The family's contract seam stops being able to refuse.** `FaceRecovery::pipeline` becomes one arm over both
  variants and returns a `Shared<B>` rather than a `Result`; `Operation::pipeline`'s face-recovery arm wraps it.
  The inaccurate refusal slice 2 kept deliberately — `FamilyNotImplemented` on a family that ran its other
  variant — goes away with the thing it described, exactly as slice 2 said it would. **The requirement that made
  the deferral per operation rather than per family stays exactly as it is**: no operation in the catalogue is in
  that state today, but each of the five unwritten families has two or three variants, so the next family to be
  written lands one of them first and is in it again. It is a contract that is temporarily uninstantiated, not a
  dead one.

**Santorini's measured provider tuning**, currently `EpProfile::default()` — CoreML asked to specialize the graph
for latency (`FastPrediction`), and sequential execution. Transcribed from `santorini.go` with its conditions
intact. Two things about it are load-bearing and both are what a reader tidying the two variants into agreement
would get wrong:

- **It is the first model in this catalogue to declare a specialization strategy.** Worth 1.04x at FP16 on an M2 Max
  and a tie at FP32 in steady state, at the price of 20-30ms of cold start per FP32 session build — which is paid
  once per session and amortised by a resident model.
- **It deliberately does *not* restrict CoreML's compute units, where Athens does.** Carrying Athens' rule over
  costs about 9%: Santorini's FP16 graph runs FP16 through its upsampling path, so the Neural Engine takes it whole,
  where CodeFormer's normalisation and transpose mix thrashes on it. The difference is the export, not the
  architecture — before these weights were re-exported their Resize nodes were held at FP32 with 84 Cast nodes
  wrapping 42 upsamples, and Athens' rule looked right for them too.

Its CUDA figures are recorded as prose rather than as settings, because both of the flags that look like free wins
**break this graph**: `prefer_nhwc` fails at `Run` — 23 of its 95 convolutions take a weight computed by a `Reshape`
rather than an initializer, which is the modulated-convolution hazard the profile's own documentation describes —
and `fuse_conv_bias` costs 1.4ms at FP32 and fails in the cuDNN frontend at FP16.

**No `perf` work.** Slice 2's detection pass was written over the family rather than over Athens: `perf` already
resolves Santorini rows, already feeds them the faces from the one detection pass, and already fails them when there
are none. Those rows stop being refused and start being measured with no line changed.

### Deliberate divergence from the reference

**None that is new.** Santorini inherits slice 2's three, and each was argued there: the whole progress range belongs
to the faces because detection is a separate operation here; the result is RGB at the run's requested depth including
over an empty selection; and the warp samples the source at full precision straight into the float tensor. Nothing in
this slice adds a fourth.

The structural differences from `santorini.go` are exactly the ones the project expects and does not justify: the
`Fidelity: -1` sentinel is an `Option` (slice 1), `Inputs: []string{...}` is a match on that `Option` rather than a
list of tensor names, and the per-variant `TileSize` field is a family-tier constant for as long as both variants
agree on it.

### Infrastructure: what `rust-sak` already covers

**Nothing applies, and nothing is missing.** This slice moves existing files, adds one provider profile and matches
on an `Option` at a call site. There is no new arithmetic, no new I/O, no new dependency, and nothing here is
general-purpose to another project. Its `image`, `crypto`, `fetch`, `fs`, `memo` and `sysinfo` modules have no
bearing on any of it. **No new third-party crate**, and none was weighed.

### Task estimate

**12.** The file moves and the tier list, the shared pipeline, the weight resolution and the two-call match,
Santorini's profile, the seam, four test tasks, and the toolchain pass.

## Capabilities

### New Capabilities

None. Every behaviour here is a requirement the family already states, now true of both its models.

### Modified Capabilities

- `face-recovery`: what a run does where its model takes **no** fidelity weight — nothing is bound, and the same
  photograph at the same selection is one result rather than two. And a new requirement that the family's models
  restore through the same alignment, the same feathering and the same bound, so that choosing between them changes
  the restoration and nothing about where a face is framed or how it is blended back.
- `execution-providers`: Santorini's measured settings join Osaka's, New York's and Athens' as a pinned
  requirement — including the compute-unit restriction it must **not** inherit from Athens — and the requirement on
  what a model may declare is widened to admit how a provider compiles the graph for the device, which is a trade
  between session build and steady-state latency rather than a straight win.

## Impact

**Crate: `opai` alone.** No `perf` — see above. No `gui`, no `cli`: neither front end reaches inference yet.

- **Moved** — `models/face_recovery/athens/{transform,warp,mask,composite}.rs` to
  `models/face_recovery/`, with their tests.
- **`crates/opai/src/models/face_recovery/restore.rs`** — new: the shared pipeline, promoted out of
  `athens/mod.rs`, over a resolved `Option<f32>` weight.
- **`crates/opai/src/models/face_recovery/athens/mod.rs`** — reduced to the profile and its prose.
- **`crates/opai/src/models/face_recovery/santorini/mod.rs`** — new: the profile and its prose.
- **`crates/opai/src/models/face_recovery/variant.rs`** — Santorini's `profile` arm, and where a run's weight is
  resolved from the variant and what the operation carries.
- **`crates/opai/src/models/face_recovery/mod.rs`** — the seam becomes one arm and stops returning a `Result`.
- **`crates/opai/src/models/operation.rs`** — the face-recovery arm wraps an infallible seam.
- **`crates/opai/src/models/mod.rs`** — the promotion list loses four entries.
- **`crates/opai/tests/public_api.rs`** — Santorini joins the operations that are served rather than refused.

**And four files the seam change reaches rather than targets** — three carry prose that counted the refused families,
and the fourth held a field down until a model measured it:

- **`crates/opai/src/inference/process.rs`** — the test predicate that had to be asked per operation rather than per
  family goes back to being answerable per family. **Test code only**; `inference/` gains no file and loses none.
- **`crates/opai/src/error.rs`** — `FamilyNotImplemented`'s and `Unsupported`'s doc comments named "every family but
  upscale", which face recovery stops being an instance of. No arm is added or removed.
- **`crates/opai/src/lib.rs`** — `Opai::process`'s "what is refused" section, for the same reason.
- **`crates/opai/src/providers/profile.rs`** — `CoreMlSpecialization::FastPrediction` loses its
  `expect(dead_code, …)`, whose stated reason was that the first model to measure it arrives with its family's
  pipeline. This is that model, so the attribute would now fail the build it was added to keep clean.

**Public API: additive, and smaller than slice 2's.** `Opai::process` gains the last operation it was refusing;
no signature changes, no type is added or removed, and `UnsupportedReason` loses no arm — the five deferred families
still answer it.

**The riskiest part is the promotion, not the model.** Moving a pipeline that four tested files feed and then
widening it is the kind of change that passes its own tests while quietly altering Athens: a weight resolved in the
wrong place would send a deserialized Athens operation down the single-input path, which fails at the runtime, or
send Santorini down the weighted one, which fails the same way. Both directions are pinned by tests against a fake
backend with no runtime at all.
