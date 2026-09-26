//! The file sink: where an application's records go, and what a reader finds there.
//!
//! One file, `<config dir>/logs/opai.log`, shared by every binary of this project. [`init`] opens it, installs the
//! subscriber and returns the path; after that, library code emits through `tracing` and nothing else in this crate
//! knows the sink exists.
//!
//! The subscriber has two destinations, each with its own filter: the file, and this project's collector. The second
//! is installed in every binary and sends nothing until the application calls
//! [`telemetry::start`](crate::telemetry::start), so the file is the only destination most processes ever have, and
//! it reads the same whether or not the other is sending.
//!
//! # The library does not own a logger
//!
//! There is no `set_logger`, no handle threaded through [`Opai`](crate::Opai), and no accessor. Library code calls
//! `tracing::info!`/`warn!`/`debug!` directly, so a library consumer that does not ask for logging gets none, and the
//! opt-in is spelled as installing any subscriber at all, including one this crate had nothing to do with.
//!
//! # Levels
//!
//! A level is a statement about what the application does next, and every module in this crate follows the same
//! rule rather than restating its own:
//!
//! - **`error`** is written for what the process does not survive: a panic, and nothing else.
//! - **`warn`** is written for a failure, and for a degradation the application continues through that costs the user
//!   something they did not choose. A failure returned to the caller is a `warn` even when the caller treats it as
//!   fatal, because whether it is fatal is the caller's decision. It is written once, by the layer where the failure
//!   happened.
//! - **`info`** is the account of units of work: each one opens with a record and closes with one — finished, failed
//!   or stopped. A stop (a cancellation or a shutdown) closes at `info` with its `reason`, never as a failure.
//! - **`debug`** is everything per step inside a unit, so a file's size is bounded by what was done rather than by the
//!   size of the images it was done to.

// With no subscriber installed, each of those macros is a load of a global, a `None` and a return — the arguments are
// never formatted. So silence by default is a property of the facade rather than of an API this crate maintains: the
// same guarantee the reference implementation buys with `internal.Log()` holding a `slog.DiscardHandler`.
//
// `init` is called by the binary, not by `Opai::initialize`, because `gui` calls `initialize` only when its window asks
// for setup, from a command, and would otherwise have no log for anything before that — the opposite of what a
// windowed application, which has no terminal to fall back on, needs. Logging is a property of the process;
// installation is a property of the application.
//
// ONNX Runtime's C++ diagnostics arrive here too, through `ort`'s `tracing` feature (see `runtime::start`), and they
// are the records that explain the two failures the reference implementation actually had reported against it: an
// execution provider that declined to attach, and a native crash inside a CUDA session. That feature replaces the
// whole of the reference's stderr-capture stack — a descriptor tailer and its Windows variant, a regex over ORT's
// text format, a second `native.log` and the replay of its unread tail on the next launch. The records also arrive
// **on the thread that produced them**, so they are on disk before the C++ call that emitted them returns — which is
// the property that stack was built to get and did not reach.
//
// Nothing is buffered: every record is a `write(2)` on the emitting thread, behind one `Mutex`.
// `tracing_appender::non_blocking` is the reflex here and is precisely wrong: it hands records to a worker thread,
// and a process dying from a signal or a native abort stops that thread with whatever it had not yet written still in
// the queue. The records worth having are the last ones before a failure, and a buffer is what loses exactly those.
//
// The cost is a lock and a syscall per record. It is affordable because of the volume: this crate writes a few dozen
// records per session at `info`, and ORT at warning writes a handful. If an instrumentation sweep ever makes a hot
// path chatty, the answer is that record's level — a `debug!` under an `info` filter costs a global load and a
// branch — and not a buffer between the record and the disk.

use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use rust_sak::fs::{self, FsError};
use thiserror::Error;

pub(crate) mod format;
#[cfg(test)]
pub(crate) mod observed;
mod panic;
mod writer;

use writer::{Rotator, Sink};

#[cfg(test)]
use crate::app::CLI;

// A directory of its own rather than the configuration root, so that the rotated history sits beside the live file
// instead of among `cache/`, `models/`, `runtime/`, `libs/` and the instance lock.
/// The directory the log file lives in, beneath the application's configuration directory.
const LOGS_DIR: &str = "logs";

