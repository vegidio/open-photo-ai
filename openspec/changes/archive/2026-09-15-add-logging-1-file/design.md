## Slices

Seven changes, in order. Single-instance came first because the changes after it are designed against
its guarantee — the logging change puts three binaries in one file and needs to know whether a second
writer is possible, and the benchmark needs to know nothing else is competing for the GPU, the cache
or the model directory while it is measuring. **This change is logging slice 1, change 3 of the
seven.**

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-single-instance-1-lock` | At most one OPAI process per user: an advisory lock taken in `Opai::initialize`, the holder recorded, the second process refused | none | **done** (archived `2026-09-14`) |
| 2 | `add-single-instance-2-gui` | The GUI raises its existing window instead of refusing, the ending a double-clicked icon deserves | 1, and a GUI with a window | **done** (archived `2026-09-14`) |
| 3 | `add-logging-1-file` | The sink: `tracing` through `opai`, ORT's C++ diagnostics via `ort`'s `tracing` feature, `<config>/logs/opai.log`, daily rotation, the `---` session divider, the session header, the per-record `app=` field, `APP_NAME`, and the initialization path instrumented | 1 | **current** |
| 4 | `add-logging-2-records` | The instrumentation sweep: inference, the session registry, the installer, the image cache and the model catalogue, against the 84 records the Go library emits | 3 | pending |
| 5 | `add-perf-1-opai` | What a benchmark needs from the library: `release_sessions` made public, the `(requested, actual)` execution provider surfaced out of `process`, skip-verify | 3 | pending |
| 6 | `add-perf-2-harness` | `crates/perf` → the `perftest` binary: `clap`, catalogue-driven model selection, the warm-up/cold-start/timed-runs loop, statistics, plain and JSON output | 5 | pending |
| 7 | `add-perf-3-tui` | The `ratatui` inline viewport: spinner, per-tile bar, ETA, finished models into scrollback via `insert_before`, TTY detection | 6 | pending |

**Row 4 is new to this table**, and it is a split rather than an addition: the archived tables carried
one logging row, and the work behind it does not fit one change. The Go library logs from 84 call
sites; choosing a level, a message and a field set for each of them, against the Rust code that
replaced it, is a slice's worth of work on its own and cannot share fifteen tasks with building the
sink it writes into. The name this series was given — `add-logging-**1**-file` — already said the file
was one slice of something, and this is the rest of it. Slice 5 depended on "3" when 3 was the whole
of logging, and it still does: `perftest` wants a sink to see the library through, not the sweep.

## Context

See proposal.md — Why for the motivation, specs/logging/spec.md for the behaviour, and
specs/initialization/spec.md for the one requirement that changes.

Six facts about the code, read rather than assumed, shape this design:

- **Nothing in this project logs, and nothing ever has.** There are 34 `println!`/`eprintln!` calls
  in `crates/`, and all but two are inside `#[cfg(test)]` modules or the live-check helpers. This is
  a green field: there is no `log` facade to migrate off, no existing sink to reconcile with, and no
  format anyone depends on.
- **`ort`'s `tracing` feature installs a C++ logging callback**, not just Rust-side spans.
  `EnvironmentBuilder::create_environment` does `let logger = logger.or(Some((tracing_logger,
  null_mut())))` under `#[cfg(feature = "tracing")]` and then calls `CreateEnvWithCustomLogger`, and
  `tracing_logger` turns each record into a `tracing` event carrying the runtime's severity, its
  `id` and its `code_location`. Switching the feature on replaces Go's entire stderr-capture stack
  with a manifest line.
- **The manifest turned that feature off on purpose, and said when to turn it back on.**
  `crates/opai/Cargo.toml` names `tracing` among what `default-features = false` drops, and
  `runtime.rs:113-117` records the condition: *"Where those records go is the logging change's to
  decide; `ort`'s `tracing` feature stays off until there is a subscriber to receive them."*
- **`runtime.rs` already clamps the runtime's own level** to `LogLevel::Warning`, the same level Go
  sets, and `openspec/specs/initialization/spec.md` already requires it. That decision is not
  reopened here; only its destination is.
