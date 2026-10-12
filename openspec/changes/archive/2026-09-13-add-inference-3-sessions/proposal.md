## Why

`opai` can name a model, put its verified weights on disk, and decide exactly what it should run on —
and it cannot open one. Slice 2 produces a `SessionPlan`: the providers to attach in order, each with
the option map it was measured with, and the session settings the graph wants. Nothing consumes it.
No code in this repository has ever called `ort::Session::builder()`.

This slice closes that gap: a named artifact becomes a loaded, runnable session, built on the
provider the machine could serve, kept resident so the next tile and the next image do not pay for it
again, and released when nothing is using it. It is also the first time any of slice 2's option keys
reach ONNX Runtime — until now every one of them is a string nobody has validated.

## What Changes

Task estimate: **15**. The inference feature estimates far past 15 and is split into six slices
(see design.md - Slices); this proposal covers slice 3 only.

**In `opai`:**

- **Session construction.** A `SessionPlan` and a model file become an `ort::Session`: each provider
  attached in order with its option map, the session settings applied, and the graph committed from
  the file slice 1 installed. The session reports which provider it was actually built on.
- **Where a provider's compiled output lives.** Slice 1 deferred this and slice 2 took the two
  directories as arguments; this is the slice that first compiles an engine, so it decides. An
  `engines/` tree beside `models/` and `runtime/`, holding `engines/<artifact-id>/` per model and
  one shared `engines/.timing/`.
- **Invalidating it.** A model whose weights are replaced loses what a provider compiled from them,
  and the whole tree is emptied when the ONNX Runtime version behind it changes — a TensorRT engine
  is valid only for the runtime that built it.
- **CPU fallback.** A session that cannot be built on the requested provider is rebuilt with nothing
  attached rather than failing the run, and the result carries what was asked for beside what it was
  built on. Failures that are not about the provider are not retried.
- **The resident session cache**, keyed on the artifact and the provider the session was built on,
  with concurrent requests for one key served by a single build. A session stays resident for **15
  minutes after it was last used**, is never reclaimed while something is using it, and is released
  when the last user lets go.
- **Install, then build.** A session request installs the artifact's weights first, through slice 1's
  path — which becomes reachable rather than dead code. One place turns a model name into a runnable
  session, so a caching or fallback decision cannot differ between two callers.
- **Serialized TensorRT builds.** The timing cache is one file shared by every model and ONNX Runtime
  rewrites it with no locking of its own, so two concurrent builds that attach TensorRT would
  interleave into it. A failed build with TensorRT in the chain drops that cache.

**Not exposed publicly in this slice.** The entry point is `pub(crate)`, as slices 1 and 2 were.
Slice 5 is what gives it a caller and a public surface.

## Capabilities

### New Capabilities

- `model-sessions`: turning a named model artifact into a loaded, runnable ONNX session and deciding
  how long it lives — building one from the resolved plan, what happens when the requested provider
  cannot build it, where what a provider compiles from a model is cached, keeping a built session
  resident and reclaiming it when it goes unused, and serving concurrent requests for one model from
  a single build.

  Distinct from both neighbours, and neither is widened. `execution-providers` decides *what* a model
  should run on and with which settings, and does it without a runtime present or a model file open.
  `model-install` puts verified bytes on disk and explicitly opens nothing. This capability is where
  the two meet a live ONNX Runtime.

### Modified Capabilities

- `model-install`: adds a requirement that replacing a model's weights invalidates what an execution
  provider compiled from the previous ones. The install pipeline is what removes it, because it is
  what knows a replacement happened; slice 1 could not state it because nothing compiled anything yet.
- `initialization`: adds a requirement that the compiled-engine tree is emptied when the ONNX Runtime
  version behind it changes. It belongs to initialization because that is where the runtime version
  in force is decided, and because the check must be durable rather than inferred from "the runtime
  was just replaced" — a process that died in between would otherwise lose the invalidation for good.

## Impact

**Crates touched:** `opai` only. `gui` and `cli` are untouched, and no logic is placed in a front
end: a session cache and a provider fallback are exactly what both front ends would otherwise each
have to own.

**`rust-sak` - what was found before anything was proposed.**

- `memo` is the near miss, and it is **not** the answer here. It offers what this slice's cache looks
  like from a distance — a TTL, and concurrent calls for one key coalesced into a single computation
  — but `get_or_compute` is bound on `T: Serialize + DeserializeOwned` because it encodes values into
  a store, and a live ONNX session is a native handle that cannot be encoded. Its TTL also runs from
  the write rather than from the last use, which is the opposite of what a resident model wants. This
  is not a gap to fill in `rust-sak` either: what this cache is actually about is the lifetime of a
  native resource, which is an `Arc`, a `Mutex` and a map — not a storage engine.
- `fs::mk_user_config_dir` already backs `config::sub_dir`, which is what resolves and creates the
  new `engines/` tree. Nothing new is needed for it.
- Emptying a directory while keeping the directory itself already exists **in this project**, as
  `deps::manifest::empty_dir`, written for the installer and reused here rather than rewritten. It is
  not a `rust-sak` gap: what it skips and why is this project's install bookkeeping.
- `crypto`, `fetch` and `sysinfo` are untouched by this slice.

**New third-party crates:** none. `ort` gains its `coreml`, `cuda` and `tensorrt` features, which are
marker features that compile no code and link nothing — they only gate the provider types this slice
configures.

**Parity divergences from the Go implementation, deliberate:**

- **No latched provider failure.** The reference latches the first execution provider that fails to
  build a session, so every later model in the run goes straight to the CPU without attempting it.
  Here each session build attempts the provider it was asked for. The latch exists to avoid paying a
  failed provider initialization per model, and the reference clears it only when the user changes
  processor — an action this project has no settings surface for yet, so a latched downgrade would be
  unclearable for the life of the process. The cost is bounded instead by the fallback re-entering
  the cache under the CPU key (design.md - D6): a repeated request pays one failed build, not a
  rebuild.
- **A flat 15-minute idle life, from the last use.** The reference scales it: a five-minute floor,
  extended to twenty times what the build cost, capped at thirty minutes. That scaling exists to
  cooperate with a memory budget this series defers, and it makes "why is this model still resident"
  a calculation rather than a fact. One number is answerable, and 15 minutes is inside the range the
  reference's own floor and ceiling bracket.
- **Sessions are cached per artifact, not per operation.** The reference keys its registry on the
  operation id, so one entry owns every graph that operation needs. Keying on the artifact means two
  operations that need the same graph share one session rather than loading the weights twice — an 8x
  Kyoto and a 4x Kyoto share the 4x graph, and today the second would build its own copy.
- **No leaked-lease diagnostics.** The reference reports a model held for more than fifteen minutes
  without a sign of life, because a lease it forgot to release pins a model for the rest of the
  process. That bug class does not exist here: a handle is an `Arc`, and letting go of it is what
  dropping it means.
- **Nothing is logged.** The reference logs which provider a session got, what the build cost, and
  every eviction and downgrade. This project has no logging yet — introducing it is a cross-cutting
  change of its own — so each of those facts is carried as data a caller can read instead. Recorded
  as a parity gap rather than pretended away; see design.md - Risks.

**Not in scope:** tiling, tensor conversion and blending (slice 4); the Kyoto pipeline and the public
inference entry point (slice 5); the per-operation image cache (slice 6). Also out, and each deferred
with a reason in design.md: the model memory budget and its LRU eviction; evicting the models built
on a provider the user just switched away from; logging and telemetry; and any `gui` or `cli` change.
