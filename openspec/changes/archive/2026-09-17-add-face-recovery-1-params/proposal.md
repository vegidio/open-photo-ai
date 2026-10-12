## Why

Athens and Santorini are named but cannot run, and the two things standing between them and a pipeline are both
parameter problems rather than inference problems. Athens takes a **fidelity weight** the reference hard-codes to
`1.0`; exposing it is a per-variant parameter, and no family in this library has ever had one — `FamilyEntry` states
outright that every variant of a family takes the same parameters. Face recovery also takes a **set of faces**, which
the catalogue cannot describe at all, because `ParameterEntry` is a numeric range with a `min` and a `max`.

Underneath both sits a seam that was built and then never connected. Every family already publishes a `params()`
returning what a run needs beyond the operation's identity, and **nothing outside the test suite calls it** — each
pipeline reaches around it to the operation's fields instead. Wiring Athens' fidelity through a seam that is
decorative would make it decorative too.

This slice closes all of that, and takes the opportunity to fix the one fiction it uncovers: the eight hand-written
`PartialEq` impls that drop a per-run parameter so that "a registry keyed on an operation" reuses a loaded session.
There is no such registry. `SessionCache` keys on `(ArtifactId, ExecutionProvider)` and always has.

## What Changes

- **Add `Fidelity`**, a bounded, quantized parameter type alongside `Strength`, `Bias` and `Scale`. Range `0.0..=1.0`,
  where `1.0` is maximum fidelity to the original face — the value the reference hard-codes.
- **BREAKING — every model gains a per-variant constructor returning its carrier.** `Upscale::kyoto(precision, scale)`,
  `Denoise::stockholm(precision, strength)`, `FaceRecovery::athens(precision, faces, fidelity)`,
  `FaceRecovery::santorini(precision, faces)` — twenty in all. They return `Operation` (or `Analysis`; see below)
  rather than the family type, so a caller writes `opai.process(&picture, &[athens, kyoto], None)` with no
  `Operation::Family(…)` wrapping and no `.into()`. `Upscale::osaka` takes `OsakaPrecision`, so a precision that
  family does not publish stops compiling rather than being refused.
- **BREAKING — Athens' fidelity is unrepresentable on Santorini.** Only `FaceRecovery::athens` takes one;
  `FaceRecovery::fidelity()` answers `Option<Fidelity>`. Santorini's graph has no second input tensor, so there is
  nothing for a value to bind to.
- **BREAKING — a second carrier, `Analysis`, for the families whose result is not an image.** `Operation` carries the
  seven image families and is what `process` takes; `Analysis` carries detection and is what `execute` takes. A model
  constructor returns the carrier for the path it runs on, uniformly. Both compile-time properties survive: an
  `execute` result is still the operation's own type with nothing to unwrap, and an image operation still cannot
  reach `execute` at all.
- **BREAKING — a progress report's subject spans both carriers.** A second carrier means both run paths report, so
  `InferenceProgress.operation: Operation` becomes `subject: Subject`, where `Subject` is `Enhancement(Operation)` or
  `Analysis(Analysis)` and forwards the five questions every family answers. A front end that only labels a bar calls
  `Subject::display_name` and never matches at all. This is the carrier split reaching the one place that names an
  operation without running it; it is reporting vocabulary, and neither entry point takes one.
- **BREAKING — two operations are equal when every field matches.** The eight hand-written `PartialEq`/`Hash` impls
  are replaced by derives. `Denoise` at strength 0.5 and at strength 2.5 stop comparing equal, which is what a front
  end asking *did the user change anything?* needs. Nothing regresses: the session registry never keyed on this.
- **BREAKING — `Point` and `Confidence` quantize to two decimals on acceptance**, which is what keeps `Face`, and
  therefore `Operation`, `Eq` and `Hash` once faces are compared by value. Two decimals is already what
  `FaceRecovery::cache_tag` treats as the meaningful precision.
- **`params()` becomes load-bearing.** Each family's pipeline is constructed from its `Params` value rather than from
  the operation's fields — the upscale refactor that makes the seam real rather than declared.
- **BREAKING — `UpscaleParams.scale` stops being an `Option`.** It was `Option<Scale>` to express "the operation's
  identity does not carry this", which full value equality makes false: every operation now carries every field. What
  is left in a `Params` is what a *run* takes, and every upscale run takes a scale — the convolutional pipeline to
  pick its passes, the diffusion pipeline because one set of weights serves every scale.