- **`tracing` 0.1.44 and `tracing-core` are already in `Cargo.lock`**, pulled in through
  `reqwest`/`h2`. Naming `tracing` directly in `opai`'s manifest compiles nothing new.
- **No front end has ever named this application.** `Opai::initialize(name, app, ..)` takes the name,
  `config::app_dir` resolves the configuration directory from it, and every caller so far has been a
  test passing a temporary directory's name. `gui` never calls `initialize` at all. A log file is the
  first thing a shipping binary needs a real name for.

## Goals / Non-Goals

**Goals:**

- One file, `opai.log`, that a user can be asked to attach without being asked which of several files
  to look in.
- The inference runtime's own diagnostics in that file, with what identifies them intact, and without
  a single line of descriptor plumbing.
- A record is on disk before the call that emitted it returns. The records worth having are the last
  ones before a failure.
- The library stays silent for anyone who has not asked for a log, with no API to say so.
- The sink is a property of the process, so the GUI — which does not initialize yet — still has a log.
- Where the file is, stated once, so the settings screen that will show it cannot disagree with the
  code that writes it.

**Non-Goals:**

- The instrumentation sweep. Slice 4, and the bulk of the parity work.
- Telemetry. `rust-sak`'s `o11y` and the OTLP exporter the Go app runs alongside its file log are a
  separate decision with a separate privacy story, and nothing here forecloses it.
- A console sink. `perftest`'s `--verbose` stderr logger is slice 6's, and today's `cli` has one line
  of output that is not a log record.
- Showing the log to the user. The Go GUI has a settings screen with a "show logs" action; this
  project has no settings screen. `logging::log_path` exists so that screen has one place to ask.
- Structured output for machines — JSON lines, OTLP, a schema. The file is read by people.
- Capturing arbitrary native output. What ORT writes is captured through ORT's callback, not by
  owning file descriptor 2; see D9 for what that gives up.

## Decisions

### D1. `tracing`, and the library simply emits

Library code calls `tracing::info!`/`warn!`/`debug!` directly. There is no `SetLogger`, no logger
handle threaded through `Inner`, and no accessor like Go's `internal.Log()`.

Go needed all three because `slog` has no ambient sink that is off by default: a package-level
`slog.Info` writes to the process's default handler, which for a library means writing to a consumer's
stderr uninvited. `internal.Log()` exists to hold a `DiscardHandler` so that *"any third-party
application importing the OPAI library produces no log output and creates no files unless it
explicitly opts in"*.

`tracing` is that property by construction. With no subscriber installed, an event is a load of a
global, a `None`, and a return; the arguments are never formatted. The spec's *"a library consumer
that does not ask for logging gets none"* is therefore satisfied by the facade rather than by an API
this project maintains — and the opt-in that Go spells `opai.SetLogger` is spelled, here, as
installing any subscriber at all, including one this crate had nothing to do with.

**Alternatives considered.** The `log` crate: a smaller facade, but ORT's records arrive as `tracing`
events with span-carried fields whatever this project picks, so `log` would mean a bridge and the loss
of those fields. A logger object passed through `Opai` and into every module: the Go shape, ported
rather than designed, and it cannot reach the ORT callback at all — that callback is a bare `extern
"system" fn` with no room for a handle.

### D2. The sink is `opai::logging::init(name, app)`, called by each binary at the top of its entry point

In `opai`, because all three binaries need it and this project's rule is that such code lives there —
the same reasoning that put the single-instance lock in the library rather than in three `main`
functions. In each binary's entry point rather than inside `Opai::initialize`, because `gui` does not
call `initialize` and would otherwise have no log at all, which is the opposite of what a windowed
application needs.

It does six things in this order: resolves and creates `<config dir>/logs`, opens `opai.log` for
append through the rotator, writes the `---` divider, installs the subscriber, installs the panic
hook, and emits the session header. It returns the path it opened, so a caller that wants to show the
file does not rebuild it — Go's `GetLogsPath` carries the comment *"it mirrors how shared.SetupLogging
builds the path so the two stay in sync"*, which is a duplication to inherit as a warning rather than
as a design.