// Fixed because it is what a user is asked to attach to a bug report, so *"attach
// `~/.config/io.vinicius.opai/logs/opai.log`"* has to go on being a sentence somebody can write. Rotation may move the
// old records out from under this name; it may never move the live ones. See `writer` for what that requirement cost
// in the choice of rotator.
/// The live log file, and the name is fixed.
///
/// It does **not** follow [`APP_NAME`](crate::APP_NAME): the directory above it is the application's, this is fixed.
const LOG_FILE: &str = "opai.log";

/// The field that keeps a record out of the collector: an event declaring it is written to the file only.
///
/// For a record whose account reaches the collector by another path, so the collector is not told the same fact twice.
/// Only its presence is read, so any value marks the record; `true` is what the file then says. Braces make a constant a
/// field name, and a level macro needs the fields in a block of their own around it:
/// `tracing::warn!({ { opai::logging::FILE_ONLY } = true, error }, "…")`.
pub const FILE_ONLY: &str = "file_only";

/// Whether this process has installed a sink.
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Everything that can stop an application from getting a log file.
///
/// Every variant is a reason to start **unlogged**, never a reason not to start: an application that cannot write a
/// log is still an application. There is no variant for a failed *write*: a write that fails after the sink is
/// installed is discarded by the subscriber, as `tracing` discards every writer error.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LogError {
    /// A sink is already installed in this process.
    ///
    /// It reports that *this* caller did not install the sink — the one installed first is untouched and still
    /// writing — so a caller seeing it should go on without installing anything rather than retrying.
    #[error("a log sink is already installed in this process")]
    AlreadyInstalled,

    /// The application's configuration directory could not be named.
    ///
    /// The platform has none, or `name` could not be a directory. Raised by resolving the path alone, so it is the
    /// one failure [`log_path`] can report without anything having been created.
    #[error("the log directory could not be resolved: {0}")]
    Fs(#[from] FsError),

    /// `app` would not survive being recorded.
    ///
    /// The same rule, and the same message, [`InitError::InvalidApp`](crate::InitError::InvalidApp) reports: the
    /// identity must be non-empty and carry neither whitespace nor a control character.
    #[error("application identity {app:?} cannot be recorded: {reason}")]
    InvalidApp {
        /// The rejected identity, quoted back so the caller can see what was passed.
        app: String,
        /// Which rule it broke.
        reason: &'static str,
    },

    /// The log directory or the log file could not be created or opened.
    #[error("the log file at {path} could not be opened: {source}")]
    Io {
        /// The file or directory the operation was against.
        path: PathBuf,
        /// What the operating system said.
        #[source]
        source: std::io::Error,
    },
}

impl LogError {
    /// An [`LogError::Io`] against `path`, for the `map_err` of a filesystem call.
    fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        |source| Self::Io { path: path.into(), source }
    }
}

/// Where the log file for `name` is, **without creating anything or installing anything**.
///
/// `<platform config dir>/<name>/logs/opai.log`. The path is derived from the application name alone, so an interface
/// can tell a user where to look whether or not this process is the one logging there. [`init`] opens the path this
/// returns.
///
/// # Errors
///
/// Returns [`LogError::Fs`] if the platform has no configuration directory or `name` could not be one.
pub fn log_path(name: &str) -> Result<PathBuf, LogError> {
    Ok(log_dir(name)?.join(LOG_FILE))
}

/// The directory the log file lives in, resolved and not created.
fn log_dir(name: &str) -> Result<PathBuf, LogError> {
    // Through `rust-sak`'s non-creating `fs::user_config_dir` rather than the `mk_` variant `config::app_dir` uses,
    // which is the whole of what makes `log_path` free of side effects.
    Ok(fs::user_config_dir(name, LOGS_DIR)?)
}