- **Athens' fidelity enters the run cache tag** as a `-w<value>` segment, and Santorini's tag gains nothing. The same
  image at `fidelity = 1.0` is served from the cache; at `0.5` it is recomputed.
- **BREAKING — the catalogue publishes parameters per variant, and describes more than ranges.** `parameters` moves
  from `FamilyEntry` to `VariantEntry`, so fidelity can belong to Athens and not to Santorini. `ParameterEntry`
  grows a `ParameterKind` — `Range { min, max }` or `Faces` — so face recovery can publish that it needs a detection
  run's output instead of publishing an empty list indistinguishable from colorization's.

**Deliberate divergence from the reference.** Two, both behavioural:

- Fidelity is a per-run parameter rather than a variant constant. The reference's `athens.go` fixes it at `1.0` and
  `santorini.go` marks its absence with `-1`. A sentinel inside a variant row is how Go expresses "this graph has no
  second input"; here the absence is a type, and the value is the user's. `1.0` remains the default a front end
  offers, so an untouched control matches the reference exactly.
- Face coordinates are rounded on acceptance rather than carried at full `f32` precision. The reference does not
  round, and neither did this project until now — `face.rs` argues against it explicitly, and that paragraph is
  rewritten rather than quietly contradicted. The reference already treats two decimals as the meaningful precision
  in `FacesCacheKey`; this makes the value agree with the key instead of only the key agreeing with itself.

**`rust-sak`:** nothing applies. This slice adds no infrastructure — it is model vocabulary, bounded parameter types
and enum shape, all of which are this project's domain. `Fidelity` follows `Strength`, `Bias` and `Scale`, which are
already local for the same reason.

**Crates touched:** `opai`, and `perf` as the one crate that already consumes these types — removing the two
catalogue entry points takes its sweep selection with them. No front-end work; `gui` and `cli` do not consume any of
these types yet.

**Estimated tasks:** 15.

## Capabilities

### New Capabilities

None. Every behaviour here refines a family the library already names.

### Modified Capabilities

- `models`: an operation's identity becomes full value equality rather than "the same weights"; a per-run parameter
  moves inside the identity; a face quantizes on acceptance; face recovery carries an optional fidelity that only
  Athens can be given; the catalogue publishes parameters per variant and describes a non-numeric one; a catalogue
  row is turned back into an operation through a per-variant constructor; an operation of any family is now one of
  two carriers rather than one.
- `data-inference`: the bound that makes a non-image run's result type the operation's own is expressed through the
  `Analysis` carrier rather than a trait over the family type. The guarantee is unchanged and the shape is not. The
  same is true of what such a run reports: it reported before by wrapping its detection back into an `Operation`, and
  now carries it in a `Subject` that spans both paths, so no requirement here moves.
- `face-detection`: the coordinates a detection run reports are quantized to two decimals.

## Impact

- **`crates/opai/src/models/`** — every family module (`upscale/`, `denoise/`, `sharpen/`, `colorization/`,
  `light_adjustment/`, `color_balance/`, `detection/`, `face_recovery/`), plus `face.rs`, `catalogue.rs`,
  `operation.rs` and a new `fidelity.rs`.
- **`crates/opai/src/lib.rs`** — `execute`'s signature takes `&Analysis`; new re-exports for `Fidelity`, `Analysis`,
  `ParameterKind` and `Subject`.
- **`crates/opai/src/inference/progress.rs`** — the `Subject` carrier, and `InferenceProgress.operation` becomes
  `subject`. `ChainProgress::operation` takes an `impl Into<Subject>`, so a caller holding either carrier hands it
  over unchanged.
- **`crates/opai/tests/public_api.rs`** — every operation is constructed the new way, and the equality assertion at
  line 199 inverts.
- **`crates/perf/`** — the one crate already built on this API, and the only one this slice migrates. `select.rs`
  builds each swept row through the model's own constructor instead of the two removed catalogue entry points, and
  refuses detection now that it is carried in `Analysis` rather than in an enhancement chain; `cli.rs` gains a
  `--fidelity` flag and answers `None` for a parameter whose kind is not a range; `main.rs`, `view.rs` and
  `viewport.rs` carry a swept row as a `Subject`, which spans both carriers.
- **No pipeline is added.** Athens and Santorini remain unable to run; their pipelines are slices 2 and 3. This slice
  makes them expressible, cacheable and describable, and leaves the six deferred families exactly as they were.
