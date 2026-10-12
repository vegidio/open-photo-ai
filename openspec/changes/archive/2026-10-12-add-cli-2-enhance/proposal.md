## Why

The README's CLI section still says "Coming soon", and `crates/cli` only prints the version. Slice 1 moved chain face
finding into `opai`, so every step a terminal run needs is now in the library. This slice is the CLI itself:
`opai photo.jpg --up --fr -o out.avif` enhances one photograph and writes it.

This change is slice 2, the last, of the `add-cli` series. See design.md, "Slices".

## What Changes

- **`cli` replaces its scaffold** with a binary named `opai`:
  `opai <input> [--upscale|--up [kyoto|osaka|tokyo|saitama]] [--scale 1.0-8.0] [--face-recovery|--fr [athens|santorini]] [--quality|-q hd|sd] --output|-o <file>`
  - `--up` with no model means Tokyo. `--fr` with no model means Athens. `-q` defaults to `hd`.
  - `hd` means FP32 and `sd` means FP16. For Osaka, `hd` means FP16 and `sd` means INT8, its two published builds.
    Face recovery runs at maximum fidelity, which is the library's default.
  - `--scale` requires `--up`. Without `--scale`, the library's `suggested_scale` picks the factor. An image already
    large enough skips the upscale and says so.
  - Giving neither `--up` nor `--fr` is a usage error. If `--up` is skipped as unnecessary and no `--fr` was given,
    the output is still written (re-encoded), with the note.
  - The output format comes from the `-o` extension. An unknown or missing extension is a usage error, and so is an
    unreadable input. Both are reported before anything is downloaded.
  - An existing output file is overwritten without asking.
  - Flag order does not change the order operations apply in. The library decides that. The input path comes first,
    so a bare `--up` or `--fr` never takes it as a model name.
- **The run:**
  1. Initialize, showing install progress for the runtime and GPU libraries.
  2. Load the input.
  3. Find faces with the default selection and report what was found.
  4. Process on the best available provider, showing per-operation progress, including model downloads.
  5. Save at the source's bit depth with the same per-format encoder settings a GUI export uses.
  - If the GUI (or another instance) holds the single-instance claim, the run stops with the library's message
    naming the holder.
  - Ctrl-C during the run stops it cleanly and writes nothing.
  - `--version` prints the same version the GUI's navbar shows.
- **`opai` gains the per-format encoder defaults** that today live only in `crates/gui/src/export/format.rs`:
  - each lossy format's default quality: JPEG 90, AVIF and HEIF 60, WebP 75
  - the 1 to 100 quality clamp
  - AVIF at speed 6
  Both front ends need these, so under the layering rule they move before the CLI is written against them. The GUI
  keeps its `ExportFormat` wire enum and its format table, and reads the defaults from the library. What the GUI
  writes does not change.
- The README's CLI section documents the command.

Task estimate: about 13 tasks, within the 15-task budget.

## Capabilities

### New Capabilities

- `cli-enhance`: the terminal front end's single-image enhance command. It covers arguments and their defaults,
  validation before any install, the run sequence, progress and messages, the single-instance refusal, stopping,
  and exit status.

### Modified Capabilities

- `image-io`: adds a requirement that the library publishes each format's recommended encoder settings (default
  quality, the quality range, and AVIF speed), so every front end writes a given format the same way. The existing
  "Encoder settings" requirement is unchanged: writing with no settings still uses the codec's own defaults.

## Impact

- **Crates touched:**
  - `opai`: new public encode-defaults items under `image`, plus their tests and crate docs.
  - `gui`: `export/format.rs` delegates to the library. No visible change.
  - `cli`: rewritten.
  - `perf`: not touched.
- **Logic in a front end:** the CLI holds only parsing, the mapping from `hd`/`sd` and model names to `opai`
  constructors, terminal rendering and exit codes. Building the chain from flags is the CLI's own vocabulary, in the
  same way the GUI maps its wire `Requested` to `Operation`. The scale ladder, face selection, apply order, format
  inference and encode defaults all come from `opai` or `rust-sak`.
- **rust-sak:** `image` already covers inferring the output format from a path (`ImageFormat::from_path`) and the
  encoders. It has nothing for argument parsing or terminal progress, and should not: those are front-end concerns,
  not reusable infrastructure.
- **New dependencies (cli only):** `clap` (derive, already used by `perf`), `tokio` (already in the workspace),
  `indicatif` for progress bars, and `thiserror`. See design.md, D7.
- **Parity:** the Go CLI was not available to read during planning (`../open-photo-ai` resolves to this repository).
  The flag set follows the series plan recorded in slice 1. Any divergence from the Go CLI's spelling or output is
  recorded as an open question in design.md.
- **Not in this change:** release packaging of the CLI binary and a `just build cli` recipe. The README mentions the
  recipe, but the Justfile has none. See design.md, Non-Goals.
