## Why

Colorization is the last family `Operation::pipeline` refuses. Delhi, Mumbai and Jaipur can be named, catalogued,
downloaded, opened and cached, and they cannot run. Every other family's pipeline was built from arithmetic this crate
already had. Colorization's cannot be: its whole contract is a round trip through CIELab. The photograph's own
full-resolution L is kept, the model's predicted a/b is put onto it, and the result is converted back to sRGB. There is
no Lab conversion anywhere in `opai` today.

That conversion is also where this family can go wrong most quietly. Three failure modes each render a plausible
photograph and raise no error:

- using the 8-bit Lab encoding (L scaled by 255/100, a/b offset by 128) where the graphs were trained on OpenCV's float
  convention;
- an L-only fast path that drifts from the full conversion;
- a gamma encode that rounds one way here and another way there.

The reference pins each of these with its own test. This slice ports the conversion and those tests on their own, ahead
of the pipeline that needs them. The pipeline slice can then be built against arithmetic that is already proven.

This is slice 1 of a two-slice series. It adds no model, no pipeline and no observable behaviour.

## What Changes

**`models/colorization/lab.rs`**: new, at colorization's family tier. It is the reference's `internal/utils/color.go`
colour-space half, ported with its constants and its reasoning:

- The sRGB transfer in both directions, and CIELab forward and inverse, with OpenCV's D65 constants and its
  linear-segment approximation below the cube-root threshold. L is on `[0, 100]` and a/b are zero-centred.
- `lab_to_linear_rgb`: the inverse stopped one step short, unclamped and in `f64`, for a caller that finishes with its
  own encode.
- **The luminance-only path** that the full-resolution compose needs. L depends only on Y, so two of the three cube
  roots and both chroma terms are dead work there.
- **The gray rendering** Delhi's and Mumbai's input is built from: `LabToRgb(L, 0, 0)` collapsed to one gamma encode of
  Y, as in the reference.
- **One linearization table, keyed by the 16-bit sample** `Sampler::rgb` hands back for every pixel at every depth.
  For an 8-bit source, the entries it reads (`v * 257`) are exactly the reference's 256-entry table. For a 16-bit
  source, it covers every value the sampler can produce. One path serves both depths (design D2).
- **The encode back to a channel at each depth.** At 8 bits it searches the reference's 255 step thresholds and
  replaces a `powf` per channel. At 16 bits it is the direct transfer plus `Channel::from_unit` (design D3).

**The reference's eight colour tests, ported**, plus the ones the 16-bit path owes:

- the table agrees with the direct transfer at every 16-bit sample;
- the 16-bit encode returns every sample it is handed, after linearizing it.

**Nothing calls any of it yet.** The module declaration carries
`#[cfg_attr(not(test), expect(dead_code, reason = ...))]` naming slice 2, which is how `add-inference-5-kyoto`'s
predecessors marked layers that were waiting for their first caller. When slice 2 lands, the attribute comes off.

### Deliberate divergence from the reference implementation

**None in anything this slice can render**, because it renders nothing. Two choices take effect once slice 2 uses them,
and both are recorded here so they are not discovered there:

- **The run's depth is kept.** The reference writes every colorized photograph as 8-bit. Here, a 16-bit photograph's L
  is read at 16 bits and the result is encoded at 16 bits, as every other family already does. At 8 bits the arithmetic
  is the reference's, value for value (design D2, D3). This was decided in exploration: this app decodes RAW files to
  16 bits, and a colorized RAW reduced to 8 bits would be the one family that throws precision away.
- **The 8-bit encode rounds the way `Channel::from_unit` rounds.** The reference's threshold table is pinned against
  `uint8(Clamp255(x*255 + 0.5))`. Here it is pinned against this crate's own `u8::from_unit`, so that the fast path and
  the general path of this crate cannot disagree (design D3). Both round half up on the non-negative values an encode
  produces. Any case where they could differ is a float-addition tie, which the boundary test would expose rather than
  hide.

### Infrastructure: what `rust-sak` already covers

**Nothing applies.** `rust-sak`'s `image` module is codecs: encode and decode behind one API. It has no colour-space
conversion, and none of `crypto`, `fetch`, `fs`, `image-raw`, `memo`, `o11y` or `sysinfo` is relevant. The gap was
weighed in exploration: OpenCV-convention CIELab is general enough that another project could want it. But it has
exactly one caller here, and the constants and fast paths are tuned to what colorization graphs were trained on, so it
stays local to this project at the lowest tier that holds its caller. **No new third-party crate.** The one-time table
builds use `std::sync::LazyLock`, which `models/upscale/osaka/colorfix.rs` already uses for the same purpose.

### Task estimate

**8** against the 15-task budget: the conversions and their tests, the table-backed luminance and gray paths and their
tests, the two encodes and their tests, the module prose, and the toolchain pass. Slice 2 is estimated at 14; see
design.md for the series.

## Capabilities

### New Capabilities

None. This slice introduces no observable behaviour. The `colorization` capability, covering what a colorization run
does to a photograph at either depth, is created by slice 2, the first change that can run one.

### Modified Capabilities

None. `.openspec.yaml` sets `skip_specs: true`, as `add-color-balance-1-canvas` did for the same reason. A requirement
about arithmetic no run reaches yet would be a requirement about internal structure, which this project's `specs` rule
forbids.

## Impact

**Crates touched: `opai` only.** `gui`, `cli` and `perf` reach the crate through its public API, and nothing public
changes.

| File | Change |
| --- | --- |
| `crates/opai/src/models/colorization/lab.rs` | new: the conversions, the tables, the encodes, and their tests |
| `crates/opai/src/models/colorization/mod.rs` | the module declaration with its `expect(dead_code)`; the header gains the Lab contract |

**Public API: unchanged.** Everything is `pub(super)` or narrower, and `tests/public_api.rs` is untouched.

**Risk is confined to arithmetic, and all of it is checked on every CI platform.** No ONNX Runtime, no model file, no
GPU and no network are involved. The ported tests pin the float convention against OpenCV's own values, the fast paths
against the general ones bit for bit, and the encode at every one of its 255 step boundaries.
