## Context

See proposal.md for why. Three facts about the surrounding code shape everything below.

**Colorization is a whole-image family, not a tiled one.** The published graphs take `1x3x512x512` (Delhi, Mumbai,
returning `1x2x512x512`, the a/b planes) and `1x3x560x560` (Jaipur, returning `1x3x560x560`, RGB). The reference
stretches the photograph to that square, runs it once, and uses only the chroma. The detail comes from the photograph's
own L at full resolution. So the Lab conversion runs twice per photograph pixel: once to read L, once to convert back to
sRGB. It is the hottest arithmetic in the family, and that is why the reference carries fast paths for it.

**Every pixel already arrives as a 16-bit sample.** `inference::tensor::Sampler::rgb` returns `[u16; 3]` for every
source variant: an 8-bit channel `v` widened to `v * 257`, and straight alpha premultiplied against black as the
reference does. The reference's 8-bit linearization table is built from exactly that number, `float32(v*257) / 65535`.
So a table keyed by the `u16` sample is the reference's table at 8 bits, and it is complete at 16.

**`Channel::from_unit` is this crate's single rounding rule.** Every family writes a result channel through it: clamp to
`[0, 1]`, multiply by the channel's maximum, round to nearest. The reference truncates `x*255 + 0.5`, which is the same
rule on non-negative input except at float-addition ties.

## Goals / Non-Goals

**Goals:**

- A Lab module that slice 2's compose and its three graph adapters use as they are, with no signature added or changed
  there.
- 8-bit results that are the reference's value for value, and 16-bit results computed at 16-bit precision.
- Every fast path proven equal to the general path it replaces, on every CI platform, with no model, runtime or GPU.

**Non-Goals:**

- **Anything that reads a photograph or a tensor.** The stretch to the square, the gray and luma inputs, the chroma
  extraction, the upsample and the fused compose are slice 2's. This slice ends at per-pixel scalar functions.
- **The ITU-601 luma Jaipur's input uses.** It is `0.299 R + 0.587 G + 0.114 B` over gamma-encoded channels: a weighted
  sum, not a colour-space conversion. It belongs beside the adapter that uses it, in slice 2.
- **Measuring the 16-bit path's cost.** It needs slice 2's compose to be a meaningful number (see "Deferred").

## Decisions

### D1. `models/colorization/lab.rs`, at family tier

Colorization is the only caller: its compose, its Ab input and Jaipur's chroma extraction. `config.yaml` puts a file
with one family's callers under that family. `osaka/colorfix.rs`, the other colour-correcting code in the crate, was
checked and shares nothing: it is an à trous wavelet low-frequency swap in RGB, with no colour space involved.

**Named `lab`**, because a module named `color` would invite everything with "colour" in the name. What it holds is the
sRGB transfer and CIELab. The first is only here because the second needs it.

**Rejected: `inference/`.** It would be a promotion with no second caller, which the tier rule forbids in both
directions. **Rejected: `rust-sak`.** See proposal.md: one caller, and the constants are tuned to what colorization
graphs were trained on.

Visibility is `pub(super)` throughout. The callers are `models/colorization/process.rs` and the variant adapters in
slice 2, all inside the family.

### D2. One linearization table, keyed by the `u16` sample

```
  reference                               here
  srgbLinearLUT [256]float64              LINEAR: LazyLock<Box<[f64; 65536]>>
    index: the 8-bit byte v                 index: the u16 sample Sampler::rgb returns
    value: srgbToLinear(f32(v*257)/65535)   value: srgb_to_linear(f64::from(w as f32 / 65535.0))

  8-bit source:   w = v * 257   -->  LINEAR[w] == srgbLinearLUT[v]    bit for bit
  16-bit source:  every w       -->  LINEAR[w] == srgb_to_linear(..)  bit for bit
```

Two functions read the table:

- `lightness([u16; 3]) -> f32`, the L half of the forward conversion, which the compose calls once per photograph
  pixel;
- `gray([u16; 3]) -> f32`, `LabToRgb(L, 0, 0)` collapsed to one gamma encode of Y, which Delhi's and Mumbai's input
  is built from.

Both are exactly the reference's `RgbToLabLBytes` and `GrayFromRgbBytes` at 8 bits, and remain exact at 16.