/// Installs the process's log sink and returns the path of the file it opened.
///
/// `name` is the application name the configuration directory is resolved from — [`APP_NAME`](crate::APP_NAME) for
/// every binary of this project. `app` is the identity this process declares, and it is written onto **every** record
/// in the file, so that two sessions interleaved in one file can be told apart. This project's binaries pass
/// [`GUI`](crate::GUI), [`CLI`](crate::CLI) or [`PERF`](crate::PERF); an application embedding this library passes
/// its own.
///
/// It should be the same identity passed to [`Opai::initialize`](crate::Opai::initialize) — see
/// [`InitOptions::app`](crate::InitOptions::app) — and it is held to the same rule here, against the same check. The
/// sink is deliberately installed *before* `Opai` exists, which is what puts a refused second launch's message inside
/// the log rather than ahead of it; a front end that never initializes at all therefore reaches this entry point and
/// no other, so validating only at the other one would leave the identity on every line of its log unchecked.
///
/// Call it once, at the top of the binary's entry point, before anything that might have something to say. It does
/// six things in this order: resolves and creates `<config dir>/logs`, opens `opai.log` for append through the
/// rotator, writes a `---` divider, installs the subscriber (the file and, dormant until
/// [`telemetry::start`](crate::telemetry::start), the collector), installs the panic hook, and emits the session
/// header.
///
/// # What ends up in the file
///
/// Records from this library, from the front end that called this, and from ONNX Runtime's own C++ diagnostics — all
/// at `info` and above by default, with the runtime floored at `warn` separately. `RUST_LOG` overrides both for one
/// run.
///
/// The file is appended to, never replaced, and is rotated daily with seven compressed days kept.
///
/// # Nothing to close
///
/// There is no guard to hold and no flush to remember. Every record is written synchronously, so there is nothing
/// buffered at exit, and the subscriber is installed for the life of the process. The reference implementation
/// returns an `io.Closer` here; it has one because it runs a rotation worker and because it took ownership of
/// stderr, and this does neither.
///
/// # Errors
///
/// Returns [`LogError::InvalidApp`] if `app` would not survive being recorded — checked before anything else, so a
/// rejected identity does not consume the one-shot latch below — [`LogError::AlreadyInstalled`] if this process
/// already installed one (the first sink keeps working), [`LogError::Fs`] if the configuration directory could not be
/// named, and [`LogError::Io`] if `logs/` could not be created or `opai.log` could not be opened.
///
/// **A failure here is not a reason to stop.** A front end reports it and runs unlogged.
pub fn init(name: &str, app: &str) -> Result<PathBuf, LogError> {
    // Latched **first**, so that a second caller is refused before it can create a directory or open a file. Doing
    // the work and then discovering the latch would leave the second call's side effects behind on a path whose whole
    // promise is that it changed nothing.
    if let Err(reason) = crate::app::check(app) {
        return Err(LogError::InvalidApp { app: app.to_string(), reason });
    }

    if INSTALLED.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Err(LogError::AlreadyInstalled);
    }

    // From here on a failure has to clear the latch again, or a front end that reported the failure and carried on
    // would have permanently poisoned a sink it never installed.
    match install(name, app) {
        Ok(path) => Ok(path),
        Err(error) => {
            INSTALLED.store(false, Ordering::SeqCst);
            Err(error)
        }
    }
}

