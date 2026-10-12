## 1. The role on the row

- [x] 1.1 Add `Provider` to `opai/src/providers.rs` — the execution provider a GPU library unlocks (`Cuda`, `TensorRt`) — and a `SupportedProviders::with(provider)` that sets the matching field. Verify with unit tests that each variant sets its own field and nothing else, and that the base report from `detect()` still claims neither GPU provider (design D1, D2).
- [x] 1.2 Change `Artifact::lib` from `&'static str` to `Option<&'static str>` and add `provides: Option<Provider>` to `Release`, filling both in for all four pinned releases: the runtime names its per-platform library and provides nothing; CUDA provides `Cuda`, TensorRT provides `TensorRt`, and cuDNN provides nothing of its own. Verify the existing table tests still pass with `None` replacing the empty-string assertions (design D1).
- [x] 1.3 Carry both onto `deps::release::Dependency` in `from_release_at`, beside `dir` and `progress`. Verify the release tests assert the runtime descriptor carries `Some(lib)` on all five published platforms and each GPU library carries `None`.
- [x] 1.4 Change `runtime::library_path` to take the library name rather than re-checking for an empty one, deleting the `UnsupportedPlatform` branch that `runtime.rs` documents as unreachable. Verify the "a dependency that names no library is rejected rather than joined" test is replaced by one asserting the situation is now unrepresentable — a GPU descriptor has no library name to pass (design D1).

## 2. The plan

- [x] 2.1 Add `Plan { runtime: Descriptor, gpu: Vec<Descriptor> }` to `opai/src/lib.rs` with `all()` iterating runtime-first and `providers()` deriving the report by folding each GPU row's `provides` onto `SupportedProviders::detect()`. Verify with unit tests that `all()` yields runtime, CUDA, cuDNN, TensorRT in that order — cuDNN links against the CUDA runtime and TensorRT is larger than the other three together — and that `providers()` claims a provider only where its row is in the plan (design D2).
- [x] 2.2 Change `select_at` to return `Plan` instead of `(Vec<Descriptor>, SupportedProviders)`, deleting the hand-written `providers.cuda = true` / `providers.tensorrt = true` assignments. Verify every existing selection test passes unchanged in meaning: an RTX machine selects all four, a non-RTX NVIDIA machine gets CUDA and cuDNN only, a machine with no NVIDIA adapter gets the runtime alone, and an unpublished GPU platform reports the provider unsupported rather than failing.
- [x] 2.3 Change `Opai::install` to take a `Plan` and `Opai::initialize` to use `plan.runtime` by name, deleting `let runtime = plan[0].clone()` and the comment defending the positional invariant. Verify the install order is unchanged by the existing progress test, which asserts the four dependencies never interleave and each ends on exactly `1.0`.
- [x] 2.4 Update the `lib.rs` test that asserts `report.cuda == plan.contains("cuda")` so it reads as a check of the derivation rather than of two hand-maintained things agreeing, and add one asserting a plan built with a GPU row always reports that row's provider (design D2).

## 3. Error context

- [x] 3.1 Change `InitError::Io` to `Io { path: PathBuf, source: std::io::Error }` and drop its `#[from]`. Verify with a unit test that its `Display` names the path, and that `source()` still reaches the underlying `std::io::Error` so a front end walking the chain sees what the OS said (design D4).
- [x] 3.2 Fill the path in at every construction site — the manifest write and clear, the archive deletion after extraction, the directory creation in `config::sub_dir`, the fallback hash in `install::transfer`, and `manifest`'s own `io::Result` helpers as they surface. Verify `cargo build` has no remaining `?`-on-`io::Error` conversions and that the existing failure-path tests still produce the variants they assert on (design D4).
- [x] 3.3 Add `InitError::Cancelled` for the `JoinError` non-panic case in `task::spawn_blocking`, replacing `io::Error::other(err)`. Verify with a unit test that a `spawn_blocking` on a shut-down runtime yields `Cancelled` rather than `Io`, and that a panicking closure still resumes the panic rather than becoming either (design D4).
- [x] 3.4 Update `Opai::initialize`'s doc comment, which currently promises "Every one of them names the URL of the artifact that failed" — true of `Download` and the runtime variants, and now true of `Io` in the sense that it names the file. Verify the doc lists `Cancelled` and no longer overclaims.

## 4. The typed platform key

- [x] 4.1 Replace `Artifact::platform: &'static str` with a `Platform { os: Os, arch: Arch }` pair carrying a `const fn` that spells the asset segment, and rewrite `asset_platform` as `Platform::from_consts(os, arch) -> Option<Platform>`. Verify every pinned platform still resolves to the same asset name it does today — the release assets already exist and their names cannot change — and keep the guard test asserting the mapper is total (design D5).

## 5. Gates

- [x] 5.1 Run `cargo fmt --all --check`, `cargo clippy --all-targets --locked -- -D warnings` and `cargo test --locked`, and confirm the `#[ignore]`d live runtime test still compiles. Verify no test's *meaning* was weakened to accommodate the refactor: the suite should still fail if a provider is claimed without its library, if the install order changes, or if an error stops naming what it was working on.
