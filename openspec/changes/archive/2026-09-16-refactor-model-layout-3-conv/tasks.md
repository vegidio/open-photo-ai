## 1. The pipeline leaves `inference/`

- [x] 1.1 `git mv crates/opai/src/inference/upscale.rs crates/opai/src/models/upscale/passes.rs`, declare
      `pub(crate) mod passes;` in `models/upscale/mod.rs`, delete `pub(crate) mod upscale;` from
      `inference/mod.rs`, and repair only the paths — the `super::{chain,pipeline,tensor}` and
      `super::{ChannelDepth, RunTile, TileGeometry, TilingError, run_tiled}` imports become
      `crate::inference::…`; `models/upscale/mod.rs`'s `use crate::inference::upscale::PassSequence` becomes
      `use passes::PassSequence`; and `osaka/latent.rs`'s intra-doc link to `correct_overshoot` in
      `crate::inference::upscale` becomes `crate::models::upscale::passes`. **No body is edited**, including
      the tests. Verify `cargo build -p opai --all-targets` is clean, that `git diff -M --stat` reports the
      file as a rename rather than as an add, that grepping the workspace for `inference::upscale` and
      `inference/upscale` finds nothing, and that the slice-1 seam test asserting each of the four variants
      reaches the pipeline its contract names passes unedited.

## 2. The convolutional contract gets a file

- [x] 2.1 Create `models/upscale/conv.rs` and move `ScaleBucket`, `ConvVariant` and `FOUR_ONLY` into it
      out of `variant.rs`, carrying every doc comment verbatim — including the reason the bound is stored
      in quantization steps and the reason `max` is a whole number. Move the bucket-table tests with them
      (`every_bucket_table_is_ordered_by_ascending_bound`,
      `every_bucket_table_reaches_the_families_maximum_with_no_gap`, `every_bucket_names_at_least_one_pass`,
      `kyoto_uses_its_native_two_times_weights_where_the_others_repeat_four`,
      `every_codename_is_the_lower_case_ascii_its_filenames_need`,
      `a_bucket_covers_every_request_up_to_its_bound_and_nothing_past_it`) and repoint their `ALL` fixture
      at the three rows in their new homes. Verify all six pass from `conv.rs` asserting the same values,
      and that `variant.rs` no longer declares a bucket or a row.
- [x] 2.2 Add `profile: fn(Precision) -> EpProfile` to `ConvVariant` and the same field to
      `DiffusionVariant` in `osaka/graph.rs`, and declare the `UpscaleModel` trait in `variant.rs` with the
      seven methods design.md D1 lists. Write the field's doc comment to say what D3 says: this is the
      reference's signature and not its decision, the `Fp16Only` wrapper it pairs with there is not adopted,
      and the precision is inside the variant here. Verify `cargo build -p opai` is clean with the trait
      declared and not yet implemented.

## 3. One directory per model

