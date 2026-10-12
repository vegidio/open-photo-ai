## Why

Slice 1 proved the Lab arithmetic. What colorization still needs before it can run is everything between a photograph
and that arithmetic: the stretch to the graph's square, the two input renderings, the two ways chroma comes out of a
graph, and the fused compose that puts the predicted a/b onto the photograph's own full-resolution L. Most of that has
no model, runtime or GPU in it, and all of it can go wrong while still rendering a plausible photograph and raising no
error. So it is built and proven here, on its own, the way slice 1 was.

It is also where the tier rule bites. The compose reads a square chroma plane back onto the photograph through the
align-centres bilinear read the reference calls `sampleAxis`. That read already exists here as
`models/color_balance/saopaulo/weights.rs`, and both its header and the archived São Paulo design name colorization as
the second family that promotes it to `inference/`, in the change that introduces the caller and before that caller is
written. The compose's row-banded parallelism is the same situation, with `models/upscale/osaka/colorfix.rs`'s
`for_each_row`. Slice 1's series table put all of this into one `add-colorization-2-pipeline` change estimated at 14
tasks, without the promotions. Adding them takes it past the 15-task budget. So the series is re-cut: this change is
slice 2 of three, and `add-colorization-3-pipeline` wires it to the graphs.

This slice adds no model, no pipeline and no observable behaviour. Colorization is refused before and after it by the
same arm.

## What Changes

**Two promotions to `inference/`, unchanged in behaviour.**

- `models/color_balance/saopaulo/weights.rs` (`Tap`, `Axis`, `sample`) moves to `inference/bilinear.rs`. São Paulo is
  rewired to it, and its tests move without an assertion edited.
- `for_each_row` moves out of `models/upscale/osaka/colorfix.rs` into `inference/rows.rs`, generalised from `f32`
  rows to rows of any `Send` element with a caller-given row length. `colorfix.rs` keeps everything else that is its
  own, and its bit-identical tests pass unchanged.

**The colorization arithmetic, at the tier each piece's callers put it.**

- **The stretch** (`models/colorization/stretch.rs`, family tier). The photograph is stretched to the graph's square,
  not fitted: the reference distorts the aspect ratio and the compose undoes it. A source with alpha is first
  flattened against black. That makes an unweighted resample sum exactly what the reference's alpha-weighted
  `imaging.Resize` sums (design D3).
- **The Ab contract** (`models/colorization/ab.rs`, family tier, for Delhi and Mumbai): the 512 square, the Lanczos
  stretch, the gray-Lab input rendered through slice 1's `gray`, and the two a/b planes read straight out of a
  2-channel output.
- **The Rgb contract** (`models/colorization/jaipur/rgb.rs`, model tier, for Jaipur alone): the 560 square, the
  bilinear stretch, the ITU-601 luma input, and the a/b planes extracted from a clamped 3-channel RGB output through
  slice 1's `rgb_to_lab`.
- **The fused compose** (`models/colorization/compose.rs`, family tier). Per photograph pixel it computes `lightness`,
  bilinear a/b through `inference::bilinear`, `lab_to_linear_rgb`, and `srgb_u8` or `srgb_u16` **at the run's depth**,
  with rows split across cores through `inference::rows`. It is tested bit for bit against a separate-steps oracle
  ported from the reference's `process_reference_test.go`, at both depths and over every source layout.

**Two measurements slice 1 deferred to here**, each recorded in its task:

- `image`'s `Triangle` against `imaging.Linear` for Jaipur's stretch, and `Lanczos3` against `imaging.Lanczos` for the
  Ab stretch. Both pairs have the same kernel and support. The difference to be measured is `imaging`'s 8-bit
  intermediate between its two passes.
- The 16-bit compose's cost against the 8-bit one.

**Nothing calls the new colorization code outside its tests yet.** Each new module carries
`#[cfg_attr(not(test), expect(dead_code, reason = ...))]` naming `add-colorization-3-pipeline`, and `lab`'s existing
attribute is re-pointed there from the slice name it had.

