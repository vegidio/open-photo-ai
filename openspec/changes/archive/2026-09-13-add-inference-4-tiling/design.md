## Slices

Running an image through a chain of models estimates far past 15 tasks. It is divided into six
changes, each leaving the project buildable and testable on its own. **This change is slice 4.**

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-inference-1-models` | A named model's files on disk, verified against the published manifest | the initialization series | **done** |
| 2 | `add-inference-2-providers` | The `ExecutionProvider` type including `Auto`, per-provider default options, and the per-model profile that overrides them | 1 | **done** |
| 3 | `add-inference-3-sessions` | Session construction, the resident session cache with its idle life, and CPU fallback | 2 | **done** |
| 4 | `add-inference-4-tiling` | The shared inference primitives — the tile grid, reflection padding, CHW tensor conversion at either output depth, seam blending — and the tiled driver over them, generic in the per-tile run | none (parallel with 1-3) | **current** |
| 5 | `add-inference-5-kyoto` | The upscale pipeline and the public chain entry point, with progress, cancellation and `OutputDepth` — Kyoto runs end to end | 3, 4 | pending |
| 6 | `add-inference-6-cache` | The 24-hour per-step image cache, keyed on the input identity plus the operations applied | 5 | pending |

Slice 4 depends on nothing in 1-3 and could have been implemented alongside any of them. Everything
else is a straight chain.

**Slice 4's row has widened since slice 3 recorded it**, and slice 5's has narrowed by the same
amount. The recorded plan gave slice 4 the grid, the conversion and the blend, and left the loop over
them to slice 5. Making the per-tile run a parameter (D2) lets that loop land here without depending
on a session, and it is the loop rather than the pieces that has to have one implementation — which
is the reference implementation's own conclusion, written into the doc comment on its driver after
the diffusion upscaler grew a second one. Slice 5 now wires sessions into a driver that already
works, and keeps `OutputDepth`, the multi-pass progress weighting and the public surface.

Deferred past the series, each to its own change when something needs it: the model memory budget
with LRU eviction; alpha preservation through inference; the eleven measured execution-provider
profiles other than Kyoto's; the remaining seven families' pipelines; logging; evicting sessions
built on a provider the user has just switched away from; and — added by this slice — the per-tile
divergence guard, and the diffusion upscaler's own accumulator.

## Context

Slice 3 ends at a `SessionHandle`: an `ort::session::Session` behind a `Mutex`, resident, reporting
the provider it was built on. Nothing calls `run` on it. Slice 5 is what will.

What such a session accepts is one fixed shape. Kyoto's graphs, like every convolutional model this
project ships, take `1x3x256x256` `f32` — planar, channel-major, RGB, normalised to `[0, 1]` — and
return `1x3x(256*scale)x(256*scale)` in the same layout. `image::load` produces a `DynamicImage`,
which is a photograph of arbitrary dimensions in one of ten colour types at 8, 16 or 32 bits per
channel. Nothing in this repository crosses that gap in either direction.

Three facts about the crates involved shape the design and were checked against their source rather
than assumed:

- **`DynamicImage`'s `GenericImageView` implementation has `type Pixel = Rgba<u8>`**
  (`images/dynimage.rs:1493`), and its `get_pixel` maps every variant through `to_rgba().into_color()`.
  Reading a 16-bit image through it therefore discards the low byte of every channel, silently and
  with no error. This is the single most dangerous API in the slice; D4 is about not using it.
- **`ort` can wrap a borrowed slice as a tensor.** `TensorRef::from_array_view` and
  `TensorRefMut::from_array_view_mut` (`value/impl_tensor/create.rs:214,253`) take
  `(shape, &[T])` and `(shape, &mut [T])`. So the driver can own the scratch buffers and slice 5 can
  bind them per tile without a copy on the input side — which is what makes D7 affordable.
- **`image`'s `Rgb`/`Rgba` are straight alpha**, unlike Go's `image.RGBA`, whose `RGBA()` accessor
  returns premultiplied values. The reference's tensor conversion therefore premultiplies and this
  one has to do so explicitly to produce the same numbers. See D5.

See proposal.md - Why for motivation, and specs/inference-tiling/spec.md for the behaviour.

## Goals / Non-Goals

**Goals:**

- One implementation of the partitioning rule, the padding, the normalisation and the seam ramp,
  reached by every family rather than reassembled per pipeline.
- The whole driver — the loop, the progress accounting, the cancellation check, the refusals — is
  exercised on every CI platform with no runtime, no GPU, no model file and no network.
- A 16-bit source keeps its precision on the way into a model, and the result carries whatever depth
  the caller asks for on the way out. Neither is a property of this module's opinion.
- The per-tile allocation profile the reference had to retrofit is the one this starts with: a
  thousand-tile image allocates the scratch once, not a thousand times.

**Non-Goals:**

- Running a model. No `ort` type is named in this module, and `ort` is not in its dependency path at
  all; the per-tile step is a function the caller supplies.
- Deciding the tile geometry per model. The driver takes it, and the pair every shipped fixed-shape
  model was tuned against is a named default.
- Resampling. Nothing here resizes: correcting an upscale's overshoot to the requested scale is the
  pipeline's job in slice 5, and the driver only ever crops.
- A public API. The module is `pub(crate)`, as slices 1, 2 and 3 were.

## Decisions

### D1. `inference/`, a module of its own, not inside `sessions/`

`inference/mod.rs` for the driver, with `grid.rs`, `pad.rs`, `tensor.rs` and `blend.rs` beside it.
Tests beside each, as the project's convention requires.

Slice 3's D1 said `sessions/` was "the directory slices 4 and 5 grow into". That was written before
it was clear the driver needed no session, and slice 3's own argument now points the other way: it
put `sessions/` outside `providers/` precisely because `providers/` was built to need no runtime and
its whole suite is the proof of that. This module has the same property in a stronger form — it needs
no runtime, no model file and no filesystem — so putting it inside the one module in the crate that
opens a 6.8 GB graph would give up exactly what slice 3 spent a decision protecting.

It is also the directory slice 5 grows into: the family pipelines are inference, and they will sit
beside the driver rather than inside the session cache.

### D2. The per-tile run is a parameter, which is what lets this slice stand alone

```
run_tiled(
    source:   &DynamicImage,
    geometry: TileGeometry,          // size and overlap; DEFAULT is 256/16
    scale:    u32,                   // the model's output scale: 1 for denoise, N for an NxN upscale
    depth:    ChannelDepth,          // Eight or Sixteen; D6
    norm:     Normalisation,         // Unit for [0, 1], Signed for [-1, 1]
    progress: Option<&dyn Fn(f64)>,
    cancelled: &dyn Fn() -> bool,
    run_tile: &mut dyn FnMut(&[f32], &mut [f32]) -> Result<(), E>,
) -> Result<DynamicImage, TilingError<E>>
```

`run_tile` receives the tile already converted and padded, and writes the model's output into the
buffer it is handed. The driver holds no session, names no `ort` type, and is instantiated in the
tests at a closure that does arithmetic — which is what makes every scenario in the spec a test that
runs on a CI runner with nothing installed. It is the same move slice 3's D5 made for the session
cache, for the same reason and with the same evidence behind it.

The alternative, a `TileRunner` trait with the driver generic over it, buys a name and costs a type
per call site; every implementation would be a closure wrapped in a struct. Rejected.

**Why an out-parameter rather than a returned buffer.** A `Vec<f32>` per tile is what the reference
had to remove after the fact — thousands of identically-shaped 12 MB allocations on a 4x pass, all
immediately garbage. The out-parameter also leaves slice 5 free: `TensorRefMut::from_array_view_mut`
binds this buffer directly if `ort`'s I/O binding path is used, and a single `copy_from_slice` out of
an owned output tensor covers it otherwise. That copy is ~12 MB per tile at a 4x pass against a
per-tile inference cost of a couple of hundred milliseconds, so neither choice is forced now.

### D3. The grid is a value that also reports the overlaps it actually produced

`TileGrid { size, overlap, width, height }` with `tiles()`, `columns()`, `overlaps_x()` and
`overlaps_y()`. The partitioning rule and the figures the blend needs come from one place, which is
what the reference's own comment on it asks for: the diffusion upscaler needs the same partitioning
and a completely different per-tile pipeline, so the grid has to be usable without the driver.

Two rules in it are not obvious and both are load-bearing:

- **A tile that would overhang the edge is moved back, not shrunk**, so every tile is the shape the
  graph accepts. That is a hard requirement, not a preference — a fixed-shape graph rejects anything
  else.
- **The moved tile is dropped when it lands exactly on its predecessor.** At 512/128 over a 1280-pixel
  side that is three tiles rather than four, and for a diffusion model a duplicate is tens of seconds.

The consequence is the one that has actually caused a visible bug, and it is why `overlaps_x` exists
rather than the blend assuming `overlap`: a moved tile overlaps its predecessor by however far it was
moved. At 256/16 over a 600-pixel side the offsets are `[0, 240, 344]`, so the last tile overlaps by
**152**, not 16. Ramping that seam over 16 pixels leaves the other 136 written at full strength,
which is a hard edge down the last column and along the last row.

An invalid geometry — a non-positive size, an overlap that leaves no forward progress, a tile bigger
than the axis — yields one tile covering the whole axis, so no geometry can produce an empty
partitioning. A zero-length axis yields none, and the driver turns that into a refusal (D9): the
failure it prevents is the reference's own, where the loop ran zero times and a freshly allocated,
never-written buffer was returned with no error and shown to the user as a finished enhancement.

**The driver does not take that fallback, and the difference is deliberate.** The rule above is the
grid's, and it is the right one for a caller that drives `TileGrid` itself — the diffusion upscaler,
which chooses its own shape. `run_tiled` instead normalises a degenerate geometry into a workable one
before it builds the grid: the size is floored at 1 and the overlap clamped below it. The fallback
would otherwise hand it a single tile larger than the shape the model accepts, and since the driver
converts each tile at that shape, everything past the first `size` pixels of each axis would go
unwritten — most of the picture, silently, with no error. A geometry that bad is a mistake in a
model's profile rather than anything a user can cause, so it is a `debug_assert!` in the tests that
run against every profile and a normalisation in a release build: the wrong figure should not abort
somebody's export.

For the same reason the driver floors the model's output scale at 1. A scale of zero would make the
result empty, which is the one outcome the refusal above exists to prevent, so it is not reachable
through a different door.

### D4. Pixels are read through the variant, never through `DynamicImage`'s `get_pixel`

Because that accessor is typed `Rgba<u8>` (see Context), reaching for the obvious API would quietly
halve the precision of every 16-bit RAW the application just taught itself to load and export. The
spec requires the opposite, so this cannot be left to be noticed.

A small sampler built once per run, borrowing where it can:

- `Rgb8`, `Rgba8`, `Rgb16`, `Rgba16` — borrowed, indexed directly into the buffer. These four are
  everything the loader actually produces: `rust-sak` decodes the eight file formats into the first
  two, and develops RAW into the last two.
- anything else — converted once through `to_rgba16()` and owned. Luma, LumaA and the two float
  variants land here. It costs one full-image conversion for a 16-bit greyscale scan, which is rare,
  and it is correct rather than lossy, which the borrowed-but-narrowing alternative is not.

Both paths present one operation: the 16-bit RGB triple for a pixel. An 8-bit channel widens by
`* 257`, which is exact — it maps 255 to 65535 — so an 8-bit source produces exactly the values it
would have produced had 16-bit support never been added. That equality is what the spec's
"8-bit sources are unchanged" scenario is checking, and it is the same identity the reference relies
on; the reference's own comment records that it got this wrong for years by dividing before
normalising, which quantised every 16-bit TIFF to 256 levels and biased them all low.

### D5. Alpha premultiplies against black on the way in and is gone on the way out

The reference feeds premultiplied values to the model, not because it chose to but because Go's
`color.Color.RGBA()` returns them. `image`'s `Rgba` is straight alpha, so producing the same numbers
takes an explicit multiply, and not doing it would be a silent behaviour divergence on every
transparent PNG. So: premultiply.

On the way out, the result is `Rgb` — three channels, not four with a column of 255 in the fourth.
Alpha preservation through inference is deferred series-wide, and the composite against black that
premultiplying performs has already happened by then; a fourth channel would be claiming the model
returned an opacity it was never given. It is also a quarter off the largest allocation the
application makes — the result of an 8x upscale — which is not why it is being done but is not
nothing.

Visible difference from the reference: a transparent source exported to PNG comes back without an
alpha channel rather than with an all-opaque one. The pixels are identical.

### D6. The output depth is a type parameter, and the blend is written once for both

`trait Channel: Primitive` with `from_unit(f32) -> Self`, `to_unit(self) -> f32` and the maximum,
implemented for `u8` and `u16`. The tensor decode and the seam blend are written once against it and
instantiated twice; `run_tiled` matches on `ChannelDepth` at the top and runs the generic body, so
there is exactly one dispatch point in the module.

This is what the `OutputDepth` in slice 5's row has always meant, written down at last rather than
carried as a name in a table: the user chooses 8 bits, 16 bits, or whatever the source had, and
"whatever the source had" resolves against the loaded image in slice 5 into one of the two this
module can produce. Fixing the decode at 8 — which is what the reference does — would mean a 16-bit
RAW loses its depth in the middle of a pipeline whose loader and whose four 16-bit export formats
both preserve it, and no later change could recover it.

The alternatives were both worse. Two concrete implementations duplicate the ramp arithmetic, which
is the one piece of this module where a divergence is a visible artefact rather than a wrong number.
Accumulating in `f32` and quantising at the end needs a full-size float buffer — 12 bytes per pixel
against 3 or 6 — which at an 8x upscale is the difference between fitting in memory and not.

### D7. The scratch belongs to the driver, allocated once per run

One `Vec<f32>` for the input, one for the model's output, one `ImageBuffer<Rgb<T>>` for the decoded
tile, and one borrowed-or-owned sampler. Every tile of a run has the same shape, which is what makes
one of each enough.

This is not a micro-optimisation retrofitted early: at 256/16 a 24-megapixel photograph is over 400
tiles, and at a 4x pass each tile's output buffer is 12 MB and its decoded image 3 MB. Allocating
those per tile is six gigabytes of identically-shaped garbage per pass. The reference has a comment
on each of the three explaining that it used to.

The decoded tile is read once, by the blend, before the next tile overwrites it — which holds because
the driver is a sequential loop. Tiles are not run in parallel, and this is the constraint that would
have to be revisited first if they ever were; recorded here rather than discovered then. Parallelism
across tiles is not obviously a win anyway: the session is behind a `Mutex` (slice 3's D4) and one
GPU does not finish two tiles faster than one.

### D8. Progress and cancellation are closures here, and types in slice 5

`Option<&dyn Fn(f64)>` and `&dyn Fn() -> bool`. The driver needs to report a fraction and to ask
whether to stop; it does not need to know what carries either.

This keeps the decisions slice 5's row lists in slice 5, where the callers are: whether cancellation
is a `CancellationToken`, an `AtomicBool` or something else is a question about how a front end
cancels an export queue, and answering it here would be guessing on behalf of code that does not
exist. No dependency is added on its account either.

Progress advances by `1 / tiles` per completed tile and the final value is snapped to exactly 1 — the
accumulated sum of four hundred reciprocals does not land on 1, and a progress bar that stops at
99.98% is a bug report. The **caller** emits the initial 0: a multi-pass upscale calls the driver
once per pass and must not reset to 0 at the start of each.

### D9. Errors: three cases, and `Cancelled` is not the crate's existing one

`TilingError<E>`, generic in the per-tile function's error so nothing is boxed and slice 5's session
failures arrive intact:

- `Untileable { width, height }` — an image with no area. A refusal rather than an empty success, for
  the reason in D3.
- `Cancelled` — the caller asked the run to stop. **Deliberately not** `task::Cancelled`, which this
  crate already has and which means the async runtime shut down before blocking work could run. One
  is a user closing a dialog and the other is a process on its way down; they are reported
  differently and a caller that conflated them would tell a user their enhancement failed.
- `Tile { index, source: E }` — the model failed on a tile. The index is carried because a failure on
  tile 1 and a failure on tile 400 of one image are different reports.

A failed or cancelled run returns no image at all. The half-written buffer is not a partial result:
its untouched region is whatever it was allocated as, and handing that back is the same defect class
as the zero-tile refusal above.

### D10. What this slice deliberately leaves out

- **The divergence guard.** The reference's driver can discard a tile whose raw output exceeds a
  magnitude threshold and keep the input pixels instead, guarding NAFNet denoise models that blow up
  on out-of-distribution tiles. Every upscale run has it off, so nothing in this series exercises it;
  written now it would be untested code guarding a model the project cannot yet load. It lands with
  the denoise pipeline, where a real model can be made to trigger it. The hook it needs — the raw
  output, before the decode — is where the driver already inspects it, so adding it later is local.
- **Resampling.** An upscale that overshoots (a 1.5x request served by a 2x pass) is corrected by the
  pipeline, not the driver, and the diffusion variant's fit-to-alignment resizes are its own. Nothing
  here resizes, so `imageops` is untouched by this slice despite being available.
- **The diffusion accumulator.** Osaka feathers its tiles with the same raised-cosine curve into a
  weighted float accumulator rather than into an image. It shares `RampWeight` and the grid, which is
  why both are written to be used without the driver, and nothing else.

### D11. Platform and provider validation

Every line of this module is arithmetic over buffers. There is no platform-conditional code, no
provider, no runtime, no filesystem and no network in it, so the suite runs identically and in full
on the ubuntu, macOS and Windows CI jobs — which is the first slice in this series of which that is
true. Nothing here is `#[ignore]`d and nothing waits for hardware.

