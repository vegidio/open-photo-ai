## 1. Baseline

- [x] 1.1 Before touching any code, run design D7's measurement on the current commit, in a release build
      (`cargo run --release -p perf -- ...`). Cover:
      - `kyoto`, `paris` and `stockholm` at FP32 and FP16
      - `-p coreml`
      - the embedded sample for all three, and for `paris` also one photograph of at least 12 MP through `--image`

      Use `-n 10 -w 1 --json`, and save each JSON document under `target/perf-baseline/`. Under **Measurements** in
      design.md, record the commit SHA, the photograph's path, dimensions and SHA-256, and a table of median and
      standard deviation per combination. Verify the table is complete: 8 combinations (3 models × 2 precisions on
      the sample, plus `paris` × 2 precisions on the photograph).

## 2. The `imaging` crate

- [x] 2.1 Create `crates/imaging` and add it to the workspace `members`. Its `Cargo.toml` sets `publish = false`,
      depends on `image` pinned exactly as `opai` pins it (`default-features = false`) and on `thiserror`, and declares
      an empty `test-support` feature. Give `lib.rs` a header saying what the crate holds and that it knows nothing of
      `opai`. Add `imaging` to `opai`'s `[dependencies]`, and to its `[dev-dependencies]` with `test-support` enabled.
      Verify with `cargo build -p imaging` and `cargo tree -p opai -i image`, which must show one copy of `image`.
- [x] 2.2 Move `tensor`, `grid`, `pad`, `blend`, `canvas`, `bilinear`, `rows`, `present` and `mix`, with their unit
      tests, from `opai/src/inference/` into `imaging` (design D1):
      - Widen `pub(crate)` to `pub`, and change no function body.
      - Rewrite intra-doc links that point into `opai` (the model callers named in `bilinear`'s and `rows`' headers)
        as plain prose.
      - Point every `opai` import at `imaging::...` and delete the old files.

      Verify with `cargo test -p imaging` and `cargo check -p opai --all-targets`.
- [x] 2.3 Move `TileGeometry`, `RunTile`, `ChannelDepth`, `TilingError` and `run_tiled`, with the tests in
      `inference/mod.rs` that drive them, into `imaging`. `OutputDepth` stays in `inference/depth.rs`, and its
      `resolve` returns `imaging::ChannelDepth` (design D6). Keep `TilingError::Cancelled`'s reasoning, and rewrite its
      link to `crate::task::Cancelled` as prose. Verify with `cargo test -p imaging` and
      `cargo check -p opai --all-targets`.
- [x] 2.4 Move the picture fixtures to `imaging` behind `#[cfg(any(test, feature = "test-support"))]`:
      `test_support::{gradient, photograph, nearest_neighbour}` and `canvas::fixtures::flat` (design D5). Point `opai`'s tests at
      them. Verify with `cargo test -p imaging` and `cargo test -p opai --no-run`.

## 3. The pipeline contract

- [x] 3.1 Create `opai::pipeline` from `inference/pipeline.rs` (as `pipeline/mod.rs`), `inference/session.rs` (as
      `pipeline/session.rs`) and the `Backend` trait from `inference/process.rs` (as `pipeline/backend.rs`), per design
      D2. Move their headers and reasoning with them, and update every import. Verify with
      `cargo check -p opai --all-targets`.
- [x] 3.2 Move the backend and model-shaped fixtures to `pipeline::test_support`: `NoBackend`, `Fake`,
      `stub_backend_runs`, `session`, `PAIR`, `original` and `darkened`. Leave in
      `inference::test_support` only what the orchestration tests alone use, and delete that file if nothing is left.
      Verify with `cargo test -p opai`.

## 4. `models` stops naming `inference`

- [x] 4.1 Move `inference/filter.rs` to `models/filter.rs`, `GUARDED` included (design D4). Update the denoise and
      sharpen modules, their live checks and the doc links in their model files. Verify with
      `cargo test -p opai models::denoise models::sharpen`.
- [x] 4.2 Move `Subject` out of `inference::progress` into `models`, and `PassWeights` into `models/upscale/passes.rs`
      (design D8). The dispatch (`Operation::pipeline`, `DataModel::pipeline`, `as_subject`) stays in
      `models/operation.rs`, naming only `crate::pipeline` (design D3). Verify:
      - `cargo test -p opai`
      - `grep -rn "inference::" crates/opai/src/models` finds nothing, in code or in comments

## 5. Documentation

- [x] 5.1 Rewrite the three-tier comment in `models/mod.rs`. The cross-family tier becomes this module, `models/filter.rs`
      for shared model code, `opai::pipeline` for the contract and `imaging` for primitives, plus the rule that
      `models` never names `inference`. Rewrite `inference/mod.rs`'s header as orchestration only, and fix remaining
      prose that names a moved path. Verify with:
      - `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc -p opai -p imaging --no-deps --document-private-items`
      - `grep -rn "inference::\(tensor\|grid\|pad\|blend\|canvas\|bilinear\|rows\|present\|mix\|filter\|pipeline\|session\)" crates`,
        which must find nothing

## 6. Verification and the performance gate

- [x] 6.1 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`.
      Verify all three pass with no test added, removed or changed except for its import paths.
- [x] 6.2 Run the ignored live checks on CPU and on CoreML for the three measured models: `inference::live`
      (Kyoto), `models::light_adjustment::live` (Paris) and `models::denoise::live` (Stockholm). Use
      `cargo test --release -p opai -- --ignored --test-threads=1` with each module as the filter. Verify every one
      passes as it did before the change. Osaka, sharpen, colour balance and colorization are out of this change's
      live scope by decision, so `canvas` has only its unit tests and the fake-backend suite behind it here.
- [x] 6.3 Re-run task 1.1's measurement exactly as before: same photograph, same flags. Compare against design D7's
      gate (no median worse by more than the larger of 3% and one baseline standard deviation). While any combination
      fails:
      1. add `#[inline]` to the hot non-generic primitives (`Sampler::rgb`, `bilinear::sample`, `Tap`,
         `Normalisation::encode_unit`/`decode`, `ramp_weight`) and re-measure;
      2. then, if needed, add `lto = "thin"` to `[profile.release]` and re-measure.

      Record the final table, and whichever remedy was needed, under **Measurements** in design.md. Verify every
      combination meets the gate.