**The `f32` round trip inside the index is deliberate.** `w as f32 / 65535.0` is the value `rgb_to_lab` receives from
a caller holding unit floats. Computing the table through the same expression is what lets `lightness` match
`rgb_to_lab`'s L **bit for bit**, which the reference pins and this slice pins too.

**Cost:** 512 KiB, built once on first use by 65,536 `powf` calls. That is well under a millisecond per core, and it is
only paid by a process that colorizes something. `LazyLock` rather than a build-time constant, because a 65,536-entry
float literal in source is unreviewable and `powf` is not `const`.

**Rejected: the reference's 256-entry table, plus the direct transfer for 16-bit sources.** It gives two input paths
that have to be dispatched on depth and tested against each other. It also means the path RAW photographs take (they
decode at 16 bits) is the one without a table. **Rejected: an `f32` table** at half the memory. The reference's
entries are `f64`, and narrowing them moves L by up to an `f32` ulp: small, but not the reference.

### D3. Two encodes, one rounding rule

```
  linear-light f64  ---+--> srgb_u8(c)   threshold search over 255 step boundaries   (8-bit result)
                       |
                       +--> srgb_u16(c)  u16::from_unit(linear_to_srgb(c) as f32)    (16-bit result)
```

- **`srgb_u8`** is the reference's `SrgbByte`: a binary search over the 255 linear-light values at which the encoded
  byte steps. It replaces a `powf` per channel with eight comparisons. The thresholds are found by bisecting the `f64`
  bit pattern (for positive floats, bit order is value order), in a `LazyLock`. That is about 16,000 evaluations at
  first use, the job the reference's `sync.OnceFunc` does for the same startup-cost reason.
- **The thresholds are bisected against `u8::from_unit(linear_to_srgb(c) as f32)`**, this crate's own general encode,
  not the reference's `uint8(Clamp255(x*255 + 0.5))`. Then `srgb_u8(c) == u8::from_unit(linear_to_srgb(c) as f32)` for
  every `c` by construction, and the test is the reference's boundary argument restated against that. The two rounding
  rules agree on every non-negative value except an `f32` addition that rounds `x.4999999 + 0.5` up. The boundary test
  pins whichever this crate's rule produces, so the question never has to be answered by reasoning.
- **`srgb_u16`** is the direct transfer. A threshold table for 65,535 steps would be about 4 million evaluations to
  build: a startup cost larger than the work it saves on one photograph. The 16-bit path therefore pays one `powf` per
  channel.

**No generic `encode<T: Channel>` in this slice.** Slice 2's compose is where the depth dispatch happens, once per run,
as `run_tiled` and light adjustment's `adjusted` already do it. A trait method added here would be a second dispatch
mechanism written before the first caller exists. Slice 2 can add one over these two functions if its compose reads
better with it.

### D4. What is ported, and in what form

| Reference (`internal/utils/color.go`) | Here | Used by (slice 2) |
| --- | --- | --- |
| `labXn`, `labZn`, `labT0`, `labK`, `labF`, `labFInv` | private constants and functions | everything below |
| `srgbToLinear`, `linearToSrgb` | private `srgb_to_linear`, `linear_to_srgb` | the table, the encodes |
| `RgbToLab` | `rgb_to_lab([f32; 3]) -> [f32; 3]` | Jaipur's chroma extraction |
| `LabToLinearRgb` | `lab_to_linear_rgb(l, a, b) -> [f64; 3]` | the compose |
| `LabToRgb` | `#[cfg(test)]` `lab_to_rgb` | the tests' oracle only; nothing in the reference's production path calls it |
| `RgbToLabL`, `RgbToLabLBytes` | `lightness([u16; 3]) -> f32` (D2) | the compose |
| `GrayFromRgbBytes` | `gray([u16; 3]) -> f32` (D2) | Delhi's and Mumbai's input |
| `SrgbByte`, `srgbByteSlow` | `srgb_u8`, pinned against `u8::from_unit` (D3) | the compose, 8-bit |
| — | `srgb_u16` (D3) | the compose, 16-bit |
| `RgbSaturation`, `RgbToHsv` | **not ported** | the reference's autopilot uses them, colorization does not; out of this family's scope |

