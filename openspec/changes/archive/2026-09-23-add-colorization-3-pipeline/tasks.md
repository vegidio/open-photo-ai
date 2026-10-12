## 1. The three models and the two contracts

- [x] 1.1 Create `models/colorization/delhi/mod.rs` and `models/colorization/mumbai/mod.rs`. Each gets a
      `profile(Precision) -> EpProfile`: `CoreMlComputeUnits::CpuAndGpu` over the default at FP16, and
      `EpProfile::default()` otherwise. Give `jaipur/mod.rs` the same function and drop its "added with the pipeline"
      sentence. Carry the reference's prose (design D1):
      - Mumbai: all of `mumbai.go`'s.
      - Jaipur: all of `jaipur.go`'s.
      - Delhi: that the setting is the reference's declaration beside Mumbai's, on the same DDColor architecture, with
        no measurement of Delhi's own graph behind it.

      Make `ColorizationVariant::profile()` an exhaustive match that reaches each file, with a comment on why three
      files carry the same settings. Add two tests per model, as Paris's are, asked through the variant: FP16 is
      exactly the restriction, and FP32 is the default. Verify with `cargo test -p opai models::colorization`.
- [x] 1.2 Add `models/colorization/process.rs` with `Graph::{Ab, Rgb}` and its four methods: `side`, `channels`,
      `input`, and `composed::<T: Encoded>(source, output, extent)` (design D2). The `Ab` arm of `composed` borrows
      `ab::planes`. The `Rgb` arm binds `jaipur::rgb::chroma` and passes references to it. Each arm calls
      `compose::composed` with its own `SIDE`. Add `ColorizationVariant::graph()` as an exhaustive `const` match:
      Delhi and Mumbai `Ab`, Jaipur `Rgb`. Add a test that drives every variant from `every_variant()` and pins its arm,
      side and channel count (512/2, 512/2, 560/3). Verify with `cargo test -p opai models::colorization`.

## 2. The pipeline

- [x] 2.1 In `process.rs`, add `pipeline::<B>(name, artifact, profile, graph) -> Shared<B>` and `Colorize`
      (`SingleGraph` plus `Graph`). It implements `OnOneGraph`, and `ImagePipeline` with `stages() == 1`. `run`
      dispatches the depth once over `colorized::<T>`. `colorized` does the following, in order (design D3):
      - refuses a zero-area source with `InferenceError::Untileable` before allocating;
      - checks `cancelled()`;
      - builds `graph.input`, then `checkpoint(1/3)`;
      - runs `B::run_graph` with `(3, side, side)` in and `(channels, side, side)` out, then `checkpoint(2/3)`;
      - builds `Sampler::new(input)`, calls `graph.composed::<T>`, then `report(1.0)`.

      Carry the prose on why the three steps are counted and why the sampler is built after the graph run. Verify
      that `cargo build -p opai` succeeds.
- [x] 2.2 Add `Colorization::pipeline::<B>()` in `models/colorization/mod.rs`, built from `display_name`, `artifact`,
      `profile` and `variant.graph()`. Replace `Operation::pipeline`'s `Colorization` refusal arm with
      `Ok(colorization.pipeline::<B>())` and a comment of the form light adjustment's arm has. Remove every
      `#[cfg_attr(not(test), expect(dead_code, ...))]` in `models/colorization/mod.rs`, and declare `process`, `delhi`
      and `mumbai`. Rewrite the module header: it lists `process` and the three model directories, and no longer says
      the family is refused. Verify that `cargo build -p opai` succeeds with no warning, and that
      `grep -rn "colorization-3-pipeline" crates` finds nothing.
- [x] 2.3 Add the fake-backend suite's shape and wiring tests to `process.rs`, driven through `ImagePipeline::run` with
      a `Fake` backend whose session records its input and shapes and writes a test-chosen output (design D4):
      - each variant runs the graph exactly once at its own shapes, for a photograph larger than the square and one
        smaller;
      - the recorded input equals its own contract's `input(photo)` bit for bit, on a colour photograph where the two
        contracts differ;
      - a 0×N and an N×0 source give `Untileable` and never run the graph.

      Verify with `cargo test -p opai models::colorization::process`.
- [x] 2.4 Add the suite's result tests (design D4):
      - **No colour:** an Ab fake writing zeros and an Rgb fake echoing its input give the photograph's gray, within one
        level per channel, with channels within one level of each other, at both depths.
      - **A warm constant over mid-gray:** every pixel has R greater than B, and `lab::lightness` is kept within the
        depth's rounding.
      - **A saturated blue photograph** comes back warm.
      - **Far from square:** a 64×8 source whose fake colours the square's left half warm and right half cool comes
        back warm on the left and cool on the right.
      - **Large:** a source several times the square keeps its size and a one-pixel lightness checkerboard.
      - **16 bits:** a 16-bit gradient at `ChannelDepth::Sixteen` comes back `Rgb16` with more than 256 levels.
      - **Transparency:** an `Rgba8` source comes back `Rgb8`, equal to its own flattening's result.

      Verify with `cargo test -p opai models::colorization::process`.
- [x] 2.5 Add the suite's progress and cancellation tests: the reports are exactly 1/3, 2/3 and 1, never decreasing. A
      cancellation observed at each of the three boundaries gives `InferenceError::Cancelled` and no image, and one
      observed before the stretch never runs the graph. Verify with `cargo test -p opai models::colorization::process`.

## 3. Deleting `FamilyNotImplemented`

