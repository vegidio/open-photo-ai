## 1. opai: recommended encoder settings

- [x] 1.1 Add `crates/opai/src/image/recommended.rs` with `default_quality`, the 1 to 100 quality bounds, and `recommended_encode_options(ImageFormat, Option<f64>)`, moved from `crates/gui/src/export/format.rs` (design D1). Re-export them from `image` and document them in the crate docs' "Opening and writing images" section. Unit-test every image-io delta scenario: per-format defaults, lossless formats carry no quality, clamp at 0 and 150, NaN gives 1, AVIF speed 6, and a write with the settings is never refused as another format's. Verify with `cargo test -p opai image::recommended` and `cargo doc -p opai --no-deps` with no broken links.
- [x] 1.2 Switch `crates/gui/src/export/format.rs` to the library: `default_quality` and `encode_options` delegate through `ImageFormat::from(ExportFormat)`, and the GUI-local constants are removed. Keep every existing `format.rs` test unchanged. Verify with `cargo test -p gui export::format`.

## 2. cli: arguments and pure planning

- [x] 2.1 In `crates/cli/Cargo.toml`, add `[[bin]] name = "opai"` and the `clap` (derive), `tokio` (`rt-multi-thread`, `signal`), `indicatif` and `thiserror` dependencies, each with a comment explaining the choice (design D7, D8). Restructure `main.rs` to: `prepare_library_path` first, then `logging::init`, parse, build the runtime, run, and exit with the run's code (design D2). Verify that `cargo build -p cli` produces `target/debug/opai`, and that `opai --version` prints `opai::version()` and installs nothing.
- [x] 2.2 Add `args.rs`:
  - the clap struct with `--upscale/--up [MODEL]` (default tokyo), `--scale` (validated through `Scale::new`, `requires` upscale), `--face-recovery/--fr [MODEL]` (default athens), `-q hd|sd` (default hd) and a required `-o`
  - case-insensitive model names
  - the "at least one enhancement" check

  Unit-test with `Args::try_parse_from` for:
  - bare flags taking their defaults
  - named models
  - unknown models
  - `--scale` without `--up`
  - a scale out of range
  - no enhancement
  - `opai photo.jpg --up -o x` parsing correctly
  - `opai --up photo.jpg ...` failing as an invalid model

  Verify with `cargo test -p cli args`.
- [x] 2.3 Add `chain.rs`: a pure `plan(&Args, width, height) -> Plan { operations, notes }`. It maps hd/sd to `FloatPrecision`, and for Osaka to `OsakaPrecision`. Face recovery uses `Fidelity::MAXIMUM` and empty faces. The scale comes from `--scale` or `opai::suggested_scale`, and a `None` drops the upscale and adds the "already large enough" note (design D3). Unit-test:
  - every row of the quality table
  - 4x at 1000x800, 2x at 2000x1500, and a skip at 4000x3000
  - an explicit scale honoured at 4000x3000
  - both flag orders giving equal operation lists

  Verify with `cargo test -p cli chain`.
- [x] 2.4 Add `output.rs`, which resolves the output path to `(ImageFormat, EncodeOptions)` via `ImageFormat::from_path` and `recommended_encode_options`. Add the input check, which uses `probe` or `probe_raw` depending on `is_raw` and returns the dimensions (design D3, D4). Unit-test:
  - `.webp`, `.JPG` and `.heic` resolution
  - an unknown or missing extension refused, with the error listing the writable formats
  - a missing input refused, naming the path
  - `fixtures/test.dat` is refused because `.dat` names no supported format, and copied to a `.jpg` temp path it is accepted with its real dimensions

  Verify with `cargo test -p cli output`.
- [x] 2.5 Add `error.rs` with a `CliError` (thiserror) for usage, init, inference, cancelled and write failures, and its exit-code mapping: 0, 2, 130 and 1, with clap errors keeping 2 (design D6). Each variant's message is one line naming the path or the cause. Unit-test the mapping, including that `InitError::AlreadyRunning` maps to 1 and keeps the library's message, and that `InferenceError::Cancelled` maps to 130. Verify with `cargo test -p cli error`.

## 3. cli: running and rendering

- [x] 3.1 Add `progress.rs`:
  - **Initialization:** a `MultiProgress` view built from `InitOptions::on_plan` and `on_progress`, with one bar per planned dependency showing the download and expand phases, and `AlreadyInstalled` shown as done without a transfer.
  - **Inference:** an `OnInference` renderer with one bar for the whole chain, whose message names the operation and its `Stage` (downloading, running, or from cache).
  - **Messages:** notes and warnings printed through `MultiProgress::println` / `suspend` so they don't tear the bars.
  - **Non-terminal stderr:** bars hidden when stderr is not a terminal (indicatif's draw target).

  Verify by running `opai fixtures/... --up -o /tmp/x.png 2> log.txt` and confirming the log holds the messages and no redraw sequences.
- [x] 3.2 Add `run.rs`, the async sequence (design D4 to D6):
  1. validate
  2. `Opai::initialize(APP_NAME, InitOptions { app: CLI, on_plan, on_progress })`
  3. install the Ctrl-C → `CancellationToken` handler
  4. `image::load`
  5. `find_chain_faces` with `default_face_selection`, reporting found/restoring, nobody found, or a failure warning
  6. `process` with `OutputDepth::Source` and the shared token
  7. `image::save` with the recommended options
  8. print the completion line from `ProviderVerdict`
  9. drop the handle before returning

  Verify with `cargo build -p cli`, then a run of `opai <test.jpg> --up -o /tmp/out.png` that writes a 4x image.
- [x] 3.3 Replace the README's "CLI: Coming soon" section with usage covering:
  - the synopsis
  - each flag with its default and accepted values
  - the hd/sd table
  - the skip rule
  - the output format following the extension
  - the exit codes
  - two or three examples

  Verify that the README renders and every flag it names exists in `opai --help`.

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets` and `cargo test --workspace`. All must be clean and passing.
- [x] 4.2 Manual end-to-end on macOS (CoreML via Auto), with `fixtures/test.dat` copied to a `.jpg`:
  - `--fr` reports faces found and restoring, and writes the result
  - `--up` alone at the suggested scale
  - `--up osaka -q sd`
  - `--up --fr -o out.webp` gives a WebP file
  - a >4 MP image with `--up` notes the skip and writes the original size
  - with the GUI open, the run is refused with the holder named, and nothing is written
  - Ctrl-C during an upscale prints the stop, writes nothing, and exits 130
  - a second identical run reports being served from cache
  - `echo $?` matches the spec for each case

  Also confirm that a GUI export of one photograph as JPEG, AVIF and WebP is byte-identical before and after task 1.2.
