## Context

The state slices 2 to 4 left `gui` in:

- **Telemetry starts in `setup`.** `lib.rs` calls `opai::telemetry::start()` at the top of the builder's `.setup` hook.
  It calls `stop()` on `RunEvent::Exit`. `start` latches once per process and `stop` is idempotent. `o11y` cannot be
  restarted after `stop`.
- **The Analytics choice lives only in the window.** It sits in `stores/settings.ts` under `localStorage`'s
  `settings-storage` key, and defaults to on. The code says "nothing reads it yet - by design". The `gui-settings`
  requirement says the same: send nothing "until it has something to send".
- **23 commands are registered** in `generate_handler!`, and none opens a span. Nine error enums cross the boundary:
  `SetupError`, `EnhanceError`, `SuggestError`, `DetectError`, `ExportError`, `ImagesError`, `RevealExportError`,
  `LinksError` and `LogsError`. Two answers carry a stop as `Ok`: `Enhancement::Stopped` and `Exported::Stopped`.
- **Slice 3 removed `gui`'s repeats of `opai`'s failures.** What `gui` still records is ad hoc:
  - The export's claim and commit failures are recorded at `error`, inside the body.
  - The two reveals of unknown files are recorded at `warn`, inside the body.
  - `open_link` and `reveal_log` record nothing.
  - Four command bodies write their own records with an explicit `run = request.run`: `enhance/run.rs`,
    `autopilot.rs`, `export/run.rs` and `faces.rs`.
- **The file writes span fields, never span names.** `logging::format::Line` appends every enclosing span's formatted
  fields to each record, outermost first. `fmt::layer` keeps those formatted fields up to date through `on_record`.
  The file's filter shuts out only `opai::unit`, so a span under `gui`'s own target reaches both layers. Slice 4's D2
  set this up deliberately for this slice.
- **Tauri's own `tracing` feature is no help.** It opens `debug`-level `ipc::request` spans, which the collector's
  fixed `info` floor drops. It never records the command's result, and it is off in this workspace. The generated
  handler runs async commands on the async runtime, so wrapping the `invoke_handler` closure could not enter a span
  around an async body, nor see its typed result.
- **`gui` has its own blocking hops,** through `tauri::async_runtime::spawn_blocking`. These are describing files,
  claiming an export's destination, framing a crop and revealing a file. `opai`'s `task::spawn_blocking` carries the
  caller's span onto the blocking thread (slice 2's D7). `gui`'s calls do not.
- **`opai`'s configuration directory can be resolved without `Opai`.** That is `rust_sak::fs::user_config_dir`,
  the same call `logging::log_path` makes. `Opai::config_dir` needs an initialized `Opai`, and that does not exist in
  `setup`.
- **`single_instance::raise_main_window` reports through `eprintln!`.** It runs in the application already running,
  where a subscriber is installed.

## Goals / Non-Goals

**Goals:**

- A launch that begins with Analytics off opens no connection. Turning it off mid-session sends nothing emitted after
  the call.
- One trace per window request: the command span at its root, and `opai`'s units beneath it, across every blocking
  hop.
- Every command error variant is classified in one exhaustive `match`, so a variant added later does not compile
  until someone decides who records it.
- `opai.log` changes in two ways only:
  - `run=` appears on `opai`'s records inside a request that has a run, and appears once on `gui`'s.
  - `gui`'s own failures are recorded once, at `warn`, by the wrapper. That includes the three that were silent.

**Non-Goals:**

- Resuming telemetry in the session Analytics was turned on. `o11y` initialises once per process. The series decided
  that opting in takes effect at the next launch.
- Changing the Analytics copy, or scrubbing what is sent. The owner kept both (see Risks).
- A release workflow, or mapping CI secrets (see proposal.md).
- Frontend records, error boundaries and Faro (slices 6 and 7).
- Spans for the `opai://` scheme handler. It is not a command, and its image IO already has `opai`'s unit spans.

## Slices