The one property CI cannot check is the one that matters most: whether the combined result looks
right. "No visible seam" is asserted as "no adjacent pair of columns across a seam differs by more
than a small step", which is what the reference's own regression test for the wide-overlap bug
asserts, and it is a proxy. A real model against a real photograph is slice 5, and the seam is the
first thing to look at there.

## Risks / Trade-offs

**A seam test that passes on synthetic tiles and fails on photographs** → The blend is checked
against flat and gradient tiles, where a step is arithmetic rather than perceptual. The mitigation is
the same one the reference arrived at: the property being asserted — the ramp spans the real overlap
rather than the configured one — is the exact thing that produced the visible bug, and the worked
figures for it (`[0, 240, 344]`, overlap 152) are pinned as their own test so a change to the
partitioning that silently narrows the ramp fails there first.

**Premultiplying against black is inherited without being justified** → It matches the reference
bit for bit, which is the parity bar, and it is consistent with dropping alpha from the result. But
it means a semi-transparent region darkens before the model sees it, which is a defensible choice
nobody actually made. Recorded so that the change that preserves alpha through inference revisits it
rather than building on it.

**The generic `Channel` doubles what the optimiser has to get right** → Two instantiations of the
decode and the blend, and the 8-bit one is the hot path for almost every run. A regression in it
would be invisible in the 16-bit tests. Mitigated by the arithmetic being identical in both and by
neither path having a special case, but it is a real cost of D6 against the reference's single
8-bit implementation.

**`Rgb` rather than `Rgba` results reach the encoders untested** → `image::save` has only ever been
handed what the decoders produce. An `Rgb16` buffer through the AVIF and HEIF encoders is a path
`rust-sak` supports and this project has not exercised. It is slice 5 that first writes one to disk,
and it is named here so that slice does not discover it.

**A 400-tile run holds the source, the result and the scratch at once** → At an 8x upscale of a
24-megapixel photograph that is roughly 1.2 GB of result at 16-bit, plus the source. No budget
governs it in this slice and none is planned before the memory budget change. The driver does not
make it worse than the reference does — it holds the same three things — but it does not make it
better either, and an 8x 16-bit run on a small machine is a real risk that slice 5 should measure
before the public entry point offers it.