Arrays rather than tuples for the three channels, matching `Sampler::rgb`'s `[u16; 3]` and every other colour value in
the crate.

The reference's prose travels with the code: the OpenCV float convention and the 8-bit encoding it is not, the two
properties that make the fast paths exact, why the gray rendering collapses to one encode, and why the table is built
lazily.

### D5. The tests

All eight of the reference's, ported against the Rust signatures, plus three the 16-bit path owes:

| Test | Asserts |
| --- | --- |
| OpenCV reference values | black, white, red, green, blue and mid-gray within 0.05 of OpenCV's float Lab. This is the test that catches the 8-bit encoding. |
| round trip | a 17-step RGB grid through Lab and back, within `1e-3` |
| clamping | out-of-gamut Lab comes back inside `[0, 1]`, never NaN |
| gray has no chroma | every 8-bit gray level has `|a|, |b| <= 0.01` |
| `lightness` is `rgb_to_lab`'s L | bit for bit, over the reference's coprime-strided byte sweep **and** a 16-bit sweep that includes non-`v*257` samples |
| `srgb_u8` at every boundary | at and just below all 255 thresholds, out-of-range inputs, and a 200,001-point dense sweep, each equal to `u8::from_unit(linear_to_srgb(c) as f32)` |
| split conversion | `lab_to_linear_rgb` then `srgb_u8` equals `lab_to_rgb` then `u8::from_unit` over the reference's L/a/b grid |
| `gray` against the Lab round trip | within `1e-4`, gray diagonal exhaustively plus the strided cube |
| *new:* the table is the transfer | `LINEAR[w] == srgb_to_linear(f64::from(w as f32 / 65535.0))` for all 65,536 `w` |
| *new:* 8-bit entries are the reference's | `LINEAR[v * 257]` equals the reference's table formula for all 256 `v` |
| *new:* `srgb_u16` inverts the table | `srgb_u16(LINEAR[w]) == w` for every `w`. The 16-bit encode loses nothing it was handed. |

The last one is the 16-bit counterpart of the reference's neutral-axis concern. A 16-bit photograph run through the
compose with zero chroma must come back unchanged, and this is that property at the scalar level.

### Session ownership, providers and crates

**No session, memory budget or execution provider is touched.** The module is pure scalar arithmetic with two
process-wide immutable tables. Each is built once behind a `LazyLock` and never freed, which is `Send + Sync` for free
and safe to read from slice 2's row-banded compose threads with no coordination.

**`rust-sak`:** no feature used. The gap is filled locally; see D1 and the proposal. **No third-party crate added.**
`LazyLock` is `std`.

**Division across crates:** all of it in `opai`. `gui`, `cli` and `perf` are untouched.

## Slices

