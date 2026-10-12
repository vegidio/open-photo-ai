## Context

See proposal.md, "Why". The state this design builds on:

- `crates/cli/src/main.rs` installs the log sink (`opai::logging::init(APP_NAME, CLI)`) and prints `opai::version()`.
  The package has no `[[bin]]`, so its binary is called `cli`.
- Everything the run needs is public in `opai`:
  - `prepare_library_path`
  - `Opai::initialize` with `InitOptions { app: Some(CLI), on_plan, on_progress }`
  - `image::probe` / `image::load` / `image::save`
  - `suggested_scale`
  - the `Upscale::{kyoto,osaka,tokyo,saitama}` and `FaceRecovery::{athens,santorini}` constructors
  - `Opai::find_chain_faces` + `default_face_selection` (slice 1)
  - `Opai::process` with `ProcessOptions { depth, on_progress, cancel }`
  - `ProviderReport` / `ProviderVerdict`
  - `InitError::AlreadyRunning` (its `Display` already names the holder and says "close it and try again")
- **Trapped in the GUI:** `crates/gui/src/export/format.rs` holds `default_quality` (JPEG 90, AVIF/HEIF 60,
  WebP 75), the 1 to 100 clamp, and `encode_options`, which also sets AVIF speed 6. These are keyed by the GUI's wire
  enum `ExportFormat`, and the only callers are `export/run.rs` and `capabilities()`.
- `rust_sak::image::ImageFormat::from_path` infers a format from a path's extension, case-insensitively.
- `perf` already establishes the binary-crate pattern: `clap` derive, a `tokio` multi-thread runtime, and
  `tokio::signal::ctrl_c` cancelling a `CancellationToken`.
- The Go reference (`../open-photo-ai` in the project context) is not on disk under that path, because it resolves to
  this repository. The flag set comes from the series plan recorded in slice 1.

## Goals / Non-Goals

**Goals:**

- The `cli-enhance` spec, end to end, on macOS, Windows and Linux.
- One source for the per-format encode defaults, shared by the GUI export and the CLI. GUI output is byte-identical
  before and after.
- Keep the CLI's own logic pure where it can be, so it can be unit-tested without a model. That means flags to chain,
  scale resolution, output-format resolution, and exit-code mapping.

**Non-Goals:**

- Batch input, globs, or directories. One photograph per invocation.
- Other enhancement families (denoise, sharpen, and so on), Autopilot, crop, a per-face selection, choosing a
  provider (`--provider`), a fidelity flag, or a quality-number flag. These are possible later changes, not part of
  this series.
- Telemetry. The GUI gates it on an opt-in the CLI has no way to ask for. The CLI sends nothing.
- Release packaging and a `just build cli` recipe. The README already mentions `just build cli`, but the Justfile
  has no such recipe. That belongs to a separate packaging change.
- Changing `perf`.

## Decisions

### D1. Encode defaults move to `opai::image`, keyed by `ImageFormat`

`opai::image` gains:

- `default_quality(format: ImageFormat) -> Option<u8>`: `Some` for JPEG, AVIF, HEIF and WebP, `None` for the
  lossless formats.
- `QUALITY_RANGE` (or `MIN_QUALITY` / `MAX_QUALITY`): 1 to 100.
- `recommended_encode_options(format: ImageFormat, quality: Option<f64>) -> EncodeOptions`: the body of today's
  `encode_options`, keyed by the library's format.

They live in a new `crates/opai/src/image/recommended.rs` and are re-exported from `image`. The GUI keeps
`ExportFormat`, `capabilities()` and `QualityRange`, since those are wire shapes. `default_quality(format)` becomes
`opai::image::default_quality(format.into())`, and `encode_options` delegates to the library. The GUI's existing
`format.rs` tests stay where they are and must pass unchanged. They are the regression check for the window.

Why `ImageFormat` and not a new enum: it is the type the CLI already gets from `from_path`, and the type `save`
takes. Keying the defaults by the GUI's enum would pull a wire shape into the library.

