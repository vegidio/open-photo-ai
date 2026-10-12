## Why

São Paulo is the last variant of the colour balance family and the only one left that
[`ColorBalance::pipeline`](crates/opai/src/models/color_balance/mod.rs) refuses by name. Slice 2 put everything this
family shares where São Paulo can reach it from its first line — the cropped CHW→HWC sample reader with its plane
offset, the solver already written to take several destinations — and slice 1 put the plan, the presentation and the
per-run blend a tier above that. What is left is the part that is São Paulo's alone, and it is the part that makes
this model a second *contract* rather than a second set of weights: its graph does not return a corrected photograph.
It returns three weight planes and two synthesized renderings, and the answer is a per-pixel blend of three
illuminants rather than one global correction.

This is **slice 3 of three, and it closes the family.** After it, `cb_saopaulo_fp32` and `cb_saopaulo_fp16` produce a
photograph, colour balance refuses nothing, and the family's contract seam serves both of its arms — which is the
state light adjustment reached when Lyon landed.

## What Changes

**São Paulo, the model — `models/color_balance/saopaulo/`.**

- **`mod.rs`** — the **656** square its graph was exported at, the **channel layout** its output carries, the profile
  measured for it, and the prose behind all three. The layout is the contract between the conversion script and this
  code: `3` settings (daylight, shade, tungsten), `2` synthesized renderings, `3 + 3×2 = 9` output channels, weight
  planes first and then one three-channel rendering per synthesized setting. Naming it is what lets a test pin the
  layout without opening a session; getting it wrong means reading the weight planes out of the wrong offsets and
  rendering nonsense from correctly loaded weights.

  **The 656 is the same number as Rio's and is not the same decision**, which is why it lands in this model's own
  file rather than being inherited. Rio's square is tradeable because its graph's output is only ever sampled into an
  11-term fit. Half of this graph's output is *shown*: the weight map is upsampled to full resolution and multiplies
  the image, so its size caps how finely the blend can follow an illuminant boundary — dropping to 512 costs 3.7 dB
  and to 384 costs 7.6 dB.

- **`weights.rs`** — the weight map's own geometry: the per-axis bilinear tables that map each of the photograph's
  columns and rows back onto the un-padded region of the canvas, and the read of one canvas-resolution plane through
  them. **Align-centres**, clamped at the edges, and the convention is load-bearing rather than a detail: the weight
  map is the un-padded region of a canvas built by a pixel-centre-aligned resample, so centre alignment is the
  geometrically consistent inverse of the resize that produced it and align-corners would shift against it by up to
  half a pixel. Inside the graph the convention is the opposite and has to be — there it must match how the
  checkpoint was trained — but nothing downstream of the graph is trained.

- **`process.rs`** — São Paulo's contract, and São Paulo's alone. Present onto the square, one graph run of nine
  channels out of three in, read the un-padded source view and one destination view **per rendering** back out of the
  two tensors, fit **both mappings in one solve**, and render the photograph in **one fused full-resolution pass**:
  the polynomial features built once per pixel and reused across both mappings, the daylight rendering taken as the
  source itself, each synthesized rendering clamped into gamut *before* it is weighted, and the three weighted by the
  bilinearly upsampled map. Then the photograph moved towards that result by the run's [`Bias`] through
  `inference::mix::blended`.

**The seams.**

- `ColorBalanceVariant::canvas()` and `profile()` — São Paulo's arms stop answering a literal `656` and
  `EpProfile::default()` and reach the model's own file, the shape Rio's already has.
- `ColorBalance::pipeline` — the family's contract seam **loses its refusal**. Both arms serve. The `Result` stays,
  for the reason slice 2 recorded when it introduced it: a seam that cannot refuse is the exception here rather than
  the rule, and re-widening a signature later is a worse trade than carrying an `Ok` for one arm.
- `models::color_balance::live` — São Paulo joins the live check, and the section explaining why it was absent goes
  with it.

**No new public API and no new error variant.** `InferenceError::ColourMapping` already exists and already covers
this model's fit; the only outward change is that two operations which were refused now run.

### Deliberate divergence from the reference implementation

**Two, and both are the family's rather than this model's.**

- **The per-run control is a [`Bias`] on `-1.0..=1.0`** where the reference's `intensity` is `0..1`. Already shipped,
  already specified, unchanged here.
- **The result carries the depth the caller asked for.** The reference's `blendWeighted` writes into an
  `image.NewRGBA`, so a 16-bit photograph comes back quantised to 256 levels per channel whatever was asked for. The
  fused pass here is monomorphised over the channel type and dispatched once on the requested depth, as Rio's apply
  is. This is the family's existing specified behaviour and the reference is the one that diverges from it.

**Everything else matches the reference's behaviour**, including every figure that decides it: the 656 square, the
nine-channel layout and its order, the crop before the fit, one solve for both renderings, the features built once
per pixel, the **clamp on each rendering before it is weighted** rather than on the blend afterwards, align-centres
bilinear upsampling of the weight map, and no renormalisation of the weights.

### The parity gap this slice knowingly leaves open

**The fused pass runs on one thread.** The reference splits it across cores in row bands; that band loop is a
performance property and changes no pixel — the rows are strictly independent. It is left out because the only
parallel CPU pass in this crate, `models/upscale/osaka/colorfix.rs::for_each_row`, sits at model tier and is written
against `&mut [f32]` planes, so using it here means promoting and generalising a shipped model's file ahead of this
slice. That is the doctrine's own answer to a second caller, and it is a trade this slice declines: it costs roughly
0.5–1 s on a 24-megapixel photograph and nothing at all on the picture. Recorded here, with the promotion path named,
so a later change picks it up rather than rediscovering it. Every other full-resolution pass in this crate — Rio's
apply, face recovery's composite, light adjustment's — is serial for the same reason.