**Once per process**, latched on an `AtomicBool` compare-exchange, and a second call is an error
rather than a no-op — Go's reasoning, unchanged and still correct: *"a caller that reached here
believes it owns the logging setup and silently handing it a working-looking Closer would let it tear
down the first call's capture."* The sink installed first keeps working; the second caller is told.

**Nothing to close.** Go returns an `io.Closer` to flush the rotation worker and put stderr back. This
design has no worker (D7) and never touches stderr (D3), so `init` returns a `Result<PathBuf,
LogError>` and there is no guard for a front end to hold, drop at the wrong time, or forget.

### D3. ORT's diagnostics come through `ort`'s callback, and the Go capture stack is not ported

Enable `ort`'s `tracing` feature. That is the whole mechanism.

What it deletes, concretely: `shared/stderr_capture.go` (380 lines) and its Windows variant, the
`native.log` file, `foldNativeLog` and the replay of an unread tail on the next run,
`shared/ort_log_parse.go` (100 lines of regex over ORT's text format, including `applePrefix` — the
*"second timestamp plus the process name, pid and thread id"* that macOS prepends once stderr is not
a terminal), the 150 ms poll, the 4 MiB pending-line bound, and the UTF-16 handling Windows needed.

All of it exists to work around one absence. The Go binding *"creates the environment with plain
`CreateEnv` rather than `CreateEnvWithCustomLogger`, so there is no callback to install and no sink to
swap"*, and `ort` calls `CreateEnvWithCustomLogger`. The fields Go's regex reconstructs — severity,
`ort_file`, `ort_line`, `ort_func`, `ort_logger` — arrive as the `location` and `id` fields of the
`ort` span the callback opens, never having been text.

Three consequences worth stating, because they are behaviour and not implementation:

- **ORT stops writing to the terminal entirely**, with or without a subscriber. The feature replaces
  the stdio sink rather than tee-ing it, which is why specs/initialization/spec.md gains a scenario
  for a diagnostic with no sink installed: it is discarded, not printed.
- **The records are written on the thread that produced them**, synchronously, so they are on disk
  before the C++ call that emitted them returns. That is the property Go went to a file rather than a
  pipe for — *"bytes written to a file are already the kernel's before `write(2)` returns"* — and it
  is reached here without the file, the tailer or the replay.
- **`fatal` is not flattened.** ORT's fatal records arrive with their own prefix rather than as an
  ordinary error, which is what Go's `ort_fatal` attribute was compensating for in a `slog` that has
  no level above error.

The environment's level stays where `initialize` already puts it (`LogLevel::Warning`), and
`runtime.rs`'s comment about a subscriber not existing yet is rewritten rather than left to rot. Note
that `ort` creates the environment at `ORT_LOGGING_LEVEL_VERBOSE` and our `set_log_level` clamps it
immediately afterwards; the window between the two is inside one function and is covered by the
subscriber's own `ort=warn` floor (D6).

### D4. `file-rotate` for rotation, because the live file has to keep its name

`FileRotate::new(path, AppendTimestamp::with_format("%Y-%m-%d", FileLimit::MaxFiles(7),
DateFrom::DateYesterday), ContentLimit::Time(TimeFrequency::Daily), Compression::OnRotate(0),
Some(append_options))` — timberjack's settings, spelled in another library: daily, seven files, gzip,
and the live records in `opai.log`.

**The suffix is `DateYesterday` rather than the crate's `AppendTimestamp::default`, and that is a
correction to an earlier draft of this decision.** `default` stamps a rotated file with the moment
rotation *fires*, and daily rotation fires on the first write **after** the boundary — so every
rotated file would be named one day later than the records inside it, against specs/logging/spec.md's
*"a compressed file named for the day they cover"*. `DateFrom::DateYesterday` is `file-rotate`'s own
answer to exactly this, documented as *"date yesterday, to represent the timestamps within the log
file"*, and it makes `opai.log.2026-09-14.gz` the file holding the 14th's records. The format drops to
`%Y-%m-%d` with it: the rotation is daily, so a time-of-day component names nothing.