/// [`init`]'s body, once the latch is held.
///
/// Separated so that every early return above is a latch this function does not have to know about, and every early
/// return here is one the caller clears.
fn install(name: &str, app: &str) -> Result<PathBuf, LogError> {
    let dir = log_dir(name)?;
    let path = dir.join(LOG_FILE);

    // Created here rather than left to `FileRotate`, which creates its parent with an `.expect("create dir")` — a
    // panic at the top of `main` for a read-only home directory, where the specified behaviour is an error and an
    // application that starts unlogged.
    std::fs::create_dir_all(&dir).map_err(LogError::io(&dir))?;

    // Opened before the rotator is built, for the same reason: `FileRotate` swallows a failed open into a `None`
    // file and goes on discarding every write. This is the one place that can still tell a caller the file is not
    // writable, so it is checked here and the handle dropped.
    OpenOptions::new()
        .read(true)
        .create(true)
        .append(true)
        .open(&path)
        .map_err(LogError::io(&path))?;

    let sink = Sink::new(Rotator::open(&path));

    // The divider goes through the sink rather than to the file directly, so it is subject to the same rotation
    // decision every record is: a live file whose last write was yesterday is rotated *by* this write, and this
    // session's divider is therefore the first line of a new `opai.log` rather than the last line of yesterday's.
    sink.write_line(DIVIDER);

    let subscriber = format::subscriber(app, sink, format::filter());

    // `set_global_default` can only fail if something else already installed one, which the latch above has already
    // ruled out for this crate — but not for a consumer that installed its own subscriber before calling this. That
    // is their sink and their decision, so it is left in place and this reports nothing: the records still go
    // somewhere, just not here.
    let _ = tracing::subscriber::set_global_default(subscriber);

    panic::install_hook();

    // Probed after the subscriber is installed, so a machine whose adapters cannot be enumerated gets the warning
    // saying so in the file — one line above the header rather than nowhere.
    let hardware = crate::hardware::snapshot();

    // Last, so that it is the first record under this session's divider, and after the subscriber so that it goes
    // through the same format every other record does.
    // No `app` field of its own: the formatter writes it onto every line, this one included, so naming it here would
    // render it twice.
    tracing::info!(
        version = crate::version(),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        cpu_model = %hardware.cpu_model,
        cpu_cores = hardware.cpu_cores,
        memory = %crate::hardware::gib(hardware.memory),
        gpus = %hardware.gpus_summary(),
        "Open Photo AI starting"
    );

    Ok(path)
}

/// A sink writing into `path`, for a test that needs the real writer without the real configuration directory.
///
/// The rotator and the lock exactly as [`init`] builds them, so a test asserting on a record is asserting on the
/// bytes a reader of `opai.log` would find rather than on something a collector reconstructed.
#[cfg(test)]
pub(crate) fn test_sink(path: &std::path::Path) -> impl for<'a> tracing_subscriber::fmt::MakeWriter<'a> + 'static {
    Sink::new(Rotator::open(path))
}

/// The subscriber [`init`] installs, at `directives` instead of from the environment.
///
/// So that a test elsewhere in this crate can bind it with `set_default` for one call — which the suite needs,
/// because `init` itself installs globally and may be called once per process.
#[cfg(test)]
pub(crate) fn test_subscriber<W>(app: &str, writer: W, directives: &str) -> impl tracing::Subscriber + Send + Sync
where
    W: for<'a> tracing_subscriber::fmt::MakeWriter<'a> + Send + Sync + 'static,
{
    format::subscriber(app, writer, tracing_subscriber::EnvFilter::new(directives))
}

/// Installs a permissive global subscriber, once per test process, so that no callsite is ever cached as "never".
///
/// **This is not a sink and records nothing.** It exists for one property of `tracing`: a callsite's interest is
/// computed *once*, the first time that callsite is reached, and cached in a **global** — and when only scoped
/// subscribers exist, that computation asks whatever dispatcher is current *on the registering thread*. The suite
/// runs its tests in parallel, so a record this crate emits is very often first reached by a thread with no
/// subscriber at all, which caches it as `never` for the rest of the process. A later [`records_of`] would then find
/// that record missing from its file with nothing wrong at the call site — which is a flake that looks exactly like
/// a bug in the code under test.
///
/// A global that answers [`Interest::sometimes`] for every callsite and `false` to every [`enabled`] leaves the
/// decision to whichever subscriber is actually current: the recording thread's, where there is one, and this one —
/// which discards everything — everywhere else.
#[cfg(test)]
fn ask_always() {
    use tracing::span;
    use tracing::subscriber::Interest;
    use tracing::{Event, Metadata};

    struct AskAlways;

    impl tracing::Subscriber for AskAlways {
        fn register_callsite(&self, _: &'static Metadata<'static>) -> Interest {
            Interest::sometimes()
        }
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            false
        }
        fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
            span::Id::from_u64(1)
        }
        fn record(&self, _: &span::Id, _: &span::Record<'_>) {}
        fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}
        fn event(&self, _: &Event<'_>) {}
        fn enter(&self, _: &span::Id) {}
        fn exit(&self, _: &span::Id) {}
    }

    static INSTALLED: std::sync::Once = std::sync::Once::new();

    INSTALLED.call_once(|| {
        let _ = tracing::subscriber::set_global_default(AskAlways);
    });
}