Full observability, in order. Only the current slice is proposed; the rest are proposed when they are reached.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-observability-1-o11y-bridge` | `rust-sak` `o11y`: runtime log emit, owned spans with explicit parents, span context + `traceparent`, links, status, and the `tracing` bridge behind `o11y-tracing`; released as `26.9.9`; pins bumped in `opai`, `gui` and `perf` | - | done |
| 2 | `add-observability-2-telemetry-seam` | `rust-sak`: the `set_attribute` fix released as `26.9.10`. `opai`: the subscriber becomes a registry with a file layer and a Grafana layer (each with its own filter); `opai::telemetry` start/stop; export failures recorded file-only; span context carried through `task::spawn_blocking`; endpoint and auth baked in via `option_env!`; `logging` spec updated. `gui`: start in `setup`, stop on exit | 1 | done |
| 3 | `add-observability-3-record-cleanup` | `opai`: the existing records fixed. Silent failures recorded (initialize, session build, image IO, analysis refusal); each failure recorded once (step folded into run, refusal outside the bracket, unread signal folded into the conclusion); cancellations and shutdowns recorded as stops; durations and subjects added to closing records; `error` reserved for panics. `gui`: its repeats of `opai`'s failures removed | 2 | done |
| 4 | `add-observability-4-spans-metrics` | `rust-sak`: a public read-back on metric instruments, released. `opai`: spans on every unit of work, sent to the collector only; waiters linked to the build they joined; outcome and failed status on every span; the metric instruments | 3 | done |
| 5 | `add-observability-5-gui-switch` | `gui`: the Analytics choice mirrored into a Rust-owned file, read in `setup` before telemetry starts; opting out stops sending at once; one wrapper giving every command a span with its `run` and outcome, and recording once the failures only `gui` observes; `gui`'s blocking hops carry the span; the session's end recorded; the `eprintln!`s in `single_instance.rs` made records | 4 | **current** |
| 6 | `add-observability-6-frontend-log` | frontend: a log command into `opai.log`; an ErrorBoundary and global `error`/`unhandledrejection` handlers; the errors swallowed today reported | 5 | pending |
| 7 | `add-observability-7-faro` | frontend: Grafana Faro for errors and console, usage events and traces, with a `traceparent` passed into the Tauri commands so frontend and backend spans form one trace; CSP opened for the collector; paused and resumed on the Analytics switch | 5, 6 | pending |

Slice 5's row changed from the one planned in slices 1 to 4 in three ways:
- **It lost "CI secrets wired".** This repository has no release workflow. The owner chose to leave the mapping to
  the change that adds one (proposal.md).
- **"Logs every command's error" became "records once the failures only `gui` observes".** Slice 3's D1 forbids
  recording `opai`'s failures a second time. Slices 3 and 4 both said so.
- **It gained the span carry on `gui`'s own blocking hops.** Without it, describing files would root its image IO
  spans outside the command's trace.

### Decisions already taken for slices 2 to 7

Settled with the repository owner during exploration, and repeated in every slice's design.md. One was revised in
slice 2 and is marked as such.

- **`tracing` stays the one logging API.** Grafana is a second layer beside the file, fed by slice 1's bridge. There is
  no second set of `o11y::log` calls.
- **Analytics defaults to on and sends from the first launch.** Opting out stops sending at once (`o11y::shutdown()`
  and `faro.pause()`). Opting in takes effect at the next launch for the backend; Faro may resume at once.
- ~~**Path fields never reach Grafana.**~~ **Revised in slice 2:** paths are not scrubbed. Records reach Grafana with
  their paths as `opai.log` has them. This slice's command spans and records carry paths on the same terms.
- **ONNX Runtime warnings and above go to Grafana too**, with the same floor as the file.
- **Only the GUI exports.** `cli` and `perf` keep writing the file only.
- **Endpoints and credentials are baked into release builds from CI secrets.** A build without them sends nothing.
  Release builds report `production`, anything else `development`. (This slice does not wire the secrets. See the
  row note above.)
- **Faro covers errors and console, usage events, and traces linked to Rust.** Web vitals are out. Frontend errors also
  reach `opai.log` through slice 6's log command, because Faro only sends to Grafana.
- **Divergence from the Go application, deliberate:**
  - It exported to Grafana whatever the Analytics switch said; this honours the switch (from this slice).
  - It exported error logs only; this adds metrics and traces.
  - It used Aptabase for frontend events; Faro replaces it.
- **`imaging` stays silent.** Its callers in `opai` instrument per pass or per run, never per tile.

## Decisions

### D1 - The choice is mirrored into `<config>/opai/analytics.json`, and the window's value wins

`gui::analytics` owns one file, `analytics.json` in `opai`'s configuration directory, holding `{"enabled": <bool>}`.

- **Read in `setup`**, before `telemetry::start`:
  - **The file is absent** (a first launch): the choice is on, the recorded default.
  - **The file says `false`:** `start` is never called. A `debug` record in the file says why.
  - **The file is present but unreadable or malformed:** the choice is off, with a `warn` in the file. This is the
    one departure from "default on". A file exists only because a choice was mirrored, and a torn opt-out must not
    read as an opt-in. The boot mirror (D2) corrects it within the session, in either direction.
- **Written through a temporary file in the same directory, then renamed over the old one.** That uses
  `rust_sak::fs::mk_temp_file_in`, then `into_inner().persist(target)` on its handle. A crash mid-write therefore leaves the old choice or the
  new one, never a torn file. A write that changes nothing is skipped.
- **The window's store is the source of truth; the file is its mirror.** The file exists only because Rust must decide
  before any window exists. What the user sees in Settings is what they chose, so the file follows it, not the other
  way round.

*Alternative:* move the choice's storage into Rust, and have the window read it through a command. Rejected. The
store rehydrates synchronously at import, and every other preference lives there. Splitting one out would give the
settings dialog's draft, Save, Cancel and Reset two backends.

*Alternative:* delay `start` until the window reports the choice. Rejected. Everything before the window mounts, such
as the first launch's install, would never be sent. The recorded decision is "sends from the first launch".

*Alternative:* Tauri's `app_config_dir()`. Rejected. That resolves to the bundle identifier's directory
(`io.vinicius.opai`), not `opai`'s, and would put this one file apart from the logs, models and cache it sits beside.

### D2 - `set_analytics(enabled)`: stop first, then write, then record

```rust
#[tauri::command] fn set_analytics(enabled: bool) -> Result<(), AnalyticsError>
```

- **Off:**
  1. `opai::telemetry::stop()`, first.
  2. Write the file.
  3. Record `info` "analytics was turned off; nothing more is sent this session".

  Because the stop comes first, nothing after it, including that record, can reach the collector.
- **On:**
  1. Write the file.
  2. If telemetry is not running because of an earlier opt-out, record `info` "analytics was turned on; it takes
     effect at the next launch".
- **A failed write is recorded at `warn` and returned.** The in-session stop has already happened, so the opt-out
  holds for this session whatever the disk says.
- **Exempt from the command wrapper (D3).** A span for this command would be one more thing about the opt-out that
  could leave the process. The `gui-settings` requirement says turning it off is reported nowhere.

**The window calls it in two places**, through a new `ipc/analytics.ts` binding and a small mirror module that
`main.tsx` starts:
- **Once at boot**, with the rehydrated store's value. This is what corrects a missing or disagreeing file, and it is
  why a user who opts out on another path still stops at once.
- **On every store change to `analytics`,** through `useSettingsStore.subscribe`. Since `apply` (Save) is the only
  writer of that field, this means on every Save that changes it.

A rejected call is dropped with a console message. Reporting frontend failures is slice 6's job.

*Alternative:* call it from `apply` inside the store. Rejected, because it would put IPC in a store that is otherwise
pure. A subscription sees every writer, including any added later.

### D3 - One wrapper, `gui::command`, called from every command's body

```rust
pub(crate) const TARGET: &str = "gui::command";