Alternative considered: putting it in `rust-sak`'s `image` module. Rejected. These numbers are this application's
product choice, copied from the reference's `utils.EncodeImage`, and not a general-purpose codec default. Another
project would want its own.

### D2. Module layout of `crates/cli`

```text
main.rs      prepare_library_path → logging::init → parse → runtime → run → exit code
args.rs      clap derive: Args, UpscaleModel, FaceModel, Quality; the "at least one" and early file checks
chain.rs     pure: (Args, input dims) → Plan { operations, notes }   (hd/sd mapping, scale resolution, skip note)
output.rs    pure: output path → (ImageFormat, EncodeOptions) via from_path + D1
progress.rs  indicatif rendering of InitOptions callbacks and OnInference
run.rs       the async sequence; owns the CancellationToken and the Opai handle
error.rs     CliError (thiserror) → message + exit code
```

`main` stays synchronous and calls `prepare_library_path()` as its first statement, as the library requires. It then
builds a `tokio` multi-thread runtime and `block_on`s `run`. `#[tokio::main]` is not used: on Linux,
`prepare_library_path` may re-exec the process, and that has to happen before any runtime threads exist.

### D3. Chain building is the CLI's own vocabulary and is pure

`chain::plan(&Args, width, height) -> Plan` maps:

- `Quality::{Hd,Sd}` to `FloatPrecision::{Fp32,Fp16}`, and for Osaka to `OsakaPrecision::{Fp16,Int8}`
- `--fr` to `FaceRecovery::athens(p, Faces::empty(), Fidelity::MAXIMUM)` or `santorini(p, Faces::empty())`
- `--up` to the variant at `--scale`, or `suggested_scale(width, height)`. A `None` from `suggested_scale` drops the
  upscale and adds the "already large enough" note.

This is front-end logic in the same sense as the GUI's `Requested → Operation` mapping in `enhance/operation.rs`. The
words `hd`, `sd`, `--up` and `--fr` are the terminal's, and no other front end would map them. The ladder itself is
`opai::suggested_scale`, not restated. The flags go in a fixed order, and `Opai::process` imposes apply order anyway.

Dimensions come from probing the input, which is also the early readability check (D4), and avoids decoding twice.
`image::probe` refuses camera RAW by design, so a path for which `image::is_raw` is true goes through
`image::probe_raw` instead. The image-io spec requires both to report the dimensions `load` produces.

Alternative considered: a `Requested`-like intermediate shared with the GUI. Rejected. The GUI's request is a wire
shape for the window, and the CLI's flags are a different surface onto the same constructors.

### D4. Validation happens before initialization

The order in `run` is:

1. Parse. `clap` handles unknown models, the scale range (a `value_parser` that calls `Scale::new`) and
   `requires = "upscale"` for `--scale`, all as usage errors (exit 2).
2. Check that at least one of `--up` / `--fr` was given (exit 2).
3. Resolve the output format from the extension (exit 2, listing the formats from `ImageFormat::ALL`).
4. Probe the input (exit 2, naming the path).
5. Initialize, then load, find faces, process, save.

So a typo never costs the ~175 MB first-run download. The output directory's writability is not checked early, because
that is racy anyway. A failed save is a runtime failure (exit 1).

### D5. Running the chain

- **Face detection.** `find_chain_faces(&picture, &operations, default_face_selection, Some(ExecuteOptions { on_progress, cancel }))`.
  - `Found(faces)`: print "found N faces, restoring M". M is the number of found faces `default_face_selection`
    keeps, which is exactly what the step handed the recovery.
  - `Failed(err)`: print a warning and continue with the returned chain.
  - `NotAsked`: print nothing.
  - `Err(Cancelled)`: exit 130.
- **Processing.** `process(&picture, &faced.operations, Some(ProcessOptions { depth: OutputDepth::Source, on_progress, cancel, ..Default::default() }))`.
  The provider is `Auto`. `OutputDepth::Source` is what the library's docs prescribe for a front end.