/// Held for the length of every [`records_of`] call, so that two of them never overlap.
///
/// Not about the file — each call has a temporary directory of its own — but about `tracing`'s **global** maximum
/// level. Installing a subscriber rebuilds that maximum from the subscriber's own filter, and it is a process-wide
/// value rather than a thread-local one: a test holding an `info` subscriber on one thread therefore makes
/// `tracing::debug!` a no-op on every other, so a test asserting on a debug record beside it would find an empty
/// file. The suite runs its tests in parallel, which is what turns that into a flake rather than a constant.
///
/// Taken through [`lock`](crate::task::lock), so a test that panicked while recording does not poison every later
/// one.
#[cfg(test)]
static RECORDING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The sink, the subscriber and the two globals every [`records_of`] call needs, held until it is dropped.
///
/// The two helpers below differ only in whether they await `emit`. Everything around it — the temporary directory,
/// the real rotator, the thread-bound subscriber, the interest-cache rebuild and the lock that keeps a parallel suite
/// from stepping on the global maximum level — is identical, so it lives here once rather than twice where the two
/// could drift.
///
/// # The [`RECORDING`] guard is held across [`records_of`]'s awaits, deliberately
///
/// Which is what `clippy::await_holding_lock` exists to ask about, and holding it is the point: the subscriber is
/// bound to the calling thread, so the guard has to outlive the work being recorded. A `tokio::sync::Mutex` would be
/// wrong rather than better — it would let the future resume on a thread the subscriber is not installed on. Each
/// `#[tokio::test]` owns a current-thread runtime, so nothing else can contend for this lock from inside the same
/// one.
#[cfg(test)]
struct Recording {
    /// Held so the directory outlives the read in [`finish`](Self::finish), and no longer.
    dir: tempfile::TempDir,
    path: PathBuf,
    guard: tracing::subscriber::DefaultGuard,
    recording: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl Recording {
    /// Installs the sink on **this thread** at `directives`, with `observer` beside the collector's layer if given.
    fn start(directives: &str, observer: Option<observed::Observer>) -> Self {
        ask_always();
        let recording = crate::task::lock(&RECORDING);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(LOG_FILE);
        let sink = test_sink(&path);

        let guard = match observer {
            None => tracing::subscriber::set_default(test_subscriber(CLI, sink, directives)),
            Some(observer) => tracing::subscriber::set_default(format::observed_subscriber(
                CLI,
                sink,
                tracing_subscriber::EnvFilter::new(directives),
                observer,
            )),
        };

        // Every callsite registered before now is re-evaluated against the subscriber just installed. See
        // [`ask_always`] for the half of the problem that is about callsites registered *after* it.
        tracing::callsite::rebuild_interest_cache();

        Self { dir, path, guard, recording }
    }

    /// The file's text, with the subscriber uninstalled first so nothing can still be writing into it.
    fn finish(self) -> String {
        let Self { dir, path, guard, recording } = self;

        drop(guard);
        let text = std::fs::read_to_string(&path).unwrap();
        drop(dir);
        drop(recording);

        text
    }
}

/// Every record `emit` produced, as this crate's own formatter renders them, at `directives`.
///
/// A real sink over a real file and the subscriber [`init`] installs, so what a caller asserts on is the text a
/// reader of `opai.log` would actually find — **not** a field set observed through a test-only collector, which could
/// pass while the file said something else. That is the whole reason this returns a `String` rather than a handle to
/// something structured, and why [`records`] and [`field`] read it back as text.
///
/// `directives` is the filter, so one test can drive the same code at `info` and at `debug` and assert that the
/// second says more than the first. That is what makes the volume requirement — a run's default-level record count
/// bounded by what was asked for rather than by the size of the image — a test rather than a claim.
///
/// # It binds to the calling thread
///
/// Scoped with `set_default` rather than installed globally, because the suite shares one process — and
/// `set_default` binds the subscriber to **the thread that called it**. The awaits inside `emit` therefore have to
/// stay on that thread, which they do under the current-thread runtime `#[tokio::test]` gives: a record emitted from
/// `spawn_blocking`, or from any other worker, is not seen here and its absence is not a failure of the code that
/// emitted it. Records from a blocking thread are asserted through the seam that calls it instead, or by binding the
/// recording's own dispatcher there — `tracing::dispatcher::get_default` hands it out inside `emit` — as `task`'s test
/// does.
#[cfg(test)]
pub(crate) async fn records_of<F, Fut, T>(directives: &str, emit: F) -> (String, T)
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    let recording = Recording::start(directives, None);
    let value = emit().await;

