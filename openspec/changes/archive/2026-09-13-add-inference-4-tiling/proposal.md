## Why

`opai` can name a model, verify its weights onto disk, decide what it should run on, and open it as a
resident session. It still cannot feed one a single pixel.

A fixed-shape model takes one thing: a `1x3x256x256` buffer of `f32`, planar, channel-major,
normalised. What the application holds is a `DynamicImage` of arbitrary size, in one of half a dozen
colour types and either 8 or 16 bits per channel, usually far larger than 256 square. Nothing in this
repository partitions an image into tiles, converts pixels to a tensor or a tensor back to pixels, or
stitches overlapping results together without leaving a seam down every boundary.

Those primitives are shared by all eight model families, not by the one that lands first. Written
inside the upscale pipeline they would be rewritten inside the next pipeline, and the two would
disagree — which is what the reference implementation's own comment on its tiled driver warns about,
having watched it happen.

## What Changes

Task estimate: **12**. The inference feature estimates far past 15 and is split into six slices (see
design.md - Slices); this proposal covers slice 4 only.

**In `opai`, a new `inference/` module. None of it needs a runtime, a GPU, a model file or a
network:**

- **The tile grid.** Where the tiles are for a given image and geometry, including the rule that
  shifts a tile that would fall off the edge back inside rather than shrinking it — every tile is the
  same size, which is what a fixed-shape graph requires — and the rule that drops the shifted tile
  when it lands exactly on its predecessor. It reports, per column and per row, how much each tile
  **actually** overlaps the one before it, which is not the configured overlap and is what the blend
  needs.
- **Reflection padding.** An image smaller than one tile is mirrored outwards to the graph's shape,
  rather than padded with a colour the model would then enhance.
- **Pixels to a tensor and back.** An image region becomes a planar CHW `f32` buffer normalised to
  `[0, 1]` or `[-1, 1]`, reading 8- and 16-bit sources at their full precision; a buffer becomes
  pixels again at **the bit depth the caller asks for**, 8 or 16 per channel.
- **The seam blend.** A finished tile is written into the result with a raised-cosine ramp across
  each edge that meets a tile already written, over the width those two tiles genuinely share.
- **The tiled driver.** One loop that walks the grid, pads, converts, hands the tile to a function
  the caller supplies, crops the padding back off, blends the result, reports progress and stops when
  the caller asks it to. The per-tile function is a **parameter**, so the driver holds no session and
  needs no runtime: the whole loop — partitioning, padding, progress accounting, blending, the
  refusal to tile a zero-area image — is exercised on every CI platform against a tile function that
  is arithmetic.

This is the deliberate widening of what the recorded plan gave slice 4. The plan listed the grid, the
conversion and the blend; the driver is what keeps them from being reassembled differently by each
family that needs them, and making its per-tile step a parameter is what lets it land here without
depending on slices 1-3. Slice 5 then wires Kyoto's sessions into a loop that already works.

**Not exposed publicly in this slice.** The module is `pub(crate)`, as slices 1, 2 and 3 were. Slice
5 is what gives it a caller and a public surface.

## Capabilities

### New Capabilities

- `inference-tiling`: running a model that accepts one fixed-size tile over an image of any size —
  how the image is divided, how a region of pixels becomes the tensor the model reads and how the
  tensor it returns becomes pixels again, at what bit depth, and how overlapping results are combined
  into one picture with no visible seam. It also covers what the driver refuses, what it reports
  while it works, and that it stops when asked.

  Distinct from every existing capability and none is widened. `image-io` reads and writes files and
  describes them; it owns no geometry and no tensor. `model-sessions` opens a model and says nothing
  about what is run through it. `execution-providers` decides what a model runs on. This capability
  is about the shape of the data on either side of a run, and it is the only one of the four with no
  dependency on a runtime being present at all.

### Modified Capabilities

None. Nothing here changes an existing requirement: no file is read or written, no model is opened,
no provider is consulted.

## Impact

**Crates touched:** `opai` only. `gui` and `cli` are untouched, and no logic is placed in a front
end: a tile grid and a seam blend are the definition of shared logic — every family, and therefore
both front ends, would otherwise each own a copy.

**`rust-sak` - what was found before anything was proposed.**

- `image` is the near miss and it is **not** the answer. It is a codec module: eight formats decoded
  and encoded behind one API, plus RAW. It has no geometry, no resampling, no pixel arithmetic and no
  tensor of any kind — `decode_bytes` in, `DynamicImage` out. Nothing in it overlaps this slice.
- **This is not a gap to fill in `rust-sak` either**, which is the judgement its own guidance asks
  for. A tile grid whose partitioning rule exists to satisfy fixed-shape ONNX graphs, a CHW
  normalisation in the exact spelling those graphs were trained against, and a seam ramp tuned so a
  convolutional upscaler leaves no edge, are not general-purpose building blocks that another project
  would want — they are this application's inference contract. `rust-sak` covers what is reusable;
  this is the part that is not.
- `crypto`, `fetch`, `fs`, `memo` and `sysinfo` are untouched by this slice.

**New third-party crates:** none. The `image` crate is already a direct dependency — its
`DynamicImage` is in this crate's public API — and `imageops` comes with it, so the one resampling
call this slice needs adds nothing to the graph.

**Parity divergences from the Go implementation, deliberate:**

- **The decode's bit depth is the caller's choice, not fixed at 8.** The reference decodes every
  model output to 8-bit RGBA, so a 16-bit RAW that is enhanced comes back at 8 bits per channel
  whatever it is then exported to — which this project's own image-io work, where all four of PNG,
  TIFF, AVIF and HEIF keep 16 bits, would otherwise silently undo. The decode here takes the target
  depth as a parameter. Slice 5 is what puts the choice in front of a user as `OutputDepth` — 8, 16,
  or whatever the source was — and resolves "whatever the source was" against the loaded image; this
  slice is what makes both targets expressible at all.
- **Alpha is dropped as a channel rather than carried as a column of 255.** The reference's result is
  RGBA with every alpha byte forced opaque. Alpha preservation through inference is deferred
  series-wide, so the result here is RGB: three channels rather than four saying the same thing, and
  a quarter less memory across a buffer that after an 8x upscale is the largest allocation the
  application makes. A transparent source composites against black either way, exactly as it does
  today in the reference.
- **No divergence guard.** The reference's driver can be asked to discard a tile whose raw output
  exceeds a magnitude threshold, keeping the input pixels instead; it guards NAFNet-based denoise
  models that occasionally blow up on an out-of-distribution tile. No family in this series uses it —
  it is off for every upscale — so it lands with the denoise pipeline that needs it, against a model
  that can demonstrate it firing. Recorded as a parity gap rather than pretended away.
- **Nothing is logged.** Unchanged from slice 3, and for the same reason: this project has no logging
  yet. The reference logs per-pass timings around this driver. Each fact is carried as data instead —
  progress through the callback, failure through the error.

**Not in scope:** the Kyoto pipeline, the public inference entry point, `OutputDepth` as a
user-facing choice and the multi-pass progress weighting across it (slice 5); the per-operation image
cache (slice 6). Also out, each deferred with a reason in design.md: the divergence guard; alpha
preservation; the diffusion upscaler's own accumulator, which needs the same grid and a different
per-tile pipeline; and any `gui` or `cli` change.