- **Saving.** `image::save(enhanced.picture.shared_pixels(), output, format, Some(recommended_encode_options(format, None)))`.
- **Completion line.** Built from `ProviderVerdict`:
  - `Resolved(providers)`: "ran on CoreML, CPU"
  - `NothingExecuted`: "served from cache"
  - `AsRequested` and `Downgraded`: unreachable under `Auto`, but they are rendered as well so the match stays total.
  - An empty chain (the only requested upscale was skipped): "no enhancement ran". The library reports an empty chain
    as `NothingExecuted`, exactly like a fully cached run, so the CLI checks for it first rather than claim a cache hit.
- **Testing.** The face report and the completion line are pure functions in `run.rs` (`face_report`, `provenance`),
  unit-tested for every `ChainFaces` and `ProviderVerdict` case. The rest of `run` needs real models and is covered
  by the manual end-to-end runs.

### D6. Ownership, lifetimes and stopping

- **The handle and the claim.** `run` owns the single `Opai` handle. It is dropped at the end of `run`, before
  `main` returns, which releases the single-instance claim and waits for pending run-cache writes, as `Drop`
  documents. No `settle_cache_writes` call is needed because the handle really is dropped.
- **Sessions.** Sessions are acquired and released inside `process` / `execute` through the library's registry. The
  CLI holds none across calls. The idle-release timer is irrelevant to a process that exits right after.
- **Cancellation.** One `CancellationToken` is shared by detection and processing. A `tokio::spawn`ed
  `ctrl_c().await` cancels it.
  - The handler is installed **after** `initialize` returns. Initialization has no cancel hook, so before that point
    Ctrl-C keeps its default meaning and terminates the process. That is safe: the claim is released by the OS, and
    transfers resume onto the bytes already on disk.
  - After the token fires, the run ends with `Cancelled` and the CLI writes nothing.
  - A second Ctrl-C while waiting for the stop is not handled specially. The default handler is no longer in place, so
    it is swallowed until the tile loop notices.
  - The token is checked once more after `process` returns, so a Ctrl-C that lands after the last tile still writes
    nothing. A Ctrl-C during `image::save` itself is ignored: saving has no cancel hook, the default handler is no
    longer in place, and the run completes with the file written and exit status 0. That is accepted. The save is
    short and stopping it midway would leave a partial file.
- **Exit codes** (`error.rs`):
  - usage → 2
  - `InitError::*`, `InferenceError::*` other than `Cancelled`, and `ImageIoError` → 1
  - `Cancelled` → 130

  `clap`'s own errors keep `clap`'s exit code, which is 2.

### D7. Crates

- **`clap` 4 (derive).** Already the workspace's choice in `perf`. It handles optional values for `--up [MODEL]`
  (`num_args = 0..=1`, `default_missing_value`), `requires`, aliases and `--version` from one struct. `argh` and
  `bpaf` were rejected for the same reasons `perf` records.
- **`tokio`** with `rt-multi-thread` and `signal`. The library is async, and `signal` gives Ctrl-C. This is the same
  crate and feature set `perf` uses.
- **`indicatif`** for progress bars. It is the standard Rust progress-bar crate. It hides bars on its own when stderr
  is not a terminal, which covers the "piped" scenario, and a `MultiProgress` holds the one-bar-per-dependency
  install view.
  - `ratatui`, which `perf` uses, was rejected. A full-viewport TUI is heavier than two bars, and it takes over the
    terminal, which a pipeline-friendly command should not do.
  - Hand-rolled `\r` bars were rejected because they would need the TTY detection and redraw throttling that
    indicatif already provides.
- **`thiserror`.** This is the project's error convention.

No `rust-sak` feature is enabled directly. The CLI reaches `ImageFormat` through `opai`'s re-export, as the library
intends. `rust-sak` has nothing for argument parsing or terminal rendering, and neither belongs there.

