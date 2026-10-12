## 1. The contract

- [x] 1.1 Write the guard in `models/denoise/process.rs`: `diverged(output, threshold)`, with a strict comparison on
      the absolute value (design D2). Verify with unit tests that 3.0 and -3.0 do not trip it, that 3.0001 and -3.0001
      do, that one exploding value among thousands of ordinary ones does, and that an empty slice does not.
- [x] 1.2 Write the pipeline in the same file: it embeds a `SingleGraph` and carries an `Option<f32>` guard and the
      strength, and implements `OnOneGraph` and `ImagePipeline` with `stages()` of 1. The per-tile step calls
      `B::run_tile` and copies the input over the output when the guard trips (design D2). The body is `run_tiled` at
      `TileGeometry::DEFAULT`, scale 1 and `Normalisation::Unit`, then `mix::blended` over the typed result, dispatched
      once on `ChannelDepth` (design D4). Verify it compiles, and that a test asserts `Normalisation::Unit` as a literal.
- [x] 1.3 Add the progress schedule and the cancellation check before the blend: report `0.0`, map the tiles onto
      `0.0..=TILES`, and report `1.0` only after the blend (design D3). Verify with the fake-backend suite that progress
      never decreases, that no value reaches 1 before the blend has run, and that the last value is exactly 1.
- [x] 1.4 Write the fake-backend suite for the whole pipeline. Verify: output dimensions equal input dimensions for a
      photograph of many tiles and for one smaller than a tile; strength 0 returns the photograph and strength 1 the
      backend's output, both exactly; strength 2 moves further than 1 in the same direction and stays in range; a
      backend returning 4.0 on one tile gives the input's pixels in that tile's region and the output elsewhere, at 8
      and at 16 bits; an unguarded pipeline passes the same 4.0 tile through; cancellation between tiles and before the
      blend each return no image; a zero-area image returns `Untileable`.

## 2. The three models

- [x] 2.1 Add `models/denoise/stockholm/mod.rs` with `GUARD = 3.0` and a `profile()` returning the default at both
      precisions until task 4.3 settles it (design D8). Carry the reference's prose explaining why 3.0 clears legitimate
      output (of order 1) and sits far below the NAFNet blow-up (1000+), and state that no photograph that triggers it
      has been observed in this project (design D7). Verify with a test pinning the threshold.
- [x] 2.2 Add `models/denoise/gothenburg/mod.rs`: `profile()` as a `match precision` giving CoreML `CPUAndGPU` at FP16
      and the default at FP32 (design D6). Carry the reference's measured table, the ReduceL2 and PixelUnshuffle
      re-export prose, and the single-partition precondition, and state that the figures are ported rather than
      re-measured. Verify with the reference's four tests ported: FP16 carries `CPUAndGPU`; FP32 carries the default;
      neither precision declares a sequential mode; nothing else is declared.
- [x] 2.3 Add `models/denoise/malmo/mod.rs` with the same profile shape, its own measured table and its own prose,
      including the LayerNorm rewrite that is in the export but not part of the recipe. Verify with the same four
      tests.
- [x] 2.4 Wire `guard()` and `profile()` in `models/denoise/variant.rs` as exhaustive matches that reach into the model
      directories (design D1). Verify with the family's `every_variant()` traversal: only Stockholm reports a guard,
      the Gothenburg and Malmö FP16 arms declare their profile, their FP32 arms are `EpProfile::default()`, and
      Stockholm's arms are whatever `stockholm::profile()` returns.

## 3. The family seam and the refusal

- [x] 3.1 Add `Denoise::pipeline` in `models/denoise/mod.rs`, built from `params()`, `artifact()`, `profile()`,
      `variant().guard()` and `display_name()`. Remove denoise from `Operation::pipeline`'s refusal arm, and update its
      doc comment and the cross-family tests in `models/operation.rs` that name the refused families and the measured
      variants. Verify that a pipeline for each of the six variant-precision pairs reports its one artifact and session
      pair, that every denoise variant now plans, and that sharpen and colorization still return
      `FamilyNotImplemented`.

## 4. Documentation, examples, live check and toolchain

- [x] 4.1 Update `Opai::process`'s "What is refused" in `crates/opai/src/lib.rs` to name all five running families
      (upscale, face recovery, light adjustment, colour balance, denoise) and the two still refused, fixing the colour
      balance omission it already has. Move `perf`'s declined-model test examples in `crates/perf/src/main.rs`,
      `json.rs` and `report.rs` from Stockholm to a Colorization model. Verify with `cargo doc -p opai` and
      `cargo test -p perf`, and by reading the prose against the refusal arm as it now stands.
- [x] 4.2 Add an `#[ignore]`d live check that runs `fixtures/test.dat` through Stockholm, Gothenburg and Malmö end to
      end via `Opai::process`, at both precisions, on the CPU provider and on CoreML, and prints how many tiles
      Stockholm's guard kept (design D7). Verify by hand with `cargo test -p opai -- --ignored --nocapture`: every run
      returns the source's own dimensions, strength 0 returns the source unchanged, and the guard count is recorded in
      the task's completion note.
      Done 2026-09-22 on the M2 Max: 12 runs (3 variants x FP32/FP16 x CPU/CoreML) over the 640x640 fixture, all
      at the source's own size, strength 0 identical to the source. Stockholm's guard kept 0 tiles in every run.
- [x] 4.3 Sweep Stockholm's provider settings on the M2 Max to find out whether the defaults are the best option or
      whether a profile beats them (design D8). Rebuild with each candidate `stockholm::profile()` and run
      `cargo run --release -p perf -- stockholm --provider <p> --precision <q>` at FP32 and FP16. The candidates are: the default; CoreML at `CPUAndGPU` and at `CPUAndNeuralEngine`; sequential
      execution on CoreML and on the CPU provider; and CoreML's fast-prediction hint. Adopt a setting only at a
      precision where it is outside run-to-run spread. Verify by recording every median in `stockholm/mod.rs`'s prose,
      rewriting the `execution-providers` requirement "Stockholm's measured settings" with the figures and the outcome,
      and pinning the adopted profile — including a default that won — with tests in the style of Gothenburg's four.
- [x] 4.4 Run `cargo fmt`, `cargo clippy --workspace --all-targets` and the full test suite. Verify all three exit zero.