It is exact for the case a reader is in when they go looking — a session that ran across midnight. It
is approximate for the other one: an application opened after several days away rotates a live file
older than yesterday and names it yesterday anyway. What the spec requires there is only that the
stale file is rotated *before* this session writes, so no two days' records share a file; naming it by
the day it actually covers would mean carrying the live file's modification time into a custom
`SuffixScheme`, which is more machinery than the case is worth.

**The live filename is the requirement that decides this**, and it eliminates the obvious candidate.
`tracing-appender` is the `tokio-rs/tracing` project's own appender and would otherwise win on
provenance alone, but it *"will automatically append the current date and hour (UTC format) to the
file name"*: there is no `opai.log`, only `opai.log.2026-09-14`, so *"attach
`~/.config/io.vinicius.opai/logs/opai.log`"* stops being a sentence anyone can write. It also has no compression,
which is the other half of timberjack's configuration.

`file-rotate` 0.8.0, MIT, ~1.1M recent downloads, `std::io::Write`, and pure Rust over `flate2` — no
C toolchain, which is the same property that made `redb` acceptable for the cache's disk tier. Its
one dependency of consequence, `flate2`, is already in this project's graph.

It also removes a workaround rather than porting it. Go needs a startup stale-check because
*"timberjack's `RotateAt` only fires while the process is alive across midnight"*; `file-rotate`'s
time mode reads the live file's modification time when it opens and rotates on the first write if the
boundary has passed, so *"the application starts on a later day"* is its ordinary path rather than a
special case. The check Go added at startup is simply not written here.

**Alternatives considered.** `tracing-rolling-file` and `rolling-file` — both fine, both smaller, and
both with the same dated-live-file shape. `firo` — recently published and thinly used; not where a
file written at every launch should be early. Writing it here — a day comparison, a rename and a
`flate2` call is genuinely not much code, but "not much code" is how every log rotator starts, and
the retention pruning and the partially-written-rotation cases are where they stop being small.

### D5. One line, slog's shape, with `app=` in it

```
time=2026-09-14T10:04:12.481+02:00 level=INFO app=gui target=opai msg="opai initialized" name=opai cache=disk
time=2026-09-14T10:04:19.902+02:00 level=WARN app=gui target=ort msg="Some nodes were not assigned..." id=opai location="session_state.cc:1166 VerifyEachNodeIsAssignedToAnEp"
```

`time`, `level`, `app`, `target`, `msg`, then the record's own fields — which is Go's
`slog.NewTextHandler` output with two insertions. A user who has read the Go app's log can read this
one, and a support instruction written for one works for the other.

**`app=` is the reason a custom event format exists at all.** `tracing` has no concept of a field
constant across a subscriber: fields belong to events and spans, and a span's fields reach an event
only on the thread that entered it — which the ORT callback's thread and every Tokio worker are not.
So the value is held by the formatter and written on every line, which is both the simplest
implementation and the only one that covers records this project did not emit. `App` is slice 1's
type, spelled by `App::as_str`, so the refusal message and the log field cannot drift — *"one type,
defined once, rather than a lock-holder string and a logging enum that can disagree."*

**`target=` is an addition over Go**, and it is what makes the ORT records self-identifying: `ort`
against `opai::…`, without the `source=onnxruntime` attribute Go had to attach by hand in the parser.
Span fields are rendered after the message, which is how the `id` and `location` the ORT callback
attaches reach the line.

Implementation shape: a `FormatEvent` impl over `tracing_subscriber::fmt`, walking
`ctx.event_scope()` for the `FormattedFields` the layer stores on each span. Not a bespoke `Layer` —
the field visitor, the span storage and the writer plumbing are what `fmt` already is, and only the
line's shape is this project's.

### D6. `info` by default, `RUST_LOG` to change it, and the runtime floored separately

`EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,ort=warn"))`.

`RUST_LOG` rather than an `OPAI_LOG` of our own: it is the ecosystem's variable, it costs nothing, and
a user being walked through a bug report can be given a command that any Rust developer would
recognise. Go's equivalent is `cmd/perf`'s `OPAI_PERF_DEBUG`, which is that binary's flag rather than
the application's, and slice 6 can keep it.