- [x] 3.1 Delete `UnsupportedReason::FamilyNotImplemented` from `error.rs`, and rewrite the enum's doc for the one case
      left. In `models/operation.rs`:
      - rewrite `pipeline`'s doc and match comment;
      - turn the served-or-refused traversal into "every operation the carrier can hold reaches a pipeline", and delete
        `served()`;
      - delete the refusal half of the plan test, keeping its category-error half.

      Rewrite `Opai::process`'s "What is refused" in `lib.rs` to list colorization with the other six. Reword the doc
      on autopilot's runnable-families test without naming the variant, leaving the test unchanged (design D5).
      Verify with `cargo test -p opai models::operation autopilot` and `cargo doc -p opai --no-deps` (no broken
      intra-doc links).
- [x] 3.2 Delete `unservable()` and the four tests that use it from `inference/process.rs`:
      `a_chain_whose_second_operation_cannot_be_served_is_refused_whole`,
      `a_chain_whose_second_operation_is_unsupported_installs_nothing_for_its_first`,
      `an_unservable_chain_is_refused_even_when_its_first_operation_is_already_stored` and
      `a_refused_chain_is_recorded_with_what_was_refused_and_why`. Also delete
      `an_operation_with_no_pipeline_is_refused_by_name`, which asserts the same refusal through its own served
      predicate (found during apply, deleted with the user's agreement on 2026-09-23). Remove any import or helper left unused by the
      deletion, and change nothing else. Verify with `cargo test -p opai inference::process`, and that
      `grep -rn "unservable" crates/opai/src` finds nothing.
- [x] 3.3 In `crates/opai/tests/public_api.rs`, delete the deferral pin. Pin the `Unsupported` rendering through
      `UnsupportedReason::IncompleteGraphSet { missing: "decoder" }` instead, asserting the full
      `no pipeline for …: this variant does not declare its decoder graph` text. In `perf`:
      - point `sweep.rs`'s refusal test at `IncompleteGraphSet`;
      - move the Delhi-declined fixtures in `main.rs`, `json.rs`, `report.rs` and `viewport.rs` to Osaka at FP16 (Osaka publishes no FP32 build; codename `osaka`,
        `Family::Upscale`), with that rendering as the reason.

      Verify with `cargo test -p opai --test public_api` and `cargo test -p perf`, and that
      `grep -rn "FamilyNotImplemented" crates` finds nothing.

## 4. Live check and toolchain

- [x] 4.1 Add `models/colorization/live.rs` (`#[cfg(test)] mod live;`), in the form of `sharpen/live.rs` (design D6).
      Install into `live_support::kept_dir("colorization")`. Colorize the committed photograph, reduced to gray, with
      all three variants at both precisions, in `every_variant_colorizes_a_photograph_on_the_cpu` and
      `…_on_coreml`, both `#[ignore]`d. Assert, for each result:
      - the photograph's dimensions;
      - mean `|lightness(result) − lightness(source)|` under 1;
      - mean chroma above 2;
      - at each precision, no two variants' results identical.

      Print cold and warm times, and say in the module doc that Mumbai's first CoreML run includes a compile of about
      five minutes. Run both tests by hand on the M2 Max
      (`cargo test -p opai models::colorization::live -- --ignored --nocapture --test-threads=1`). Verify by recording
      every run's time, lightness difference and mean chroma in this task's completion note. If an assertion fails, stop
      and record it; do not loosen a bound.

      Completion note (2026-09-23, M2 Max, ONNX Runtime 1.26, committed 640x640 fixture reduced to gray, run cache off;
      "cold" on the CPU includes the weights' download into the kept directory):

      | Variant   | Provider | Cold    | Warm    | Lightness drift | Mean chroma |
      |-----------|----------|---------|---------|-----------------|-------------|
      | Delhi 32  | CPU      | 36.33 s | 1.254 s | 0.071 L         | 26.41       |
      | Mumbai 32 | CPU      | 34.37 s | 1.276 s | 0.070 L         | 29.48       |
      | Jaipur 32 | CPU      | 20.52 s | 4.230 s | 0.070 L         | 10.89       |
      | Delhi 16  | CPU      | 20.97 s | 2.664 s | 0.071 L         | 26.38       |
      | Mumbai 16 | CPU      | 20.94 s | 1.319 s | 0.070 L         | 29.48       |
      | Jaipur 16 | CPU      | 10.91 s | 4.152 s | 0.070 L         | 10.90       |
      | Delhi 32  | CoreML   | 6.28 s  | 120 ms  | 0.071 L         | 26.41       |
      | Mumbai 32 | CoreML   | 7.98 s  | 215 ms  | 0.070 L         | 29.48       |
      | Jaipur 32 | CoreML   | 2.02 s  | 245 ms  | 0.070 L         | 10.83       |
      | Delhi 16  | CoreML   | 3.57 s  | 115 ms  | 0.071 L         | 26.32       |
      | Mumbai 16 | CoreML   | 3.09 s  | 113 ms  | 0.070 L         | 29.50       |
      | Jaipur 16 | CoreML   | 1.03 s  | 224 ms  | 0.070 L         | 10.86       |

      Every assertion held, with no bound loosened, and no two variants were identical at either precision. Mumbai's
      first CoreML run took 8 s, not the ~5 min the reference records; not investigated (possibly a compile already
      cached by CoreML on this machine). Timings are for the record, not a measurement.
- [x] 4.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`. Confirm that no
      `expect(dead_code)` remains in `models/colorization/`, and that `grep -rn "still refused\|not written yet"
      crates/opai/src` finds nothing that describes colorization. Verify that all three commands exit zero.
