## 1. The two promotions

- [x] 1.1 Create `crates/opai/src/inference/bilinear.rs` and declare it in `inference/mod.rs`. Move `Tap`, `Axis`,
      `sample` and their whole test module out of `models/color_balance/saopaulo/weights.rs`, widening them to
      `pub(crate)` and renaming `sample`'s `canvas` parameter to `side` (design D1). Re-point
      `saopaulo/process.rs` at `crate::inference::bilinear`, remove `mod weights` from `saopaulo/mod.rs`, and delete
      `weights.rs`. Verify that `cargo test -p opai inference::bilinear models::color_balance` passes, and that the diff
      changes each moved test's `use` lines and **no assertion**. An assertion that needed editing means behaviour
      moved; stop there.
- [x] 1.2 Create `crates/opai/src/inference/rows.rs` and declare it in `inference/mod.rs`. Move `for_each_row` out of
      `models/upscale/osaka/colorfix.rs` with its `MIN_ROWS_PER_WORKER`, its `available_parallelism` read and its prose,
      generalised to `for_each_row<T: Send, F: Fn(usize, &mut [T]) + Sync>(dst, row_len, height, row)` (design D2).
      Re-point both of `colorfix`'s convolutions at it with `row_len = width`. Verify that
      `cargo test -p opai models::upscale::osaka::colorfix` passes with no assertion edited. Add a test in `rows.rs`
      that a row length other than the width (three samples per pixel) visits every row exactly once, with its own
      slice. Verify that `cargo test -p opai inference::rows` passes.
- [x] 1.3 Update the prose the moves invalidate. `bilinear.rs`'s header drops "Model tier: one caller" and names both
      callers, and keeps its reasoning against `Triangle` and `Taps`. `sample`'s doc says "one square plane", not "one
      weight map". `inference/mod.rs` gains both modules in its list. São Paulo's `process.rs` comments name
      `bilinear::sample` where they named `weights::sample`. `colorfix.rs`'s header says where the row split now lives.
      Verify that `cargo doc -p opai --no-deps` builds with no broken intra-doc links and that
      `grep -rn "weights::" crates/opai/src` finds nothing.

## 2. From a photograph to a graph tensor, and from a graph output to chroma

- [x] 2.1 Add `models/colorization/stretch.rs` with `stretched(source, side, filter) -> DynamicImage`: `Rgb8` and `Rgb16`
      resampled as they are, other 8-bit layouts flattened to `Rgb8` and everything else to `Rgb16` through
      `Sampler::rgb`, then `resize_exact(side, side, filter)` (design D3). Carry the reasoning: `imaging.Resize`'s
      alpha-weighted sums, and why flattening first reproduces them. Declare it in `models/colorization/mod.rs` under
      `#[cfg_attr(not(test), expect(dead_code, reason = ...))]` naming `add-colorization-3-pipeline`. Verify with tests:
      the result is `side × side`, an `Rgb8` source equals `resize_exact` on it directly, a translucent `Rgba8` source
      equals the stretch of its own flattening, a 16-bit source comes back 16-bit, and an 8-bit one comes back 8-bit.
- [x] 2.2 Add `models/colorization/ab.rs`, the Ab contract for Delhi and Mumbai (design D5): `SIDE = 512`,
      `CHANNELS = 2`, `FILTER = Lanczos3`, `input(source) -> Vec<f32>` (stretched, then `lab::gray` of each pixel
      written to all three CHW planes), and `planes(output) -> (&[f32], &[f32])`. Carry the reference's prose on the 512
      square, DDColor's gray rendering, and why `gray` skips the Lab round trip. Declare it with the same
      `expect(dead_code)`. Verify with the reference's tests, ported: the input is neutral and in `[0, 1]` at every
      997th pixel of a 512-square pattern, pixel 0 equals `lab_to_rgb(rgb_to_lab(..)[0], 0, 0)`, and `planes` returns
      the first and second `SIDE²` of a 2-channel tensor.