**`ort=warn` is a separate directive because the runtime's verbose level is not like ours.** ORT at
informational logs per-node assignment for every node in the graph; a user who sets `RUST_LOG=debug`
to see this project's cache decisions should not get tens of thousands of lines about a model's
internals. Two dials, because there are two sources with very different volumes — and the C++ side is
already clamped to warning independently, so this is the second of two floors rather than the only
one.

### D7. Synchronous writes, deliberately, and no background thread

The writer is the `FileRotate` behind a `Mutex`, handed to `fmt` through a small `MakeWriter` whose
`Writer` is the lock guard. Every record is a `write(2)` on the emitting thread.

`tracing_appender::non_blocking` is the reflex here and is wrong for this application. It buffers
records and hands them to a worker thread, and *"when the process dies from a signal every goroutine
stops at once, so whatever had not been read yet is gone"* — the Go comment is about goroutines, and
the sentence transfers to a Rust worker thread without changing a word. Issue #40 is that bug: *"both
logs the reporter attached simply stop mid-session."* The whole reason this project wants a log is the
last few records before a native crash, and a buffer is precisely what loses them.

The cost is a lock and a syscall per record on the emitting thread. It is affordable because of what
the volume actually is: at `info` this project writes a few dozen records per session, and ORT at
warning writes a handful. If slice 4's sweep makes a hot path chatty, the answer is the level of that
record, not a buffer between the record and the disk.

The file is opened with `append(true)`, so each write is positioned by the kernel rather than by a
cached offset. That is what keeps two processes' records interleaved by line rather than overwriting
each other during the one window where two processes can both be writing — see the risks.

### D8. `APP_NAME` is `"io.vinicius.opai"`, published by `opai`, and it is not the Go app's directory

```rust
pub const APP_NAME: &str = "io.vinicius.opai";
```

Both front ends need a name, and a literal in each is two answers to one question: if they ever
disagree, `gui` and `cli` silently use two configuration directories, two model installs and two
locks. It therefore lives in `opai` by the same rule the rest of the shared vocabulary does.
`Opai::initialize` keeps taking `name` as a parameter — the tests depend on that, and the library is
not entitled to assume it is only ever this application.

**`io.vinicius.opai`, not the Go app's `open-photo-ai`**, at the user's direction, and it is the answer
`add-scaffold-2-gui` D3 was already relying on when it kept the shared bundle identifier: *"the Go app
derives its config and cache directories from an app name passed to `opai.Initialize`, not from the
bundle identifier, so neither app can read or corrupt the other's data."* That sentence is only true
if the names differ. With separate directories, an installed Go app and this one share no runtime, no
models, no cache, no log and no single-instance lock — which also means slice 1's claim is not
weakened by a Go process that knows nothing about it.

The value is the **bundle identifier**, spelled exactly as `crates/gui/tauri.conf.json` carries it and
as every backend of the single-instance plugin already keys on it — the D-Bus well-known name on Linux,
the named mutex on Windows, the socket path on macOS. One reverse-DNS name therefore answers the
configuration directory, the bundle identity and the activation key together, rather than the directory
being a fourth spelling of the same application. It is also the convention `~/Library/Application
Support` is organised by, which is where most of this application's users will look for it.

The log *file* does not follow the name. It is `opai.log` under any application name, because that is
what a user is asked to attach and specs/logging/spec.md fixes it; only the directory above it is
`APP_NAME`'s. So the path in a support instruction is `~/.config/io.vinicius.opai/logs/opai.log`.

The cost is a user who has the Go app downloading the ~175 MB runtime and the models a second time.
Accepted: it is one download against two applications writing each other's manifests.

### D9. A panic hook, and the parity gap it does not close

`std::panic::set_hook` around the previous hook: emit an error record carrying the payload's message
and the `PanicHookInfo`'s location, then call the hook that was there. The default behaviour —
the message on stderr, the backtrace, the unwind or abort — is unchanged; only the record is added.

This is the one thing Go's stderr capture gave that the ORT callback does not. Go owned descriptor 2,
so the Go runtime's own crash output landed in `native.log` and was folded back on the next run; here
nothing owns descriptor 2 and a Rust panic's message would otherwise reach a terminal the GUI does
not have.

