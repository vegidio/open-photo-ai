## 1. Dependencies and vocabulary

- [x] 1.1 Add `tracing`, `tracing-subscriber` (`registry`, `fmt`, `env-filter`) and `file-rotate` to
      `crates/opai/Cargo.toml`, and add `tracing` to the `ort` feature list in both the dependency and
      the dev-dependency entries, each with a manifest comment in the file's established style saying
      why it is there and what was rejected (design D4). Verify `cargo tree -p opai -i tracing` shows
      one copy and `cargo build -p opai` succeeds.
- [x] 1.2 Add `pub const APP_NAME: &str = "io.vinicius.opai"` to `crates/opai/src/lib.rs` with the
      reasoning from design D8 — one name so two front ends cannot resolve two configuration
      directories, the bundle identifier `crates/gui/tauri.conf.json` already carries so one
      reverse-DNS name answers the directory and the single-instance key together, and deliberately
      not the Go app's `open-photo-ai`. Verify a test asserts its value, since it is the directory
      every user's data lives in.

## 2. The sink

- [x] 2.1 Add `crates/opai/src/logging/mod.rs` with `init(name, app) -> Result<PathBuf, LogError>` and
      `log_path(name) -> Result<PathBuf, LogError>`, the once-per-process `AtomicBool` latch, and
      `LogError` via `thiserror`. `log_path` must resolve through `rust_sak::fs::user_config_dir` and
      create nothing. Verify unit tests: `log_path` creates no directory, a second `init` in one
      process is refused, and a `logs` directory that cannot be created returns an error rather than
      panicking.
- [x] 2.2 Build the writer: `FileRotate` at `<config>/logs/opai.log` with daily rotation, seven files,
      gzip on rotate and `append(true)`, behind a `Mutex` exposed through a `MakeWriter` whose
      `Writer` is the lock guard (design D4, D7). Verify tests in a temporary directory: a stale live
      file is rotated before the session writes, a rotated file is gzip-compressed and dated, and
      retention prunes to seven.
- [x] 2.3 Implement the `FormatEvent` line — `time`, `level`, `app`, `target`, `msg`, the event's own
      fields, then the enclosing spans' fields from `ctx.event_scope()` (design D5). Verify unit tests
      over an in-memory writer: the field order is as specified, `app=` carries `App::as_str`, a
      record inside a span renders that span's fields, and a message containing spaces or quotes is
      quoted the way `slog`'s text handler quotes it.
- [x] 2.4 Wire the filter — `EnvFilter` from `RUST_LOG`, defaulting to `info,ort=warn` (design D6).
      Verify tests: with no variable set, `debug!` is dropped and `info!` is kept; with
      `RUST_LOG=debug` set, `debug!` is kept; and the `ort` target stays at warning under the default.
- [x] 2.5 Compose `init`: create `logs/`, open the rotator, write the `---` divider through it, install
      the subscriber, then emit the session header naming the application, `version()`, OS and arch.
      Verify an integration test drives a real file in a temporary directory and asserts the first two
      lines are the divider and the header, and that a second session appends a second divider without
      truncating the first.
- [x] 2.6 Install the panic hook inside `init`, chaining the previous hook so the default message,
      backtrace and unwind are unchanged (design D9). Verify a test that catches an unwinding panic
      asserts an error record with the payload's message and its source location reached the writer.

## 3. The runtime's diagnostics

- [x] 3.1 Turn on the ORT bridge and cover it where it can be covered hermetically: rewrite
      `runtime.rs`'s comment at the `set_log_level` call, which currently says the feature *"stays off
      until there is a subscriber to receive them"*, to record that the subscriber now exists and that
      the C++ clamp is the first of design D6's two floors. Verify a test emitting through the `ort`
      target lands a record in the file with `target=ort`, its span fields rendered and warning
      severity under the default filter, and extend the existing live check to report that a real
      runtime's warnings appear in the log file and not on the terminal.

## 4. Records on the initialization path

- [x] 4.1 Instrument `Opai::initialize` and what it calls — initializing under a name and app, the
      single-instance claim, the image cache's resolved mode and path, the runtime started and from
      where, and initialization complete — at the levels design D10 sets, against the Go records in
      `../open-photo-ai/init.go`, leaving `libpath.rs`'s two `eprintln!` calls as they are with a
      comment saying why (they run before a sink can exist, because the process is about to `exec`
      over itself). Verify a test initializing against the test server asserts the expected records
      reached an installed test subscriber, and that `crates/opai/tests/reexec_linux.rs` still passes
      unchanged.

## 5. The front ends

- [x] 5.1 `gui`: call `logging::init(opai::APP_NAME, App::Gui)` at the top of `run()`, ahead of the
      single-instance plugin registration, reporting a failure to stderr and continuing (design D11).
      Convert the D-Bus gate's `eprintln!` and slice 2's second-instance raise into `warn!`/`info!`
      records. Verify `cargo test -p gui` passes and the existing ordering comment is extended to say
      why the sink is allowed ahead of the plugin while `Opai::initialize` is not.
- [x] 5.2 `cli`: call `logging::init(opai::APP_NAME, App::Cli)` before printing the version, continuing
      unlogged if it fails. Verify running the binary writes a divider and a header to
      `<config>/logs/opai.log` and still prints exactly the version string on stdout.

## 6. Surface and verification

- [x] 6.1 Extend `crates/opai/tests/public_api.rs` for the new surface — `APP_NAME`, `logging::init`,
      `logging::log_path`, `LogError` — building it the way a front end would, so the module is
      checked as something usable from outside the crate.
- [x] 6.2 Run `cargo fmt --check`, `cargo clippy --all-targets` and `cargo test` across the workspace,
      and confirm a real launch of `cli` leaves a readable `opai.log` whose lines match the shape in
      design D5.