- [x] 2.3 Add `models/colorization/jaipur/mod.rs`, declaring only `rgb` and saying that the profile is slice 3's.
      Add `jaipur/rgb.rs`, the Rgb contract (design D5): `SIDE = 560`, `CHANNELS = 3`, `FILTER = Triangle` pending 2.4,
      `input(source) -> Vec<f32>` (stretched, then `(0.299·r + 0.587·g + 0.114·b) / 65535` over the 16-bit samples, on
      all three planes), and `chroma(output) -> (Vec<f32>, Vec<f32>)` (each channel clamped to `[0, 1]`, then
      `rgb_to_lab`, keeping a/b). Carry the reference's prose on DeOldify's `render_factor` 35 × 16, the artistic
      re-export that did not move it, and the YUV-to-Lab transfer. Declare `jaipur` with the same `expect(dead_code)`,
      and keep everything no more visible than the family. Verify with the reference's tests, ported: the input is
      neutral, in `[0, 1]`, and pixel 0 equals the ITU-601 formula exactly; a gray ramp gives chroma within 0.01 of
      zero; and `[1.5, -0.5, 0.5]` gives chroma that is not NaN.
- [x] 2.4 Measure the stretch against the reference's (design D4). Use a harness that is not committed: a Go program
      against `imaging@v1.6.2` beside a temporary Rust test or binary calling `stretched`, removed before the task
      completes. Stretch `fixtures/test.dat` and a synthetic translucent image with both libraries, bilinear at 560 and
      Lanczos at 512, and compare the premultiplied samples in 8-bit levels. Keep `Triangle` and `Lanczos3` if the
      maximum difference is within **2 levels**, and write the measured maximum and mean into each `FILTER`'s comment.
      If either exceeds 2, stop: record the numbers here and do not port `imaging`'s resampler without a new decision.
      Verify by recording both pairs' maximum and mean in this task's completion note.

      **Stopped, 2026-09-23: the bound is exceeded.** Measured on the M2 Max. The harness was a Go program against
      `imaging@v1.6.2` and a temporary test in `stretch.rs`, both removed. Rust decoded `fixtures/test.dat` once and
      handed Go the same raw pixels, so no JPEG decoder difference enters. The translucent source is 700×500 with alpha
      over the whole range and fully transparent blocks. The figures are premultiplied samples in 8-bit levels:

      | source | `image` against `imaging.Resize` | max | mean | samples > 1.5 levels |
      | --- | --- | --- | --- | --- |
      | `test.dat` (opaque 640²) | `Triangle` / `Linear` at 560 | 1.00 | 0.199 | 0 of 940,800 |
      | `test.dat` (opaque 640²) | `Lanczos3` / `Lanczos` at 512 | **5.00** | 0.195 | 37 of 786,432 |
      | translucent 700×500 | `Triangle` / `Linear` at 560 | **2.28** | 0.308 | 885 of 940,800 |
      | translucent 700×500 | `Lanczos3` / `Lanczos` at 512 | **49.0** | 0.537 | 33,391 of 786,432 |

      **The cause is not alignment, support scaling or edge handling**, which D4 said an excess would mean. A control
      stretch used `imaging`'s own `precomputeWeights`, flattened the source against black, and kept floats between the
      passes. `image` agrees with it to within its final rounding: max 0.50 / 0.50 on `test.dat` (bilinear / Lanczos)
      and 0.97 / 1.13 on the translucent source. So the whole excess is `imaging`'s 8-bit intermediate, and D4's bound
      misjudged it by modelling rounding alone. The intermediate also **clamps**. Lanczos overshoot at high-contrast
      edges is clipped to [0, 255] before the vertical pass, which gives the opaque photograph's 37 outliers. It also
      **divides by `Σ w·a`**, which the negative Lanczos lobes drive towards zero beside transparent regions. That
      clamped quotient gives the translucent source its up-to-49-level differences.

      **Decided with the user on 2026-09-23: keep `Triangle` and `Lanczos3`.** The figures above are written into each
      `FILTER`'s comment, `stretch.rs` states what its flattening does not reproduce, design D4 is revised to the
      measured explanation, and the proposal lists this among its deliberate divergences. `imaging`'s resampler is not
      ported.