### D8. Binary name

`crates/cli/Cargo.toml` gets `[[bin]] name = "opai"`, matching the series plan and the README's command examples. The
package stays `cli`. There is no collision: `gui` builds `OpenPhotoAI` and `perf` builds `perftest`.

## Risks / Trade-offs

- [`clap`'s optional-value flags can swallow the next positional] → The input is positional and comes first (a
  recorded series decision). A bare `--up` followed by `-o` or `--fr` parses correctly. `opai --up photo.jpg ...`
  fails with "invalid model 'photo.jpg'", which is clear and safe. A test pins both cases.
- [Moving encode defaults could shift GUI output] → `recommended_encode_options` is a move of the same body. The
  GUI's `format.rs` tests (quality per format, clamp, NaN, AVIF speed) run unchanged against the delegating
  function.
- [Progress callbacks fire from blocking threads at high rates] → indicatif's `ProgressBar` is `Send + Sync` and
  throttles its own redraws, so the callback only sets position and message.
- [`probe_raw` and `load` disagree on dimensions for some RAW] → the image-io spec already requires that they agree.
  If a mismatch shows up, it is a library bug and is fixed there, not worked around in the CLI.
- [An input whose extension names no supported format, but which `load` could still decode] → `probe` refuses it,
  so the CLI refuses it too, with a usage error naming the file. That is stricter than `load` and accepted: a
  terminal user can rename the file, and the alternative is decoding the whole image before initialization.
- [An input whose header reads cleanly but whose pixels do not decode] → `probe` accepts it, so the refusal only comes
  from `image::load`, after initialization, and on a first run after the runtime download. It still exits 2 as an
  unreadable input, naming the path. Accepted: catching it earlier means decoding the whole image twice on every
  run to protect against a corrupt file, which is rare, and the download it costs is a one-time cost that is not
  repeated.
- [Parity with the Go CLI is unverified] → This is recorded as an open question. The flag spellings are the ones
  agreed in slice 1's exploration.

## Validation

- Unit tests cover the pure modules (`args`, `chain`, `output`, `error`) on every platform in CI.
- Manual end-to-end runs on macOS (Apple Silicon), where `Auto` resolves to CoreML with CPU fallback, use
  `fixtures/test.dat`:
  - `--up`
  - `--fr`
  - `--up osaka -q sd`
  - the skip case
  - `.webp` and `.png` outputs
  - with the GUI open (refusal)
  - Ctrl-C mid-upscale
  - piped stderr
- Windows and Linux (CUDA, TensorRT, CPU) are **not** validated by hand in this change and are covered only by the
  test suite. No provider-specific code is added.

## Slices

The `add-cli` series. It delivers `opai <input> [--up [MODEL]] [--scale N] [--fr [MODEL]] [-q hd|sd] -o <output>`.

| # | Change | Delivers | Depends on | Status |
|---|---|---|---|---|
| 1 | `add-cli-1-face-detection` | Chain face finding moved from `gui` into `opai` with a published default selection. GUI switched to it with no behaviour change. | none | **done** (archived 2026-10-12) |
| 2 | `add-cli-2-enhance` | Per-format encode defaults moved from `gui` into `opai`. The `cli` crate replaces its scaffold with the `opai` binary: argument parsing and validation before install, initialization with progress and the single-instance refusal, face finding with the default selection, processing with progress and Ctrl-C, saving like a GUI export, and exit codes. README's CLI section. | 1 | **current** |

Recorded decisions, not to be re-asked:

- flag order does not affect apply order
- the input path comes first
- an existing output is overwritten silently
- no enhancement flag is a usage error, but an upscale skipped as unnecessary still writes the output
- the encode defaults move in this slice rather than in a slice of their own

## Open Questions

- Do the Go CLI's flag spellings, messages or exit codes differ from these? This is deferrable: if they do, the
  difference is a follow-up decision about matching them, and it does not change this design's structure.