### Deliberate divergence from the reference implementation

**None this slice can render.** Four choices take effect once slice 3 uses them, and are recorded here so they are not
discovered there:

- **The stretch is `image`'s resampler, not `imaging`'s.** The kernels are the same, but `image` keeps a float
  intermediate between its passes. `imaging` stores 8-bit straight alpha there, which rounds, clips overshoot and
  divides by the weighted alpha sum. So the graph's input differs from the reference's by up to 5 levels at a few sharp
  edges of an opaque photograph, and by up to 49 beside transparent regions, where `imaging`'s division is unstable
  under Lanczos's negative lobes. Measured and decided in design D4.
- **A 16-bit photograph is stretched at 16 bits.** The reference's `imaging.Resize` always produces 8-bit NRGBA. This
  is slice 1's "the run's depth is kept", applied to the graph's input.
- **A translucent pixel is flattened before the stretch rather than weighted during it.** The sums are the same (design
  D3). Only the rounding differs.
- **The compose's 8-bit rounding is slice 1's**, including the one threshold (byte 1) its completion note records.

### Infrastructure: what `rust-sak` already covers

**Nothing applies.** The work is resampling, colour arithmetic and a row split over `std::thread::scope`. `rust-sak`'s
`image` module is codecs and has no resampler, and none of `crypto`, `fetch`, `fs`, `image-raw`, `memo`, `o11y` or
`sysinfo` is relevant. **No new third-party crate.** The stretch is the `image` crate's `resize_exact`, which
`inference::present` already uses.

### Task estimate

**12** against the 15-task budget: three for the promotions and their prose, four for the stretch, the two contracts
and the resampler measurement, three for the compose, its oracle and its cost measurement, and two for the module prose
and the toolchain pass. Slice 3 is estimated at 12; see design.md for the series.

## Capabilities

### New Capabilities

None. This slice introduces no observable behaviour. The `colorization` capability is created by slice 3, the first
change that can run one.

### Modified Capabilities

None. `.openspec.yaml` sets `skip_specs: true`, as slice 1 did for the same reason. Both promotions change where code
lives, not what São Paulo or Osaka render, so `color-balance` and `image-enhancement` take no delta either.

## Impact

**Crates touched: `opai` only.** `gui`, `cli` and `perf` reach the crate through its public API, and nothing public
changes.

| File | Change |
| --- | --- |
| `crates/opai/src/inference/bilinear.rs` | new: `Tap`, `Axis`, `sample` and their tests, moved from São Paulo |
| `crates/opai/src/inference/rows.rs` | new: `for_each_row`, generalised and moved from `colorfix.rs` |
| `crates/opai/src/inference/mod.rs` | the two modules declared and described |
| `crates/opai/src/models/color_balance/saopaulo/{mod,process}.rs` | `weights` removed, imports re-pointed |
| `crates/opai/src/models/color_balance/saopaulo/weights.rs` | deleted |
| `crates/opai/src/models/upscale/osaka/colorfix.rs` | `for_each_row` removed, import re-pointed |
| `crates/opai/src/models/colorization/{stretch,ab,compose}.rs` | new, each with its tests |
| `crates/opai/src/models/colorization/jaipur/{mod,rgb}.rs` | new: the Rgb contract's adapters and tests |
| `crates/opai/src/models/colorization/mod.rs` | module declarations and `expect(dead_code)`s; header names the new files |

**Public API: unchanged.** Everything is `pub(crate)` or narrower, and `tests/public_api.rs` is untouched.

**The riskiest part is the two moves**, because a move that silently changed São Paulo's weight-map read or Osaka's
colour fix would still render a plausible photograph. Both suites passing with no assertion edited is the proof.
Everything colorization adds is checked on every CI platform with no ONNX Runtime, model file, GPU or network, apart
from the two measurements, which run by hand.