// Opens `info_span!(target: TARGET, <command name>, outcome = Empty, error = Empty [, run = %run])`.
macro_rules! command_span { ... }

pub(crate) async fn traced<T: Outcome>(span: Span, body: impl Future<Output = T>) -> T;
pub(crate) fn traced_sync<T: Outcome>(span: Span, body: impl FnOnce() -> T) -> T;
```

Every command in `generate_handler!` except `set_analytics` is written as `traced(command_span!("enhance", run =
&run), async { ...today's body... }).await` or its synchronous twin.

- **The span's name is the command's.** It is never a field, because the file does not write span names. Its only
  field that reaches the file is `run`, and only for commands whose arguments include one: `enhance`,
  `cancel_enhance`, `suggest`, `cancel_suggest`, `detect_faces`, `export` and `cancel_export`.
- **`outcome` and `error` start `Empty`,** so the file's formatted fields for the span hold only `run` for as long as
  the body runs. They are recorded after the body, once no record inside can pick them up.
- **The body is `.instrument(span)`-ed.** So every `opai` call inside it, and every unit span `opai` opens,
  parents under the command span. Slice 4's unit spans take `Span::current()` as their parent, so this needs no
  change in `opai`.
- **After the body, `Outcome` decides one of four endings:**
  - `Finished`: records `outcome = "finished"`.
  - `Stopped`: records `outcome = "stopped"`, and no `error`, so the span is not marked failed.
  - `Failed { recorded: true }`: records `error` and `outcome = "failed"`. No record is written, because `opai`
    already wrote one inside this span.
  - `Failed { recorded: false }`: first `warn!(target: TARGET, command = <name>, %error, "a request from the window
    failed")`, emitted inside the span so that it carries `run=` and the trace. Then `error` and `outcome` are
    recorded on the span.

*Alternative:* wrap the `invoke_handler` closure once for all commands. Rejected: see Context. It sees neither the
async body nor the typed result.

*Alternative:* `#[tracing::instrument]` on each command. Rejected, because it cannot record the outcome. It would also
record every argument as a field unless each one were `skip`ped, and the file would then write them onto `opai`'s
records.

*Alternative:* a proc-macro attribute of this crate's own. Rejected: a second crate to spare 23 one-line wraps.

### D4 - `Outcome`: one exhaustive `match` per error enum

```rust
pub(crate) enum Ended<'a> { Finished, Stopped, Failed { error: &'a dyn Display, recorded: bool } }
pub(crate) trait Outcome { fn ended(&self) -> Ended<'_>; }
```

- **Implemented for `Result<T, E>`** where `T: Answer` and `E: CommandError`, each a one-method trait with its own
  exhaustive `match`:
  - `Answer` defaults to `Finished`. `Enhancement` and `Exported` override it, so their `Stopped` variants read as
    stopped.
  - `CommandError` has no default, which makes each enum decide every variant.
- **Implemented directly for the infallible answers:** `()`, `bool`, `Option<String>`, `&'static str`,
  `&'static [FamilyEntry]` and `Vec<&'static str>`.

The classification:

| Error | Stopped | Failed, recorded by `opai` | Failed, recorded by the wrapper |
|---|---|---|---|
| `SetupError` | - | `Initialize` | - |
| `EnhanceError` | - | `UnreadableSource`, `Enhance` | `NotReady`, `UnknownSource`, `UnknownOperation` |
| `SuggestError` | `Stopped` | `UnreadableSource`, `Analyse` | `NotReady`, `UnknownSource` |
| `DetectError` | - | `UnreadableSource`, `Detect` | `NotReady`, `UnknownSource` |
| `ExportError` | - | `UnreadableSource`, `Enhance`, `Write` | `NotReady`, `UnknownSource`, `UnknownOperation`, `Unwritable` (new, below) |
| `ImagesError` | `OpenImages`, `DescribeImages` (the runtime shut down) | - | `RevealImage` |
| `RevealExportError` | - | - | `RevealExport` |
| `LinksError` | - | - | `OpenLink` |
| `LogsError` | - | - | `LogPath`, `RevealLog` |

- **`ExportError::Write` is split.** Today one variant serves `opai`'s failed save and `gui`'s failed claim or commit
  alike, so no `match` can tell them apart. The claim and commit arms move to a new `Unwritable { path, message }`,
  serialized with `#[serde(rename = "write")]`. Its wire shape is therefore unchanged, and the window's formatter
  needs no edit. The save arm keeps `Write`.
- **Records that the wrapper now writes are removed from the bodies:**
  - `export/run.rs`'s `unwritable` closure's `error!`
  - `export/written.rs`'s "a reveal named a file no export of this session wrote"
  - `images/files.rs`'s "a reveal named a file this application never opened"

  Each failure's error `Display` already names the subject. The bodies go on returning the same error.
- **Explicit `run` fields on records inside command bodies are removed,** because the span supplies `run` and the
  file would otherwise write it twice. That includes the `cancel_*` commands' "a stop named …" records, since their
  span carries the run they name.

### D5 - `gui`'s blocking hops carry the command span

A crate-private `gui::task::spawn_blocking` does what `opai::task::spawn_blocking` does:

```rust
let span = Span::current();
tauri::async_runtime::spawn_blocking(move || span.in_scope(work))
```

Every non-test `tauri::async_runtime::spawn_blocking` in `gui` goes through it. Where no span is current, as in the
scheme handler, `Span::current()` is `Span::none()` and `in_scope` is a no-op.

*Alternative:* reach `opai::task::spawn_blocking`. Rejected, because it is `opai`'s crate-private plumbing over
`tokio`. `gui` runs on Tauri's runtime handle, and exporting it would add `opai` API for one caller.

### D6 - The session's end, and the raise failures

- **The session's end.** `run` stores an `Instant` when it begins. `RunEvent::Exit` records `info!(target: "gui",
  uptime = ?elapsed, "Open Photo AI is closing")` and then calls `telemetry::stop()`. This is the order that lets the
  record reach the collector in the same flush.
- **The raise failures.** `single_instance::raise_main_window`'s three `eprintln!`s become `warn!(%error, …)` with the
  same sentences. They run in the application already running, whose subscriber is installed, so the records land in
  its file.
- **`lib.rs`'s `eprintln!` for a log sink that could not be installed stays**, because no subscriber exists to
  receive a record then. `export/run.rs`'s `eprintln!` is in a test.

### Division across crates

- **`gui` (Rust):** `analytics` (D1, D2), `command` (D3, D4), `task` (D5), and D6. All of these decide *when* and
  *whether* to call `opai::telemetry`, or wrap Tauri commands. Neither has a second caller.
- **`gui` (frontend):** the `setAnalytics` binding and the mirror (D2).
- **`opai`, `cli` and `perf`:** nothing. `opai`'s filters already pass `gui::command` to both layers, and its unit
  spans already take the current span as parent.

### `rust-sak` and crates

- **`fs`:** `user_config_dir` to resolve `opai`'s directory, and `mk_temp_file_in` for the temporary file. The
  rename is `tempfile`'s `persist`, reached through the handle's `into_inner`, so `gui` names no new crate. `gui` adds `fs` to its pin's feature list, and it is already compiled for `opai`.
- **Nothing else is used or missing.** `o11y` is reached only through `opai::telemetry`.
- **No crate is introduced.** `serde_json` reads and writes the file. It moves from `gui`'s dev-dependencies to its
  dependencies, and is already in the graph through `tauri`, so nothing new is compiled.

## Risks / Trade-offs

- **[Accepted risk] The Analytics copy says "No images or personal data are ever collected", but paths are sent.**
  Slice 2's D8 sends file paths, and paths carry user names and photo file names. Slice 2 required the gap to be
  closed before this slice's release. The repository owner chose to keep both the copy and the paths as they are.
  → Recorded here. No released build sends anything until a release workflow maps the secrets, so the gap goes live
  with that change. Revisiting it then means rewording the copy in 13 locales, or scrubbing, which needs `rust-sak`'s
  bridge to pass `message` through `map_field`.
- **[Risk] A first launch sends before the window mirrors the choice.** When the file is absent but the store holds
  `false`, telemetry starts in `setup` and sends until the boot mirror calls `stop`. That happens when the file was
  deleted, or the store predates this slice. → That window is the time to mount React: a few hundred milliseconds of
  startup records. No released build predates this slice, so the only store without a file is a developer's.
- **[Trade-off] Clearing the window's storage re-opts-in at the next launch.** Clearing `localStorage` resets the store
  to its default of on, and the boot mirror writes that to the file. → The user sees Analytics on in Settings, which is
  what the application then does. The store is the source of truth (D1).
- **[Trade-off] Opting in does not resume sending until the next launch.** The settings row does not say so.
  → The series' recorded decision. The copy is unchanged at the owner's direction. The file records it at `info`.
- **[Risk] A command added later is not wrapped.** Nothing stops a new `#[tauri::command]` from skipping `traced`.
  → `lib.rs` gains a drift test: the command names in `generate_handler!` must equal the names given to
  `command_span!` across the crate's sources, plus `set_analytics`. It reads the sources from `CARGO_MANIFEST_DIR`
  at test time, much as the existing `tauri.conf.json` guards read that file.
- **[Risk] `outcome`/`error` leak into a file line.** `fmt::layer`'s `on_record` appends recorded fields to the span's
  formatted fields. → They are recorded only after the body and after the wrapper's own `warn`, so no record inside the
  span is emitted after them. A unit test asserts that the wrapper's `warn` line carries `run` and no `outcome`.
- **[Trade-off] More spans per session.** Every command is a span, including `catalogue` and `window_ready`. → They
  are bounded by the window's requests. A slider drag already sends one `enhance` and one `cancel_enhance` per step,
  and `opai`'s spans already cost more per request.

## Migration Plan

1. `gui::command` and `gui::task`, tested on their own with a recording layer (the `registry` feature, already a
   dev-dependency). Then the classification, the drift test, and each command wrapped, with its body's `run` fields
   and repeated records removed.
2. `gui::analytics` and `set_analytics`, then `setup`'s read, then the frontend mirror.
3. The exit record and the raise records.
4. Validation:
   - **Unit tests** on all three CI platforms:
     - the wrapper's four endings
     - span parentage across `gui`'s blocking hop
     - the file's read and write rules
     - the ordering in `set_analytics` (no record while telemetry runs after an opt-out)
     - the frontend mirror
   - **A manual check on macOS, handed to the owner** (the rule against driving the GUI). It uses a `gui` debug build
     with the collector variables set:
     - Enhance once and confirm in Tempo that the trace is rooted at `enhance`, with `opai`'s units beneath it.
     - Confirm in `opai.log` that `opai`'s records inside carry `run=` once.
     - Turn Analytics off and confirm in Loki that nothing arrives after it.
     - Relaunch and confirm nothing arrives from that session.
     - Turn it back on, relaunch, and confirm the session's end record arrives on quit.
   - **Windows and Linux** are covered by CI only. No execution provider is involved.

**Rollback:** revert this repository's commits. An `analytics.json` left behind is ignored by earlier builds.
