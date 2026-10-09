# Contributing to Open Photo AI

Thanks for your interest in contributing! This guide explains how to set up the project, what we expect from a pull request, and which kinds of changes need to be discussed first.

If you found a bug or have a question or suggestion, please open an issue using one of the [issue templates](https://github.com/vegidio/open-photo-ai/issues/new/choose).

## Open an issue before you start

> [!IMPORTANT]
> If your PR **adds a new model** (or a new enhancement) or **adds support for a new Execution Provider**, you must open an issue before writing any code.

These changes affect model size, download size, performance and platform support, so they need to be discussed first. Use the issue to agree on:

- whether the change is feasible and fits the project;
- the best approach (model architecture, conversion strategy, target platforms/hardware, etc.);
- how the change will be tested and validated.

Please wait until the approach is agreed on in the issue before you start implementing. PRs of this kind that skip this step may be closed.

Opening an issue first is also recommended for large refactors or new UI features. Small bug fixes, typos and translation improvements can go straight to a PR.

## Project layout

| Path             | Description                                                                                   |
|------------------|-----------------------------------------------------------------------------------------------|
| `crates/opai`    | Core library: inference, models, execution providers, model/dependency download, image IO.    |
| `crates/imaging` | Pixel primitives: tiling, tensor conversion, padding, blending.                               |
| `crates/gui`     | Desktop app built with Tauri 2. The Rust side is in `src/`; the React frontend in `frontend/`. |
| `crates/cli`     | Command-line interface.                                                                        |
| `crates/perf`    | `perftest`, the inference benchmark (see its [README](crates/perf/README.md)).                |
| `docs/`          | Project website and install scripts.                                                          |
| `fixtures/`      | Test assets. `fixtures/test.dat` is a JPEG image, despite its extension.                      |

## Development setup

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain, edition 2024)
- [Just](https://just.systems/man/en/installation.html)
- [Node.js](https://nodejs.org) 24
- [pnpm](https://pnpm.io). The version is pinned in `crates/gui/package.json`. Always use `pnpm`/`pnpx`, never `npm`/`npx`.

On Linux you also need these system packages (Debian/Ubuntu names):

```sh
sudo apt install -y build-essential pkg-config libwebkit2gtk-4.1-dev libayatana-appindicator3-dev \
  librsvg2-dev libxdo-dev libssl-dev
```

### Common commands

Run these from the repository root:

```sh
just run gui                # Run the GUI in development mode
just build gui [arm64|x64]  # Build the GUI into build/ (defaults to the host architecture)
just test                   # Run all tests (Rust + frontend)
just test rust              # Run only the Rust tests
just test node              # Run only the frontend tests
just clean                  # Delete build output and generated artifacts
```

`just test rust` builds the frontend before running `cargo test --workspace`, because the GUI crate embeds `crates/gui/dist` at compile time. If you run `cargo test` directly, run `pnpm build` in `crates/gui` first.

## Code style

### Rust

- Format with `cargo fmt --all`. The configuration in `rustfmt.toml` uses a max width of 120.
- Run `cargo clippy --workspace` and don't introduce new warnings.
- Every `unsafe` block needs a `// SAFETY:` comment, and `todo!`, `unimplemented!` and `dbg!` shouldn't be committed.
- Don't call `ort::init_from` directly. The ONNX Runtime is loaded only through `runtime::start` (this is enforced by `clippy.toml`).

### Frontend

Run these from `crates/gui`:

```sh
pnpm check      # Biome lint + format + organize imports
pnpm typecheck  # TypeScript type check
pnpm test       # Vitest
```

### General

Follow the `.editorconfig`: UTF-8, 4-space indentation (2 for YAML), a max line length of 120, and a final newline.

## Translations

Locale files live in `crates/gui/frontend/i18n/locales/`. When you add or change a string, **add it to every language file**, not only `en.json`. `parity.test.ts` fails if a key is missing from any locale. If you can't translate a string, say so in the PR and we'll help.

## Adding a new model

First open an issue (see [above](#open-an-issue-before-you-start)). Once the approach is agreed on:

### Code structure

Models are grouped by enhancement family in `crates/opai/src/models/<family>/`. Each model has its own directory named after its codename (models are named after cities, e.g. `upscale/kyoto`, `light_adjustment/paris`), and is registered in the family's `variant.rs`.

### ONNX requirements

- Export to ONNX using **opset 18**.
- Provide both **FP32 and FP16** versions. In the FP16 version the edge (input/output) nodes must stay in FP32.
- Prefer **fixed input shapes** over dynamic ones, as long as output quality doesn't suffer. Use tiling if the architecture needs it.
- The model must run as a **single partition**. Multiple partitions hurt performance and usually mean the architecture needs fixing.
- Simplify the model with [`onnx-simplifier`](https://github.com/daquexian/onnx-simplifier).
- Check the compiled CoreML cache. `weights/weight.bin` should hold essentially all of the weight bytes, and `model.mil` should be a few MB at most. A huge `model.mil` means weights are being inlined.
- FP16 won't match FP32 exactly, but its output must be visually acceptable. The FP16 version is always shipped, even if it's slower, so try to optimize it.

### Testing

- You can prototype with PyTorch, but the final ONNX models must be tested **from Rust**.
- Test both FP32 and FP16 with the **CPU** and **CoreML** execution providers.
- Use `fixtures/test.dat` as the test image.
- Benchmark with `perftest`, e.g. `cargo run --release -p perf -- <codename>`, and include the results in the PR.

In the PR description, link to the original model and state its license, which must be compatible with AGPL-3.0.

## Adding a new Execution Provider

First open an issue (see [above](#open-an-issue-before-you-start)). Once the approach is agreed on, the relevant code is in `crates/opai/src/providers/`:

- `mod.rs`: the `ExecutionProvider` enum and platform support checks;
- `options.rs`: how providers are attached to sessions, and the fallback ladder used by `Auto`;
- `profile.rs`: per-model provider tuning.

In the PR, list the operating systems and hardware you tested on and include `perftest` results comparing the new provider with the existing ones.

## Commits

We use [Conventional Commits](https://www.conventionalcommits.org), and the changelog is generated from them. Start the subject with a capital letter:

```
feat: Add batch export to the GUI
fix(gui): Keep the zoom level when switching images
perf(colorization): Reduce tile overlap
```

Common types are `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build` and `chore`.

## Pull requests

- Keep each PR focused on a single change.
- Describe what changed and why, and link the related issue.
- List the platforms you tested on (macOS, Windows, Linux).
- Include screenshots or a short recording for UI changes.
- Make sure `just test` passes and the code is formatted and lint-free before opening the PR. CI doesn't run on pull requests, so this is on you.

## License

By contributing, you agree that your contributions will be licensed under the [AGPL-3.0 License](LICENSE).
