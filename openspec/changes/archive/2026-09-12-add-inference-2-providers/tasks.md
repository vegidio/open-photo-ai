## 1. Naming a provider

- [x] 1.1 Reshape `providers.rs` into `providers/`, moving the existing report, its detection and its tests into `mod.rs` unchanged, and add the `ExecutionProvider` enum with its five arms, its user-facing spelling, its `Display`, its `serde` derive and a case-insensitive parse whose failure is a new variant of the `thiserror` enum in `error.rs`; verify tests cover each spelling parsed in three casings, text naming no published provider refused with an error naming that text, and a round trip through serialization landing on the user-facing spelling (design.md - D1, D9)
- [x] 1.2 Add `SupportedProviders::supports(ExecutionProvider)`, answering `true` for `Auto` and `CPU` on every machine and reading the corresponding flag otherwise, leaving the crate-internal `Provider` enum narrow so a GPU-library row still cannot name `Auto` or the CPU; verify tests cover a provider whose library was never installed, both unconditional answers, and that the existing GPU install-plan suite passes unchanged

## 2. Resolving what a run attaches

- [x] 2.1 Add the `Auto` resolution: the fixed preference order TensorRT, CUDA, CoreML filtered by the machine's report, with the CPU never in the list and no per-platform table anywhere; verify tests cover a machine reporting both NVIDIA providers attaching TensorRT before CUDA, a machine reporting only CoreML attaching it alone, and a machine reporting nothing beyond the CPU attaching none (design.md - D2)
- [x] 2.2 Add the explicit-request resolution for a provider the machine supports - that provider alone, never widened into the `Auto` chain - and the CPU request that attaches nothing and produces no provider options; verify tests cover CUDA requested on a machine that also supports TensorRT leaving TensorRT unattached, and a CPU request producing an empty provider list
- [x] 2.3 Add the downgrade for a request the machine cannot honour: a CPU run carrying the requested provider beside the resolved one rather than an error or a native decline; verify tests cover CoreML requested off macOS and TensorRT requested with its libraries absent, both resolving to a CPU run that reports what was asked for (design.md - D3)

## 3. The per-model profile

- [x] 3.1 Add `EpProfile` in `providers/profile.rs` with the seven fields a measurement uses and the enums they need, each carrying the measurement that justifies it, and a `Default` that means "nobody has measured this graph"; verify a test asserts the default profile changes nothing about any provider's options (design.md - D6)
- [x] 3.2 Add `profile()` to all eight families' variant enums, returning the default everywhere, and forward it from `Operation` as the existing questions are forwarded; verify a test asks every variant of every family for its profile so the hook is exercised across all eight from the start (design.md - D7)
- [x] 3.3 Declare Kyoto's measured profile - CoreML's CPU-and-Neural-Engine compute units at FP16, the default at FP32 - with the reason the FP32 half is load-bearing rather than a formality; verify tests cover both precisions and assert no other variant's profile changed (design.md - D8)

## 4. What each provider is configured with

- [x] 4.1 Add `CachePaths` and the TensorRT option map: builder optimization level 5, engine and timing caches enabled at the two supplied paths, hardware-compatible engines off, INT8 off, a 4 GiB workspace, graph capture off with its measurement recorded, and the profile's raw overlay applied last; verify tests cover every pinned default, two models carrying their own engine path and one shared timing path, and an overlay entry overriding a default (design.md - D5, D6)
- [x] 4.2 Add the CUDA option map: exhaustive convolution algorithm search, maximum convolution workspace, no GPU memory ceiling, graph capture off, and NCHW unless the profile asks for NHWC; verify tests cover the pinned defaults, graph capture off, and the layout following the profile in both directions
- [x] 4.3 Add the CoreML option map: MLProgram pinned rather than offered, static input shapes required, subgraphs off, the model cache directory pointed at the supplied engine path, and compute units and specialization taken from the profile; verify tests cover the pinned format and static shapes, Kyoto at FP16 reaching the CPU and Neural Engine, and Kyoto at FP32 taking the default compute units
- [x] 4.4 Add the session-level settings and assemble the `SessionPlan` - providers in attach order with their resolved options, the settings, and the requested/resolved pair; verify tests cover an unmeasured model resolving to parallel execution with the memory planner on and no optimizer disabled, a model declaring sequential execution carrying it whichever provider resolved, and a plan produced on a machine with no runtime installed touching no file (design.md - D4)

## 5. Verification

- [x] 5.1 Document the capability at module level - what `Auto` resolves to and why the platform is not consulted, that an unhonourable request downgrades rather than fails, that the options are ONNX Runtime's own keys and nothing here calls the runtime, and which settings are pinned for correctness rather than offered as tuning - and verify `cargo doc -p opai` emits no warnings
- [x] 5.2 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test --locked`, and verify all three are green on the ubuntu, macos and windows CI jobs