Colorization, in order. **This is slice 1.** Slice 2 is proposed when it is reached.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-colorization-1-lab` | `models/colorization/lab.rs`: the sRGB transfer, OpenCV-convention CIELab both ways, the `u16`-keyed linearization table with `lightness` and `gray`, the 8-bit threshold encode and the 16-bit direct encode, and the reference's colour tests plus the 16-bit ones. No behaviour change | – | **current** (est. 8) |
| 2 | `add-colorization-2-pipeline` | The stretch to the graph's square (Lanczos3 for Ab graphs, bilinear for Jaipur), the gray-Lab and ITU-601-luma inputs, the a/b and RGB-to-a/b chroma extraction, the fused bilinear compose onto the photograph's own L at the run's depth (row-banded on `std::thread::scope`), with a separate-steps oracle it is tested against; `process.rs` and `Colorization::pipeline`; `delhi/`, `mumbai/`, `jaipur/` with their graphs and ported profiles; `ColorizationVariant::graph()` and `profile()`; the `Operation::pipeline` refusal arm removed; **`UnsupportedReason::FamilyNotImplemented` deleted** with the refused-family tests and prose; the fake-backend suite and live check. Specs: `colorization` (new), `execution-providers` (modified), and whichever spec states the refusal reason | 1 | pending (est. 14) |

### Decisions settled in exploration that slice 2 inherits

Recorded here so slice 2 takes them from a document rather than from a conversation.

- **No tiling.** One graph run on the whole photograph stretched to the graph's square. Tiling would colorize each tile
  without the scene's context, so neighbouring tiles would disagree about what they share. Blending the overlap does not
  fix a disagreement.
- **Stretched, not fitted.** `inference::present` keeps the aspect ratio and reflection-pads, and it is **not** what
  this family uses. The reference's `imaging.Resize(img, size, size, filter)` distorts the aspect ratio to fill the
  square, and the chroma is upsampled back with the inverse distortion.
- **Two graph contracts, as data on the variant.** A two-arm `Graph::{Ab, Rgb}` returned by an exhaustive
  `ColorizationVariant::graph()`, as sharpen's `guard()` is: Delhi and Mumbai `Ab` (512, Lanczos, gray-Lab input, 2
  output channels), Jaipur `Rgb` (560, bilinear, ITU-601 luma input, 3 output channels converted to a/b).
- **The run's depth is kept** (proposal, and D2/D3 here), and alpha is not carried through, as in every other
  whole-image family.
- **All three profiles are ported, not re-measured:** CoreML `CpuAndGpu` at FP16 and the default at FP32, each with its
  own prose from `delhi.go`, `mumbai.go` and `jaipur.go`. Jaipur's is measured on its own graph, not inherited from the
  other two, and its prose says so.
- **`UnsupportedReason::FamilyNotImplemented` is deleted, not kept.** Nothing produces it once colorization runs. It
  is a public variant, so `tests/public_api.rs` changes. `perf/src/sweep.rs`'s refusal test moves to
  `IncompleteGraphSet`. The refused-family tests in `models/operation.rs` and `inference/process.rs` are deleted rather
  than repointed, because there is no family left to point them at.
- **Autopilot is unchanged.** Its "every suggestion names a runnable family" test lists only face recovery and upscale.
  Only the doc comment naming `FamilyNotImplemented` changes.
- **Translucent pixels are read premultiplied against black**, which `Sampler::rgb` already does. This matches the
  reference: `Sample16`, which its compose calls for a straight-alpha pixel that is not fully opaque, multiplies each
  channel by `a / 255` to reproduce Go's `RGBA()`. The call-site comment in `process.go` says "un-premultiply", but the
  code premultiplies. Checked against the source, not the comment. In the reference, that pixel leaves the 8-bit table
  for the `powf` path. Here, D2's `u16` table covers it with no second path.

### Deferred to slice 2, because only slice 2 can measure them

None of these would change this slice's files or tasks.

- **Bilinear resamplers.** `image`'s `Triangle` filter against `imaging.Linear` for Jaipur's stretch, and the
  reference's align-centres `sampleAxis` for the chroma upsample. The same question colour balance deferred and then
  measured.
- **The 16-bit compose's cost.** One `powf` per channel per pixel on a 24-megapixel RAW, spread across cores. If it
  proves significant, a threshold search over a coarser 16-bit table is the escape hatch, and it would be added here in
  `lab.rs` without changing any caller.

## Risks / Trade-offs

- **[The 8-bit Lab encoding is ported instead of the float one]** → The OpenCV reference-value test is exactly the
  test that catches it. It is the first test in the module and is ported unchanged.
- **[A fast path drifts from the general path, and slice 2 renders a slightly wrong photograph with no error]** →
  Every fast path is pinned **bit for bit** against the general expression: the table at all 65,536 entries, `lightness`
  against `rgb_to_lab`, and `srgb_u8` at every one of its boundaries. None is pinned within a tolerance.
- **[The rounding rule differs from the reference's on a float tie]** → Accepted and deliberate (D3). A result here
  agrees with this crate's other seven families rather than with a Go idiom. The dense sweep reports any disagreement
  as an ordinary test failure, not as a silent difference.
- **[A 512 KiB table is resident for the rest of the process]** → Accepted. It is built only by a process that
  colorizes, and it is smaller than one 512x512 `f32` tensor of the graph it serves.
- **[Slice 1 delivers no user-visible value]** → Accepted, as `add-color-balance-1-canvas` accepted it. The project
  stays working throughout: colorization is refused before and after by the same arm. Slice 2 starts against proven
  arithmetic rather than debugging the conversion and the pipeline at once.