    (recording.finish(), value)
}

/// [`records_of`], also returning what reached the collector's layer: its spans, their parents, links and fields.
///
/// The same file and the same thread binding. Work handed to a blocking thread through
/// [`task::spawn_blocking`](crate::task::spawn_blocking) is seen too, since that carries the caller's subscriber
/// across.
#[cfg(test)]
pub(crate) async fn traced_of<F, Fut, T>(directives: &str, emit: F) -> (String, observed::Observed, T)
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    let observer = observed::Observer::default();
    let recording = Recording::start(directives, Some(observer.clone()));
    let value = emit().await;
    let log = recording.finish();

    (log, observer.observed(), value)
}

/// [`traced_of`] for code that is not `async`.
#[cfg(test)]
pub(crate) fn traced_of_blocking<F, T>(directives: &str, emit: F) -> (String, observed::Observed, T)
where
    F: FnOnce() -> T,
{
    let observer = observed::Observer::default();
    let recording = Recording::start(directives, Some(observer.clone()));
    let value = emit();
    let log = recording.finish();

    (log, observer.observed(), value)
}

/// [`records_of`] for code that is not `async`.
///
/// Same sink, same formatter, same thread binding — the installers and the chain are `async`, but the cache, the
/// session registry and the model listing's resolution are not, and wrapping them in a future to assert on a record
/// would be ceremony around nothing.
#[cfg(test)]
pub(crate) fn records_of_blocking<F, T>(directives: &str, emit: F) -> (String, T)
where
    F: FnOnce() -> T,
{
    let recording = Recording::start(directives, None);
    let value = emit();

    (recording.finish(), value)
}

/// Every line of `log` whose `msg=` is `message`.
///
/// Beside [`records_of`] rather than in each module that asserts on one, for the reason that helper was promoted out
/// of `lib.rs` in the first place: five modules assert the same way, and a matcher copied five times is five chances
/// for one of them to match something subtly different. The bare and quoted forms are both looked for because the
/// formatter quotes a value only where it contains a space.
#[cfg(test)]
pub(crate) fn records<'a>(log: &'a str, message: &str) -> Vec<&'a str> {
    let bare = format!("msg={message} ");
    let quoted = format!("msg=\"{message}\"");

    log.lines().filter(|line| line.contains(&bare) || line.contains(&quoted)).collect()
}

/// The value of `field` on `line`, unquoted, or `None` where the line does not carry it.
///
/// A field is ` key=value` with the value quoted only where it contains a space — so this finds the key preceded by a
/// space, which is what keeps `requested=` from matching inside `provider=`.
#[cfg(test)]
pub(crate) fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.split_once(&format!(" {key}="))?.1;

    Some(match rest.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next().unwrap_or_default(),
        None => rest.split(' ').next().unwrap_or_default(),
    })
}