## 3. The fused compose

- [x] 3.1 Add `models/colorization/compose.rs` with an `Encoded: Channel` trait (`u8` via `lab::srgb_u8`, `u16`
      via `lab::srgb_u16`) and `composed::<T>(source, (a, b), side, (width, height)) -> ImageBuffer<Rgb<T>, Vec<T>>`
      (design D6). Build the two `Axis` tables once. Split the rows through `rows::for_each_row` with
      `row_len = 3 * width`. Per pixel, compute `lab::lightness`, `bilinear::sample` for each plane,
      `lab::lab_to_linear_rgb` and `T::encode`. Carry the reference's prose on why the four passes are fused, and
      document the panic on a zero-length axis. Declare it with the same `expect(dead_code)`. Verify that
      `cargo build -p opai` succeeds with no warning.
- [x] 3.2 Port the reference's separate-steps oracle into `compose.rs`'s tests: `l_plane` through `rgb_to_lab`,
      `resize_plane` with its own coordinate arithmetic and identity short-circuit, and `compose_lab` through
      `lab_to_rgb` and `T::from_unit` (design D7). Assert `composed` equal to it **bit for bit** over the six shapes, the
      six source layouts and both depths in design D7's table, with the reference's `synthChroma`. Port
      `resize_plane`'s constant and 2×2 → 4×4 tests, and zero-chroma-preserves-gray at both depths within one level.
      Verify with `cargo test -p opai models::colorization`. If equality fails, fix the compose, never the oracle, and
      never by adding a tolerance.
- [x] 3.3 Time `composed` at `u8` and `u16` on the reference's 4000×3000 `synth` photograph with 512-square chroma, in
      a release build on the M2 Max, with a harness removed before the task completes (design D8). Verify by recording
      both times, and the reference's 231 ms, in this task's completion note. State whether slice 1's coarser-table
      escape hatch looks warranted, without implementing it.

      **Measured, 2026-09-23.** Apple M2 Max, 12 cores, release build. The source was the reference's `synth` pattern at
      4000×3000 with `synthChroma(512)`, `Sampler` built outside the timing and the lazy tables warmed first. Median of
      15 runs, with min and max:

      | source | `composed::<u8>` | `composed::<u16>` |
      | --- | --- | --- |
      | `Rgb8` | 86.4 ms (80.2 – 93.4) | 87.3 ms (83.0 – 96.2) |
      | opaque `Rgba8` | 85.1 ms (81.8 – 91.0) | 88.9 ms (84.6 – 95.2) |

      The reference's is 231 ms. **The 16-bit compose costs 1–4 ms more than the 8-bit one**, which is inside the
      run-to-run spread and small against a graph run of 100–900 ms. The coarser-table escape hatch is not warranted.
      The harness was a temporary ignored test in `compose.rs`, and it has been removed.

## 4. Prose and toolchain

- [x] 4.1 Update `models/colorization/mod.rs`'s header to list `stretch`, `ab`, `jaipur::rgb` and `compose` beside
      `lab` and say what each holds. Keep the family named as still refused, now until `add-colorization-3-pipeline`.
      Re-point `lab`'s `expect(dead_code)` reason from `add-colorization-2-pipeline` to `add-colorization-3-pipeline`.
      Verify with `cargo doc -p opai --no-deps` (no broken intra-doc links) and with
      `grep -rn "colorization-2-pipeline" crates` finding nothing.
- [x] 4.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`. Confirm that every
      `expect(dead_code)` in `models/colorization/` is fulfilled rather than unfulfilled outside tests, and that
      `crates/opai/tests/public_api.rs` needed no change. Verify that all three exit zero.
