## Why

Nothing in `crates/` logs. Thirty-four `println!`/`eprintln!` calls exist and all but two are inside
`#[cfg(test)]` modules or the live-check helpers; the two that ship — `libpath.rs`'s failed re-exec
warning and `gui`'s D-Bus skip — write to a stderr that a double-clicked `.app` bundle does not have.
When a user reports that an enhancement was slow, or ran on the CPU on a machine with a CUDA card, or
took the process down with it, there is currently no record of it anywhere.

The records that matter most are not this project's own. ONNX Runtime writes its diagnostics from
C++, and they are the ones that explain the two failures the reference implementation actually had
reported against it: an execution provider that declined to attach, and a native crash inside a CUDA
session — `vegidio/open-photo-ai` issue #40, where *"both logs the reporter attached simply stop
mid-session, because the C++ account of the failure was sitting unread in the pipe at the moment of
death."*

Go solved this and paid heavily for it. `shared.SetupLogging` is 123 lines, and behind it sit
`stderr_capture.go` (380 lines, plus a Windows variant), `ort_log_parse.go` (100 lines of regex for
ORT's text format, including a branch for the prefix macOS adds once stderr is not a terminal), a
second `native.log` file, and a `foldNativeLog` that replays the previous run's unread bytes on the
next launch. All of it exists because the Go binding creates its environment with plain `CreateEnv`,
so *"there is no callback to install and no sink to swap"*, and the file descriptor is the only
interception point there is.

`ort` has the callback. Its `tracing` feature makes `CreateEnvWithCustomLogger` the default path and
forwards every C++ record into `tracing` with its severity, code location and logger id intact. The
stderr capture, the tailer, the parser, the second file and the fold-on-next-run all stop being
necessary at once — and the records arrive on the thread that produced them, so they are written and
on disk before the call that emitted them returns, which is the exact property the Go design went to
a file rather than a pipe to get.

Slice 1's claim is what makes one shared file safe: *"Daily rotation is a rename under a live writer,
and a second process holding a descriptor keeps writing into the renamed inode."* That is now
bounded to processes that have not yet been refused.

## What Changes

Task estimate: **14**. This is logging slice 1 of 2, and change 3 of the seven the plan design.md
records.

**In `opai`, one sink and the vocabulary to reach it:**

- **`tracing` is the library's logging surface.** No `SetLogger` and no logger handle: library code
  emits events, and a process with no subscriber installed drops them at a branch. That is the
  idiomatic Rust equivalent of Go's `slog.DiscardHandler` default — *"any third-party application
  importing the OPAI library produces no log output and creates no files unless it explicitly opts
  in"* — reached without an API to maintain.
- **`opai::logging::init(name, app)`** — the file sink, in `opai` because all three binaries need it
  and the rule this project sets is that such code lives there. It resolves `<config dir>/logs`,
  opens `opai.log` in append mode, writes the `---` session divider, installs the subscriber and
  emits the session header. It may be called once per process; a second call is an error rather than
  a no-op, exactly as Go's latch decided, because a caller that reached it believes it owns the
  logging setup.
- **`opai.log` is the file, and it keeps that name.** Rotated daily, gzip-compressed, seven files of
  history — timberjack's settings, preserved. Go needed a startup stale-check because *"timberjack's
  RotateAt only fires while the process is alive across midnight"*; the Rust rotator decides from the
  live file's modification time on the first write, so an app opened for the first time today rotates
  yesterday's file with no special case.
- **Every record carries `app=`**, the `App` slice 1 defined for exactly this: *"the logging change
  ahead of this one puts it on every log record so that two sessions interleaved in one file can be
  told apart."* This is a widening over Go, whose records are distinguishable only by the divider.
- **ONNX Runtime's C++ diagnostics land in the same file**, through `ort`'s `tracing` feature rather
  than through a captured file descriptor. Its severity, code location and logger id survive as
  fields, which is the information Go's regex was reconstructing.
- **`opai::APP_NAME`** — `"io.vinicius.opai"`, the name the front ends pass. It is the first time this
  project names itself: the configuration directory has always been a parameter, and nothing had to
  supply one until a binary needed somewhere to put a log file. The value is the bundle identifier
  `crates/gui/tauri.conf.json` already carries and every single-instance backend already keys on, so
  one reverse-DNS name answers the configuration directory, the bundle identity and the activation
  key together. Deliberately **not** the Go app's `open-photo-ai`, so the two installations share no
  runtime, no models, no cache and no lock — which is what `add-scaffold-2-gui` D3 already assumed
  when it kept the shared bundle identifier (*"neither app can read or corrupt the other's data"*).
  The log *file* does not follow the name: it is `opai.log` under any application name, because that
  is what a user is asked to attach.
- **A panic hook** that routes a Rust panic's message and location into the log before the default
  handler runs, which is the one thing Go's stderr capture gave that the ORT callback does not.

**In `gui` and `cli`:** one call each, at the top of the entry point, plus the GUI's existing D-Bus
`eprintln!` becoming a `tracing::warn!` — the record Go writes at the same point. `gui` installs the
sink **before** the single-instance plugin is registered, which is compatible with slice 2's ordering
rule: that rule binds `Opai::initialize`, not the sink.

**Instrumentation is bounded to the initialization path in this slice.** The Go library logs from 84
call sites across inference, sessions, the installer, the cache and the model registry. Porting them
is its own slice — see below — and this one carries only what proves the pipe: what `Opai::initialize`
does, and what the front ends say before and after it.

## Capabilities

### New Capabilities

- `logging`: where an application's records go and what a reader finds there. The file and its
  location, the session divider and header, the fields every record carries, daily rotation and seven
  days of retention, which sources are captured — this library, the front ends, and the inference
  runtime's own C++ diagnostics — and what happens when the sink cannot be installed at all.

  Distinct from `initialization`, which says what is put on disk before a model can run, and from
  `single-instance`, which says how many processes may be doing it. Logging is a property of the
  process rather than of the installation: a front end that never initializes still has a log file,
  which is precisely the case `gui` is in today.

### Modified Capabilities

- `initialization`: one requirement changes, and only in what it promises. *"Runtime diagnostics are
  reported at warning level"* currently ends its scenario at **THEN** it is reported — true, but
  silent about where, because until now the answer was a stderr no bundled application has. The
  level, the telemetry switch and the environment name are unchanged; what is added is that those
  diagnostics reach the application's log when a sink is installed, and are dropped rather than
  printed when one is not.

## Impact

**Crates:** `opai` for all of it — the sink, the rotation, the format and the ORT bridge. `gui` and
`cli` contribute one call each and nothing else; neither holds, flushes or reasons about the sink,
and neither knows where the file is. No frontend change: the Go GUI exposes the log path on its
settings screen, and this project has no settings screen yet, so that arrives with it.

**`rust-sak`:** nothing covers this, and one module comes close enough to be worth ruling out
explicitly. **`o11y` is not this.** It ships *"structured OTLP log records to an OpenTelemetry
collector"* over the network, opt-in, for telemetry; it has no local sink, no file, no rotation and
no `tracing` integration, and its `Telemetry` handle is a thing this application will *additionally*
want later — the Go app runs both side by side. `fs` supplies `mk_user_config_dir`, which
`config::app_dir` already goes through. There is no log file, no rotation and no subscriber
anywhere in `crypto`, `fetch`, `memo` or `sysinfo`.

A rotating file sink **is** general-purpose and would be a candidate for `rust-sak` by this project's
own rule. It is deliberately staying local, on the same terms as slice 1's D9: the series is not
being sequenced behind a `rust-sak` release. What is local is also less liftable than the lock was —
the `app=` field, the divider and the session header are this application's decisions, not a generic
appender's — so design.md records the seam rather than the debt.

**Dependencies:** three added to `opai`, and one feature turned back on.

- `tracing` — the facade. Already in `Cargo.lock` (via `reqwest`/`h2`), so naming it directly
  compiles nothing new.
- `tracing-subscriber` — the registry, the filter and the field visitor. New.
- `file-rotate` — daily rotation, gzip and retention against a fixed live filename, which is what
  keeps the file called `opai.log`. `tracing-appender` was the obvious candidate and cannot do it:
  it *"will automatically append the current date and hour to the file name"*, so there is no
  `opai.log` to tell a user to attach. design.md's D4 has the comparison.
- `ort`'s `tracing` feature, which this project's manifest explicitly turned off with the defaults —
  *"`ort`'s `tracing` feature stays off until there is a subscriber to receive them."* There now is.

**Behavioural divergence from the Go implementation**, three of them:

- **`app=` on every record**, which Go does not have. A widening, and the reason slice 1 made `App` a
  type rather than a string.
- **Rust panic messages reach the log; native aborts still do not.** Go's stderr capture caught both,
  because it owned descriptor 2. The hook here covers Rust panics, and a hard native abort — the
  issue #40 case — is covered differently and better: ORT's own account of the failure is written
  synchronously as it is produced, so it is on disk before the abort rather than replayed from
  `native.log` on the next run. What is genuinely lost is output from a third-party native library
  that writes to stderr and is not ORT. design.md records it as a known parity gap.
- **The configuration directory is `io.vinicius.opai`, not `open-photo-ai`.** A user of the Go app
  does not inherit its installed runtime or models and downloads them again.

**Deferred to logging slice 2 (`add-logging-2-records`):** the instrumentation sweep — the inference
path, the session registry, the installer, the image cache and the model catalogue, against the 84
records the Go library emits, including the decision about which of them are `debug` rather than
`info`. It is its own slice because it cannot share one with the sink under this project's 15-task
rule, and it is *after* the sink because there is nowhere to put a record until the sink exists.