- [x] 3.1 Create `models/upscale/tokyo/mod.rs` holding the `TOKYO` row as a `static` and a
      `fn profile(Precision) -> EpProfile` carrying `UpscaleVariant::profile`'s Tokyo sections **verbatim** —
      the ANE-compiler account with its 1067-second build and 22x figure, the CoreML table
      (`-23%` at FP16, `+3.9%` at FP32), the `Sequential` argument with the CPU-provider table
      (`-24.2%` against Kyoto's `+3.3%`), and all five recorded rejections with their figures. Move
      `tokyo_declares_the_same_two_settings_at_both_precisions` with it. Verify by diffing the moved comment
      text against `HEAD` that it is present character for character, and that the test passes asserting the
      same two settings and nothing else.
- [x] 3.2 Create `models/upscale/kyoto/mod.rs` the same way: the `KYOTO` row with its three-bucket table,
      the FP16 arm, the RRDBNet op-mix reasoning, the 25%-at-4x figure, the FP32-islands lesson about
      measuring an FP16 graph's compute units, the portability note about a Mac with no Neural Engine, and
      the section on what was measured and deliberately left alone. Move
      `kyoto_at_fp16_declares_the_neural_engine_and_nothing_else` and
      `kyoto_at_fp32_takes_the_default_compute_units_rather_than_the_fp16_answer` with it. Verify the same
      way, character for character, and that both tests pass.
- [x] 3.3 Create `models/upscale/saitama/mod.rs` the same way: the `SAITAMA` row, the FP16 arm, the account
      of why it is written out rather than shared with Kyoto's, and the export-regression story with its
      `-17.6%` table. The shared section arguing why Kyoto's and Saitama's FP32 arms are load-bearing is
      **duplicated into both directories rather than moved to one**, since the claim is about each model's
      own export types and a single copy under one model would make the other's FP32 arm read as a formality.
      Move `saitama_declares_the_neural_engine_at_fp16_and_the_default_at_fp32` with it. Verify the same way
      and that the test passes.
- [x] 3.4 Give `osaka/mod.rs`'s `OSAKA` row the new field, pointing at its existing `profile`, and widen
      that function's parameter from `OsakaPrecision` to `Precision` — keeping the whole of its doc comment,
      including the note that the precision is taken and deliberately unread and the reason a build
      publishing one of the two differently would name it there. Verify
      `osaka_declares_every_measured_setting_at_both_precisions` passes unedited, asserting the same literal
      values at both precisions.

## 4. The seam

- [x] 4.1 Implement `UpscaleModel` for `ConvVariant` in `conv.rs` — with the bucket walk moved out of
      `Upscale::resolve()` as its `resolve`, and `Err(UnknownRole { role, variant: self.codename })` as its
      `graph` — and for `DiffusionVariant` in `osaka/graph.rs`, whose `resolve` and `graph` are
      `graph::resolve` and `graph::artifact_for` re-signatured to take a `Precision` rather than an
      `UpscaleVariant` and to stop calling `diffusion_row()`. Verify the resolution tests in `resolve.rs`
      pass unedited — the Kyoto 8x two-pass sequence, Tokyo's repeated artifact, the 2.5x name that is never
      a requested scale, and the Osaka INT8 set that pins only the transformer.
- [x] 4.2 Replace the six exhaustive matches in `variant.rs` — `profile`, `label`, `codename`, `conv_row`,
      `diffusion_row`, `is_diffusion` — with one `model()` returning `&'static dyn UpscaleModel`, keeping
      `label`, `codename`, `profile` and `is_diffusion` as one-line forwarders through it and deleting
      `conv_row` and `diffusion_row` outright. Leave `precision()` an exhaustive match, for the reason in
      design.md's Context. Repoint `resolve.rs`'s `Upscale::resolve`/`graph` and the two test filters that
      used `conv_row().is_some()`. Verify `model()` is exhaustive with no wildcard arm, that
      `every_variant_is_backed_by_exactly_one_contracts_row` still holds as a statement about contracts, and
      that `grep -rn "conv_row\|diffusion_row" crates/opai/src` returns nothing.
- [x] 4.3 Add `static MODELS: [&'static dyn UpscaleModel; 4]` to `models/upscale/mod.rs` in the chooser
      order (Tokyo, Kyoto, Saitama, Osaka), with a test driven from `every_variant()` asserting every
      variant's `model()` appears in it — so a model added to seam 1 and forgotten here fails rather than
      being silently absent from every chooser. Verify that test fails when an entry is removed.

## 5. The catalogue stops naming models

- [x] 5.1 Move the two widened precision lists to sit with their domains — `FloatPrecision::PRECISIONS` in
      `models/precision.rs` and `OsakaPrecision::PRECISIONS` in `osaka/precision.rs`, replacing
      `catalogue.rs`'s `FLOAT_PRECISIONS` and `OSAKA_PRECISIONS` `LazyLock`s — have `simple_family` and both
      `UpscaleModel` impls read them, and rewrite `upscale_family()` to build its four `VariantEntry` values
      from `upscale::MODELS` through `codename()`, `label()` and `precisions()`, deleting `catalogue.rs`'s
      imports of `KYOTO`, `OSAKA`, `SAITAMA`, `TOKYO` and `ConvVariant`. Verify `grep -niE
      "tokyo|kyoto|saitama|osaka" crates/opai/src/models/catalogue.rs` returns nothing outside comments,
      that the catalogue tests pinning the order and contents of the upscale entry and each family's exact
      precision list pass unedited, and that `cargo test -p perf` still selects a variant by codename.

## 6. The prose, and proving nothing was given up

- [x] 6.1 Repoint the doc links the moves broke — `models/simple.rs`'s
      `super::upscale::variant::ConvVariant`, `osaka/graph.rs`'s three `[ConvVariant]` references and its
      `UpscaleVariant::profile` citation, and `providers/options.rs`'s test comment citing that same
      `UpscaleVariant::profile` Osaka section — and update `inference/mod.rs`'s and `models/mod.rs`'s headers
      where they describe a layout that is now complete rather than in progress. Verify
      `cargo doc -p opai --no-deps` reports no new rustdoc warning against `HEAD`.
- [x] 6.2 Compare the test count before and after with `cargo test -p opai -- --list | wc -l` against the
      same on `HEAD`, including the `#[ignore]`d live tests, and verify it has not decreased — then verify
      the public API is unchanged by diffing `cargo public-api` (or `cargo doc`'s index for `opai`) against
      `HEAD`, with particular attention to `Pass`, `Resolution`, `UpscaleVariant`, `Upscale::resolve`,
      `Upscale::graph` and `Upscale::params`.
- [x] 6.3 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`
      and verify all three are clean, then verify the doctrine holds: `crates/opai/src/inference/` contains
      no file any model or family owns, and its remaining hits for `tokyo`, `kyoto`, `saitama` and `osaka`
      are only the three kinds design.md D6 allows — `live.rs`'s fixtures, `chain.rs`'s test fixtures, and
      intra-doc links and prose naming a model as an example. Also verify no `use` in any of
      `models/upscale/{tokyo,kyoto,saitama,osaka}/` reaches into another of them. No frontend scripts: this
      slice touches no frontend file.
