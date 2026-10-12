## 1. `rust-sak` — the conversion (in `../rust-sak`, its own repository)

- [x] 1.1 Add `make_compatible(image: &DynamicImage, format: ImageFormat) -> Option<DynamicImage>` to `src/image/dispatch.rs`, implementing design D2's table — BMP and JPEG always `None` (D3), every other format narrowing to the widest colour type it accepts. Verify with a unit test that asserts the returned colour type for all 10 `DynamicImage` variants × 8 formats against the table, including the `None` cells.
- [x] 1.2 Call it from `encode_into` and from `encode_to_vec`'s three seek-bound arms (BMP, GIF, TIFF) so no codec is reachable without it (D4). Verify by writing every colour type in every format through both `encode_file` and `encode_writer` over a non-seekable sink, asserting every encode succeeds and every result decodes back to the fixture's dimensions.
- [x] 1.3 Add the property tests the contract turns on: a 16-bit picture written as PNG and TIFF probes back at 16 bits; written as GIF and WebP probes back at 8; a 16-bit picture with alpha keeps its alpha through PNG and WebP; an 8-bit RGB picture written as PNG reads back pixel-identical (no conversion ran).
- [x] 1.4 Add the regression guard for D3: assert BMP and JPEG still encode a 16-bit picture with `make_compatible` returning `None` for them, so a future `image` release dropping either encoder's own hook fails here rather than silently.
- [x] 1.5 Update `src/image/README.md` — the encoding section states that a picture is converted to what the target accepts, with the depth-and-alpha rule and a note that GIF and WebP are 8-bit only; remove any wording implying an encode can fail on colour type.
- [x] 1.6 Run `cargo fmt --check`, `cargo clippy` and `cargo test --features image-raw` in `../rust-sak` and verify all pass on this machine.

## 2. `rust-sak` — release

- [x] 2.1 Confirm with the user before pushing, then bump `Cargo.toml` to `26.9.4`, push, and tag `26.9.4`. Verify the tag is visible on the remote (`git ls-remote --tags origin 26.9.4`) and that `rust-sak`'s CI is green on macOS, Windows and Linux.

## 3. `opai` — adopt

- [x] 3.1 Move the `rust-sak` pin in `crates/opai/Cargo.toml` from tag `26.9.3` to `26.9.4`, keeping the `image-raw` feature list unchanged. Verify with `cargo build -p opai` and that `Cargo.lock` records the new tag.
- [x] 3.2 Replace `a_developed_raw_is_sixteen_bit_and_two_of_the_eight_encoders_refuse_it` in `crates/opai/src/image/mod.rs` with `a_developed_raw_is_sixteen_bit_and_every_encoder_writes_it`: the developed DNG fixture is still asserted 16-bit RGB, and all eight formats now write a non-empty file that reloads at `DNG_FIXTURE_SIZE`. Verify the refusal branch and its `Unsupported(Color(Rgb16))` assertion are gone and the test passes.
- [x] 3.3 Rewrite the `crates/opai/src/image/mod.rs` module-documentation section "A developed RAW is 16-bit, and two encoders refuse that" to state that every format writes a 16-bit picture, which formats keep 16 bits (PNG, TIFF, AVIF, HEIF) and which narrow to 8 (BMP, GIF, JPEG, WebP), and drop the "known limitation"/"next slice" wording. Verify `cargo doc -p opai --no-deps` builds without warnings and no stale cross-reference to the removed limitation remains (`grep -rn "refuse" crates/opai/src/image/`).
- [x] 3.4 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace` and verify all pass. No frontend scripts: this change touches no frontend code.

## 4. Close out

- [x] 4.1 Verify the delta spec's scenarios are each covered by a passing test — the `rust-sak` tests from 1.2–1.4 for the general conversion contract, and 3.2 for the developed-RAW requirement — and note in the change which test covers which scenario.

### Scenario coverage

Which test covers which scenario of the delta spec. The general conversion contract is `rust-sak`'s, so it is
tested there against all eight formats; what `opai` tests is what `opai` promises — that a picture it loaded can be
saved in every format it offers.

**Requirement: Writing a picture a format cannot hold natively** — all in `../rust-sak/src/image/tests.rs`:

| Scenario | Test |
|---|---|
| Every format writes a 16-bit picture | `every_color_type_encodes_in_every_format_through_both_paths` (the `L16`/`La16`/`Rgb16`/`Rgba16` rows), plus `bmp_and_jpeg_write_a_sixteen_bit_picture_with_their_own_hook` for the two the dispatch leaves alone |
| Every format writes a grayscale picture | `every_color_type_encodes_in_every_format_through_both_paths` (the `L8`/`La8`/`L16`/`La16` rows) |
| Precision is kept where the target can hold it | `depth_survives_where_the_target_format_holds_it` |
| Precision is narrowed only where the target forces it | `depth_is_narrowed_only_where_the_target_format_forces_it` |
| Transparency survives where the target can hold it | `alpha_survives_where_the_target_format_holds_it` |
| A picture the format accepts is written unchanged | `a_picture_the_format_accepts_is_written_unchanged`, with `every_color_type_converts_to_what_its_target_format_accepts` pinning every `None` cell of the table |

**Requirement: Depth of a developed camera RAW** — both scenarios in
`crates/opai/src/image/mod.rs::a_developed_raw_is_sixteen_bit_and_every_encoder_writes_it`: it asserts the developed
DNG fixture is 16-bit RGB, then writes it in all eight formats and reloads each at `DNG_FIXTURE_SIZE`.