**Inherited from the reference rather than introduced here:** upstream's optional post-processing, a fast bilateral
solver that refines each weight channel against the image and renormalises, is not implemented. It cannot be
expressed in ONNX, the reference skips it too, and skipping it costs a little edge snapping at illuminant boundaries.

### Task estimate

**12**, against the 15-task budget. Slice 2's design projected 11; the extra one is the crate-wide `served`
predicate in `operation.rs`'s own test, which that estimate folded into the family seam and which is separate here
because it is the statement of *which operations the library runs* rather than a line of this family's.

### Infrastructure: what `rust-sak` already covers

**Nothing applies, and nothing was expected to** — the same finding slice 2 recorded, for the same reason. This slice
adds no hashing, HTTP, retries, downloads, extraction, temp files, config directories, image codecs, caching,
telemetry or hardware probes. What it adds is a separable bilinear resample of a single float plane and a per-pixel
polynomial blend; `rust-sak`'s surface — `crypto`, `fetch`, `fs`, `image`, `image-raw`, `memo`, `o11y`, `sysinfo` —
holds no such thing, and should not: this resample exists to be the exact inverse of the alignment convention of the
resize that built the canvas, which is a property of this pipeline rather than of a general image library.

**No new third-party crate either.** Specifically **not** `image`'s own `Triangle` filter for the weight map: it is a
support-based filter with its own normalisation and its own alignment, not the four-tap align-centres read the
reference's convention requires, so it would be a different filter reached for because it was nearby. `image` is
already used for the resample that *builds* the canvas, through slice 1's `presented`, which is the one place a
general resampler is the right answer.

## Capabilities

### Modified Capabilities

- `color-balance`: the family's two contracts, where the spec today describes one. **Three requirements change** —
  the one that says the result is the photograph re-coloured rather than the model's output enlarged (generalised
  from *a single global mapping* to *a colour transform the model contributes*, which for a mixed-illuminant model is
  one mapping per rendering plus a predicted weight map); the one that names which variants run (São Paulo stops
  being refused); and the extension-discarded requirement, which gains the weight map. **One requirement is added**:
  what a mixed-illuminant run does — the daylight rendering being the photograph itself, each synthesized rendering
  clamped into gamut before it is weighted, the weight map carried to the photograph's own resolution, and the
  weights needing no renormalisation.
- `execution-providers`: one requirement added — **São Paulo's measured settings**, which are the provider defaults
  at both precisions. That is a measurement rather than an omission, and it is the requirement that forbids carrying
  Rio's answer sideways to the model sitting next to it in the same family.

## Impact

**Crates touched: `opai` only.** `gui` reaches the library through `Opai::process` and the catalogue and has no line
to change. `cli` has none. `perf` already builds a colour balance operation from a codename, a precision and a bias —
`crates/perf/src/select.rs` has the arm — so the confirming sweep needs no code at all.

| File | Change |
| --- | --- |
| `crates/opai/src/models/color_balance/saopaulo/mod.rs` | new — `CANVAS`, the channel layout, `profile`, and the prose behind all three |
| `crates/opai/src/models/color_balance/saopaulo/weights.rs` | new — the align-centres bilinear axis tables and the weight-plane read |
| `crates/opai/src/models/color_balance/saopaulo/process.rs` | new — São Paulo's contract and its fake-backend suite |
| `crates/opai/src/models/color_balance/variant.rs` | São Paulo's `canvas()` and `profile()` arms reach `saopaulo` |
| `crates/opai/src/models/color_balance/mod.rs` | `pipeline()` — the refusal arm becomes São Paulo's contract; the module declaration |
| `crates/opai/src/models/color_balance/live.rs` | São Paulo's live check; the "why São Paulo is not here" section goes |
| `crates/opai/src/models/operation.rs` | **tests only** — `served` gains São Paulo |
| `crates/opai/src/inference/process.rs` | **tests only** — `an_operation_with_no_pipeline_is_refused_by_name`'s `served` match gains São Paulo |
| `crates/opai/src/models/color_balance/mapping.rs` | `evaluate` split into `kernel` + `evaluate_kernel` so the fused pass builds the features once (design D5); `evaluate`'s answer unchanged, pinned bit for bit by a new test |
| `crates/opai/src/models/color_balance/samples.rs` | **untouched** — this slice is its second caller, which is what slice 2 placed it for |
| `crates/opai/src/inference/present.rs`, `mix.rs` | **untouched** — their third and second callers respectively |

**Public API.** Two operations that used to be refused now run. Nothing added, removed or re-spelled, so
`crates/opai/tests/public_api.rs` does **not** change — and that file *not* changing is the mechanical sign this
slice is invisible from outside the crate, which slice 2's was not.

**Risk is where it has been for this whole family: every failure mode is a plausible photograph and no error** — and
this slice adds a new one that the previous two did not have, because the weight map is *shown* rather than sampled.
A weight plane read at the wrong offset, a rendering read as a weight, the axis tables aligned at corners instead of
centres, the clamp moved from each rendering to the blend, the two renderings swapped against their weight planes:
none of them is reportable and each produces a photograph. The mitigations are the three this family already has, and
the first of them is where the new risk lands: the layout and the upsample are tested against hand-laid tensors where
a wrong answer is a wrong *number*, the fake-backend suite drives the whole contract with no runtime so every CI
platform runs it, and the live check against the real weights is run by hand because only a real graph proves a real
photograph comes out.