**What stays uncovered, stated rather than implied:** output from a native library that writes to
stderr and is not ONNX Runtime — a CUDA or cuDNN loader message, a driver warning — and the abort
message of a hard native crash. Go caught those. The trade is deliberate: the alternative is porting
the whole capture stack to keep hold of output whose one important source now arrives through a
callback, and the failure that motivated it (issue #40) is *better* covered here, because ORT's own
account of the failure is written as it is produced rather than replayed from a file on the next run.
Recorded as a known parity gap for the archive.

### D10. Instrumented here: the initialization path, and nothing else

The records this slice adds are the ones on the path that already exists and runs at every launch:
what `Opai::initialize` is doing and under what name, the runtime started and from where, the image
cache's mode and path, the single-instance claim, and the two things the front ends say — the GUI's
D-Bus gate (`eprintln!` today, `warn!` after this) and the second-instance raise slice 2 added.

That is roughly a dozen records against Go's 84, chosen because they are the ones that make the file
non-empty on a launch that goes well and self-explanatory on one that does not. Everything else —
the cache hit and miss lines, the per-operation timings, the session build, the provider fallbacks,
the installer's manifest decisions — is slice 4, where each one gets a level and a field set decided
against the Go record it replaces.

`libpath.rs`'s two `eprintln!` calls stay `eprintln!`, and the reason is the same one Go writes down
twice: they run *before* the sink can exist, because the process is about to replace its own image
with `exec`. Logging there would write a divider and a header into the file for a process that is
about to stop existing, and then do it again in the new image.

### D11. The GUI installs the sink first, before the single-instance plugin

Slice 2's D2 binds `Opai::initialize` to come after the plugin — *"a second GUI that initialized
before the plugin runs would take slice 1's claim, be refused by the first process's lock, and surface
`InitError::AlreadyRunning`"*. It does not bind the sink, which takes no claim and refuses nobody.

The sink goes first, as Go's does, so that the D-Bus gate's decision, the plugin's own behaviour and
any panic during `Builder::build()` are all inside the log rather than ahead of it. The consequence
is that a second GUI launch writes a divider, a header and one record before the plugin exits it —
which is exactly what the Go app does today, and what the `app=` field and the divider make legible.
The one sharp edge in it is rotation, and it is in the risks below rather than hidden here.

## Risks / Trade-offs

**A refused second process can rotate the log out from under the first** → The window is narrow and
real: a GUI left running since yesterday, a second launch today, and the second process opens the
rotator, sees a stale live file and renames it before the plugin exits it. The first process keeps
writing into the renamed inode, so the rest of its session lands in a file named for yesterday. On
Windows the rename fails instead, with a sharing violation the rotator reports as a write error that
the subscriber discards, and the second process simply appends. Not defended against: the cost is a
day's worth of one session's records being in the previous day's file, the same exposure the Go app
has had since the day it shipped rotation, and closing it means the GUI logging nothing until the
plugin has spoken — which costs the records from the launch path that most needs them.

**Synchronous writes put a lock and a syscall on the emitting thread** → D7's deliberate trade. At
this slice's volume it is unmeasurable; the risk is slice 4 adding a record to a per-tile or per-node
path, where it would be measurable. Mitigation is the level of the record, not the writer: `debug`
records cost a global load and a branch when the filter says `info`.

**ORT's callback runs on C++ threads, into our formatter, holding our lock** → A panic inside a
`tracing` event formatter, unwinding through an `extern "system"` frame, is undefined behaviour in the
general case and an abort in practice. The formatter is written to have no fallible path of its own —
it writes into a `fmt::Writer` and lets the writer's errors be discarded, as `fmt` already does — and
the lock is a plain `Mutex` whose poisoning is handled rather than unwrapped. This is worth naming
because it is the one place in this design where our code runs on a thread we did not start, under a
C++ frame that cannot accept an unwind.

**`io.vinicius.opai` as the directory name strands the Go app's installed data** → By design (D8), but it is a
real cost for anyone with the Go app installed: the runtime and the models are downloaded again.
There is no migration and none is planned; a later change could offer to adopt an existing
`open-photo-ai` tree, and would then have to answer what happens when both applications are running.

**The log is written before anything validates that the application should be running** → The sink is
installed at the top of `main`, so a process that is about to be refused by slice 1's claim, or by the
plugin, still creates the directory and writes two lines. Accepted: creating `logs/` and appending a
divider is not the class of side effect the claim exists to prevent — no transfer, no install, no
cache — and slice 1's ordering rule is about what `initialize` does, which this is not.

**`RUST_LOG` is a public knob that can be set to something expensive** → `RUST_LOG=trace` will ask ORT
for per-node verbosity through a filter that no longer floors it at `warn`, on a file that rotates
daily and keeps a week. Bounded by the C++ clamp, which `initialize` sets to warning independently of
the subscriber, so the flood needs both dials moved. Documented rather than defended.

**`file-rotate`'s last release is from early 2025** → Not abandoned by download count or by API
stability, but not on a monthly cadence either. The exposure is small and bounded: it is a
`std::io::Write` behind a `MakeWriter`, so replacing it means changing the two lines that construct it
and nothing that emits a record. That is the same shape of reasoning slice 1 used for `fs4` — and the
same conclusion, which is that the sink's interface to the rest of the project is one trait.

## Platforms

The design targets **macOS, Windows and Linux**, and every mechanism in it is cross-platform by
construction: `tracing` and `tracing-subscriber` have no platform surface, `file-rotate` is pure Rust
over `std::fs`, and the ORT callback is the same C API on all three.

**Validated on none of them yet** — this is a design. The honest statement is the one slice 1 made:
this project's live checks have only ever run on macOS arm64. What CI can check hermetically on all
three, and what the test tasks cover, is the file's content and shape: the divider, the header, the
`app=` field on every line, a stale file rotating at startup, retention pruning at seven, a second
`init` refused, and a failure to open the directory leaving the application running.

Two platform-specific things are worth naming because CI will and will not catch them:

- **Rotation while another process holds the file open** differs — POSIX renames it happily, Windows
  refuses without `FILE_SHARE_DELETE`. It is in the risks above rather than tested, because it needs
  two real processes and a date boundary.
- **ORT's own output format no longer matters on any platform.** Go needed `applePrefix` in its parser
  because macOS reformats stderr when it is not a terminal; the callback delivers structured
  arguments, so that whole class of platform difference is gone rather than handled.

**Execution providers are untouched.** No provider is resolved, configured or built differently; what
changes is where a provider's own complaint about attaching is written down, which is the diagnostic
this whole change is for.

## Division across the crates

**`opai` holds all of it**: the `logging` module, the rotator, the format, the filter, the panic hook,
`APP_NAME`, and the initialization-path records. Every binary needs a log; the rule this project sets
is that such code lives in the library.

**`gui` and `cli` contribute one call each.** `logging::init(APP_NAME, App::Gui)` at the top of `run()`
before the plugin registration, and `logging::init(APP_NAME, App::Cli)` at the top of `main()`. Neither
holds a guard, flushes anything, knows where the file is, or decides a level. `gui` additionally
turns its existing D-Bus `eprintln!` into a `warn!`, which is a record moving from a stream the
application does not have into the file it now does.

**`perf` does not exist yet** and will pass `App::Perf` when it does, plus whatever `--verbose`
console layer slice 6 decides it wants alongside the file.

**`rust-sak` features used: none new.** The configuration directory is already resolved through
`fs::mk_user_config_dir` by `config::app_dir`, and `logging::log_path` uses `fs::user_config_dir` —
the non-creating variant `config::sub_dir` already borrows the verdict of — so that asking where the
file is creates nothing. `o11y` is not a candidate and proposal.md says why: it is an OTLP network
exporter, not a local sink. A rotating file sink is general-purpose enough to belong in `rust-sak`
one day, and is staying here on slice 1 D9's terms — the series is not sequenced behind a `rust-sak`
release. The seam is narrower than the lock's was: what is liftable is the rotator and the `MakeWriter`
(two dozen lines around a third-party crate), while the divider, the `app=` field and the session
header are this application's decisions and would not travel.