/// The line that separates one session from the next: three hyphens and nothing else.
///
/// Three hyphens rather than a formatted record, because it is read by a person scrolling a file rather than by
/// anything that parses it, and because it has to be written *before* the subscriber exists — the divider's whole job
/// is to be the thing above this session's first record.
///
/// Written unconditionally, including after a session that ended abruptly, since that is the case a reader most needs
/// to find the boundary of.
const DIVIDER: &str = "---";

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // The real [`init`] resolves the platform's configuration directory, which a unit test cannot redirect and which
    // it must not install a subscriber into — the suite shares one process. So the tests here cover the parts that
    // take a path ([`writer`]) or a subscriber of their own ([`format`], [`panic`]), and the one test that drives a
    // real `init` end to end is `tests/logging_session.rs`, which owns its process.

    #[test]
    fn asking_where_the_log_is_creates_nothing() {
        // A unique name so the assertion is about this call rather than about whatever else has run on this machine.
        let name = "opai-log-path-test";
        let path = log_path(name).unwrap();

        assert!(path.ends_with(Path::new(LOGS_DIR).join(LOG_FILE)));
        assert!(path.parent().unwrap().ends_with(LOGS_DIR));
        assert!(!path.exists(), "asking for the path created the file");
        assert!(!path.parent().unwrap().exists(), "asking for the path created the directory");

        // And the application's directory itself, which `mk_user_config_dir` would have created.
        let app_dir = path.parent().unwrap().parent().unwrap();
        assert!(!app_dir.exists(), "asking for the path created the application directory");
    }

    #[test]
    fn the_path_is_built_from_the_name_alone() {
        // Two names give two files, which is what makes the path derivable without installing anything: nothing but
        // `name` is consulted.
        let one = log_path("opai-log-path-a").unwrap();
        let two = log_path("opai-log-path-b").unwrap();

        assert_ne!(one, two);
        assert_eq!(one.file_name(), two.file_name());
        assert_eq!(one.file_name().unwrap(), LOG_FILE);
    }

    #[test]
    fn a_name_that_cannot_be_a_directory_is_refused_rather_than_resolved() {
        for name in ["", "../escape"] {
            let error = log_path(name).unwrap_err();
            assert!(matches!(error, LogError::Fs(_)), "{name:?} gave {error:?}");
        }
    }

    #[test]
    fn a_second_installation_in_one_process_is_refused() {
        // The latch is process-wide and this test shares a process with every other unit test, so it is driven
        // directly rather than through `init` — which would resolve the real configuration directory and install a
        // subscriber that the rest of the suite would then be running under.
        assert!(INSTALLED.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok());

        let refused = init("opai-second-init-test", CLI);
        assert!(matches!(refused, Err(LogError::AlreadyInstalled)), "got {refused:?}");

        // And the refusal created nothing: the second caller is told before it touches the disk.
        let path = log_path("opai-second-init-test").unwrap();
        assert!(!path.parent().unwrap().exists(), "a refused init created the log directory");

        INSTALLED.store(false, Ordering::SeqCst);
    }

    #[test]
    fn an_identity_that_could_not_be_recorded_is_refused_before_anything_is_installed() {
        // The half of the rule that had no enforcement: a front end reaches this entry point before `Opai` exists,
        // and one that never initializes reaches no other. An identity carrying whitespace would split the `app=`
        // field and break the one-record-per-line property the formatter rests on.
        let refused = init("opai-invalid-app-test", "My Photos");
        assert!(
            matches!(&refused, Err(LogError::InvalidApp { app, reason }) if app == "My Photos" && *reason == "it contains whitespace"),
            "got {refused:?}"
        );

        // Ahead of the latch as well as the disk — the refusal is `InvalidApp` rather than `AlreadyInstalled`, which
        // it could not be if the check ran second in a process whose latch the suite has already taken. (Not asserted
        // on the latch directly: it is process-wide and the sibling test above toggles it.)
        //
        // Nothing was created either, for a call that was never going to install anything.
        let path = log_path("opai-invalid-app-test").unwrap();
        assert!(!path.parent().unwrap().exists(), "a refused init created the log directory");
    }

    #[test]
    fn a_log_directory_that_cannot_be_created_is_an_error_rather_than_a_panic() {
        // A file where the `logs` directory has to go, which is the portable way to make `create_dir_all` fail on
        // every platform — a read-only parent is not, since a test process may be running as a user it does not stop.
        let root = tempfile::tempdir().unwrap();
        let app_dir = root.path().join("opai-unwritable");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::write(app_dir.join(LOGS_DIR), b"not a directory").unwrap();

        let dir = app_dir.join(LOGS_DIR);
        let error = std::fs::create_dir_all(&dir).map_err(LogError::io(&dir)).unwrap_err();

        assert!(matches!(error, LogError::Io { .. }), "got {error:?}");
        assert!(error.to_string().contains(LOGS_DIR));
    }

    #[test]
    fn the_divider_is_three_hyphens_and_nothing_else() {
        // Pinned as a literal: it is what a reader looks for to find where a session begins, and what the tests for
        // the file's shape count.
        assert_eq!(DIVIDER, "---");
        assert!(!DIVIDER.contains('\n'));
    }
}
