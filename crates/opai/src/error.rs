//! The error type initialization reports.

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

use rust_sak::fetch::DownloadError;
use rust_sak::fs::FsError;

use crate::app::Holder;
use crate::providers::ExecutionProvider;
use crate::task::Cancelled;

/// Everything that can stop the application from getting its dependencies onto disk — and, since a stale choice
/// is refused at the same boundary the downloads are set up at, the one piece of text the library parses rather
/// than assumes ([`InitError::UnknownProvider`]).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum InitError {
    // The three cases a caller is expected to tell apart carry named fields rather than a formatted string: an
    // application name that cannot be a directory, a platform the runtime is not published for, and an archive whose
    // bytes do not hash to what was pinned. A front end reports each differently, and the tests here assert on them.
    /// The application name cannot be used as a directory, so no configuration directory could be named from it.
    #[error("application name {name:?} is not usable as a directory: {reason}")]
    InvalidName {
        /// The rejected name, quoted back so the caller can see what was passed.
        name: String,
        /// Which rule it broke.
        reason: &'static str,
    },

    // The sibling of `InitError::InvalidName` and deliberately a separate variant: they reject two different
    // arguments, and a caller told only that "a name" was rejected would have to guess which one it passed.
    /// The identity the process declared would not survive being recorded, so nothing was claimed.
    ///
    /// Raised before the claim is taken, so a refused initialization has recorded nothing and installed nothing. The
    /// rule is that an identity must survive being written as one word.
    #[error("application identity {app:?} cannot be recorded: {reason}")]
    InvalidApp {
        /// The rejected identity, quoted back so the caller can see what was passed.
        app: String,
        /// Which rule it broke.
        reason: &'static str,
    },

    /// Another process of this user is already initialized, so this one is refused.
    ///
    /// The one failure here that is about another process rather than about this machine's disk, network or hardware,
    /// and it is raised before anything is probed, transferred or created — so a refused initialization has left
    /// nothing behind and spent nothing. See the `single-instance` capability for what the claim covers.
    ///
    /// `holder` is a **courtesy, not a verdict**. Whether this process may proceed was decided by the operating
    /// system's answer to the lock request; the record kept beside the lock file only says who to go and close. Every
    /// way of failing to read it — absent, empty, truncated, half-written, written by a version that used another
    /// format — arrives here as `None`, which is a refusal that names no holder rather than an error or, far worse, a
    /// grant.
    #[error("Open Photo AI is already running{}; close it and try again", holder_suffix(holder))]
    AlreadyRunning {
        /// Which application holds the claim and its process id, where that could be read.
        holder: Option<Holder>,
    },

    // Deliberately an error rather than a fallback: an unpinned platform has no expected hash, and installing
    // something unverified is worse than not starting.
    /// No archive is published for the operating system and architecture this is running on.
    #[error("no {dependency} is published for {os}/{arch}")]
    UnsupportedPlatform {
        /// The dependency that has no artifact for this platform.
        dependency: &'static str,
        /// [`std::env::consts::OS`] as reported by this build.
        os: &'static str,
        /// [`std::env::consts::ARCH`] as reported by this build.
        arch: &'static str,
    },

    // Deliberately a refusal rather than a fallback, and the one place this project refuses to do what the reference
    // implementation does: it composes a URL with an empty expected hash and installs the file unchecked, logging a
    // warning. That failure lands on a multi-gigabyte binary which is then loaded into the process, and an ordinary
    // timeout reaches it: the whole session then downloads models with nothing checking them, and says nothing.
    /// No published hash could be obtained for a model artifact, so there is nothing to verify it against.
    ///
    /// Reached only when the published listing cannot be read, no cached copy of it exists, **and** the artifact is
    /// absent from the manifest compiled into this binary — or when it is simply not published, which is the same
    /// answer to the same question. Nothing is transferred: an artifact no listing names has no URL worth requesting.
    #[error("no published hash is known for the model {artifact}, so it cannot be installed")]
    UnpublishedModel {
        /// The artifact that could not be placed, so a caller can say which model it was.
        artifact: String,
    },

    // Refused because a name that was never published is a bug in whatever produced it, and accepting it would let a
    // typo in a settings file reach a session build as a silent no-op.
    /// Text naming no published execution provider was parsed as one.
    ///
    /// The one thing about a provider choice that is refused rather than downgraded. A provider this machine cannot
    /// serve — CoreML on Linux, CUDA before its libraries are installed — is an ordinary answer and resolves to a CPU
    /// run.
    #[error("unknown execution provider {name:?}")]
    UnknownProvider {
        /// The text that was given, quoted back so a caller can show what it could not read.
        name: String,
    },

    /// The downloaded archive does not hash to the value pinned for it, after the one clean retry.
    #[error("{url} hashed to {actual} but {expected} was expected")]
    HashMismatch {
        /// Where the bytes came from.
        url: String,
        /// The pinned SHA-256.
        expected: String,
        /// What the bytes actually hashed to.
        actual: String,
    },

    /// The transfer itself failed — an HTTP status, a transport error, or a disk write.
    #[error("downloading {url} failed: {source}")]
    Download {
        /// The artifact being fetched, which the underlying error does not carry.
        url: String,
        /// The failure `rust-sak` reported.
        #[source]
        source: DownloadError,
    },

    /// The installed ONNX Runtime could not be loaded: the file is missing or unreadable, it exports no
    /// `OrtGetApiBase`, or it is an older runtime than this build was compiled against.
    ///
    /// Recoverable by reinstalling, which is what makes it worth telling apart from [`InitError::RuntimeStart`].
    #[error("failed to load the ONNX Runtime at {}: {source}", path.display())]
    RuntimeLoad {
        // `ort`'s own error names it in its message but not as a field, so it is carried here, as the URL is on every
        // download error.
        /// The library this was attempted against.
        path: PathBuf,
        /// What `ort` reported. [`ort::LoadDynamicError::BadVersion`] carries the version string it found.
        #[source]
        source: ort::LoadDynamicError,
    },

    /// The runtime library loaded but refused to create an inference environment.
    ///
    /// Unlike [`InitError::RuntimeLoad`] there is nothing to reinstall: the runtime is the published one and it did
    /// not start.
    #[error("failed to start the ONNX Runtime environment for {}: {source}", path.display())]
    RuntimeStart {
        /// The library the environment was started against.
        path: PathBuf,
        /// What `ort` reported.
        #[source]
        source: ort::Error,
    },

    // Why a retry cannot succeed in this process: see `runtime::FAILED_LOAD`.
    /// The runtime library already failed to load earlier in this process, so no later attempt can be made.
    ///
    /// The process has to be restarted; there is nothing a caller can do to make a second attempt in this one succeed.
    #[error("the ONNX Runtime at {} failed to load earlier in this process and cannot be retried: {reason}", path.display())]
    RuntimeUnavailable {
        /// The library the first attempt was made against.
        path: PathBuf,
        // A message rather than the error, a deliberate exception: `ort::LoadDynamicError` is neither `Clone` nor
        // reconstructible.
        /// What that attempt reported.
        reason: String,
    },

    /// A filesystem operation from `rust-sak` failed: resolving the configuration directory, or expanding the
    /// archive — including an archive entry refused for trying to write outside the install directory.
    #[error(transparent)]
    Fs(#[from] FsError),

    // The path is carried rather than left to the underlying error, which does not have one: `std::io::Error` says
    // `permission denied (os error 13)` and nothing about what was being written. No `#[from]`, deliberately — a bare
    // `?` is how the path gets lost, and every construction site here is next to the path it was working on.
    /// A filesystem operation this crate performs itself failed: recording the installed tree, or the manifest.
    #[error("{}: {source}", path.display())]
    Io {
        /// The file or directory the operation was against.
        path: PathBuf,
        /// What the operating system said.
        #[source]
        source: std::io::Error,
    },

    /// The asynchronous runtime shut down before work handed to a blocking thread could run.
    ///
    /// Its own variant rather than an [`InitError::Io`] over a `JoinError`, because it is the one failure here a
    /// caller can genuinely act on differently: nothing is broken and nothing needs reinstalling — the process is on
    /// its way down, and the right response is to stop rather than to report an install failure to a user who is
    /// already closing the application.
    #[error("initialization was cancelled: the runtime shut down before the work could run")]
    Cancelled,

    /// Every request waiting on this install asked to stop, so the transfer was stopped where it stood.
    ///
    /// **Not [`Cancelled`](Self::Cancelled)**, and the distinction is the same one [`InferenceError::Cancelled`] and
    /// [`InferenceError::Shutdown`] draw one layer up: that one is a process on its way down, this one is a user who
    /// changed their mind. A caller told the wrong one either reports a failure for something nobody asked to
    /// succeed, or keeps a multi-gigabyte transfer running for a window that has moved on.
    ///
    /// Nothing is broken and nothing needs reinstalling: what had arrived is still on disk as a partial, and the next
    /// request for the same artifact resumes onto it rather than starting again.
    #[error("the install was stopped: nothing was left waiting for it")]
    Stopped,
}

/// `" (gui, pid 4821)"`, or nothing at all where the holder could not be read.
fn holder_suffix(holder: &Option<Holder>) -> String {
    // A function rather than a `match` inside the attribute above, so that the message reads as one string in the
    // source the way it does on a terminal.
    holder.as_ref().map_or_else(String::new, |holder| format!(" ({holder})"))
}

impl From<Cancelled> for InitError {
    // Hand-written rather than a `#[from]`, which would have to carry the marker as a field of a unit variant. The
    // marker says nothing a caller could read, and carrying it would change the variant's public shape for no one's
    // benefit.
    fn from(_: Cancelled) -> Self {
        Self::Cancelled
    }
}

/// Everything that can stop a named model artifact from becoming a session that can run it.
///
/// Two outcomes: a model that could not be **put on disk**, and one that is on disk and could not be **opened**. A
/// missing or unreadable model file is the first of the two rather than the second — it fails the same way whatever
/// provider is attached, which is why the fallback never retries it on the CPU.
#[derive(Debug, Clone, Error)]
pub(crate) enum SessionError {
    // Not variants on `InitError`. That type is `pub` and documented as what initialization reports; a session failure
    // is not an initialization failure, and this one folds into `InferenceError` rather than a single enum accreting
    // every layer's failures.
    //
    // Both sources are behind an `Arc`, which is what makes this `Clone`. The cache serves several concurrent requests
    // for one model from a single build, so one failure has to reach every request that was waiting on it — and
    // neither `std::io::Error` nor `ort::Error` is `Clone`. The `From<InitError>` below is written out rather than
    // derived through `#[from]` for the same reason.
    /// The model's files could not be put on disk, or what was put there could not be read back.
    #[error(transparent)]
    Install(Arc<InitError>),

    // Also what the CPU fallback matches on: this is the failure a session built with nothing attached might not
    // have.
    /// The model is on disk and the runtime would not open it, naming the provider that was being attached.
    #[error("failed to open the model {artifact} on {provider}: {source}")]
    Build {
        /// The artifact that could not be opened.
        artifact: String,
        /// The execution provider the session was being built on.
        provider: ExecutionProvider,
        // Erased rather than typed as one, and not for taste: every `ort::Error` constructor goes through the
        // runtime's `CreateStatus`, so one cannot be built at all without a loaded runtime — and the fallback this
        // variant drives is required to be exercisable on a runner that has none. Nothing downcasts it; what a caller
        // reads is the message and the chain, both of which survive.
        /// What the builder reported, which in production is always an [`ort::Error`].
        #[source]
        source: Arc<dyn std::error::Error + Send + Sync>,
    },
}

impl SessionError {
    /// The error's stable name: [`InitError::kind`] for a failed install, which is its root cause.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Install(source) => source.kind(),
            Self::Build { .. } => "session_build",
        }
    }
}

impl From<InitError> for SessionError {
    fn from(error: InitError) -> Self {
        Self::Install(Arc::new(error))
    }
}

impl From<Cancelled> for SessionError {
    /// The asynchronous runtime shut down before a build handed to a blocking thread could run.
    ///
    /// Carried through the install arm, as [`InitError::Cancelled`].
    fn from(cancelled: Cancelled) -> Self {
        // Rather than as a variant of its own: it is `InitError::Cancelled`'s own answer — nothing is broken and the
        // process is on its way down — and a session failure adds nothing to it.
        Self::from(InitError::from(cancelled))
    }
}

/// Why an operation has no pipeline.
///
/// Every family this crate publishes has a pipeline for every variant it publishes, so the one case left is *"this
/// model's declaration is incomplete"*: a bug in this crate, not a feature gap, and unreachable from any shipped
/// variant. Matched rather than string-compared, so the wording is not a compatibility surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum UnsupportedReason {
    // Written as a refusal rather than a panic so that a variant added without a graph is a failed run rather than a
    // crash.
    /// A diffusion variant does not declare a graph for one of the roles its pipeline addresses.
    ///
    /// Unreachable while every shipped diffusion variant declares all three.
    #[error("this variant does not declare its {missing} graph")]
    IncompleteGraphSet {
        /// The role with no graph behind it.
        missing: &'static str,
    },
}

/// Everything that can stop a run from producing an enhanced image.
///
/// The public failure of [`Opai::process`](crate::Opai::process).
///
/// The distinctions it keeps are the ones a front end acts on. A model that could not be **put on disk** is retried;
/// one that is on disk and would not **open** is a broken install; one that failed while **running** names the tile it
/// failed on. And a run the caller **cancelled** is separate from one that failed, so nobody is told their enhancement
/// broke when they stopped it themselves.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum InferenceError {
    // The one type the three layers below `Opai::process` fold into: the session layer's two outcomes, the tiled
    // driver's failures, and the runtime shutting down under a blocking thread.
    //
    // `Clone` because every source it carries is already behind an `Arc` — which a cache single-flighting a run would
    // want, handing one failure to every waiter exactly as the session cache does.
    /// No pipeline exists for this operation, so the run is refused rather than served wrongly.
    ///
    /// Named, so a front end can say *which* enhancement it cannot run — see [`Opai::process`](crate::Opai::process)
    /// for which are refused.
    #[error("no pipeline for {operation}: {reason}")]
    Unsupported {
        /// The operation that cannot be served, as a user would see it named.
        operation: String,
        /// Why it cannot be served, which is what lets a front end tell one refusal apart from another rather than
        /// reporting them all as "not supported".
        reason: UnsupportedReason,
    },

    /// The operation's model could not be put on disk, or what was put there could not be read back.
    #[error(transparent)]
    Install(Arc<InitError>),

    /// The model is on disk and the runtime would not open it, naming the provider that was being attached.
    #[error("failed to open the model {artifact} on {provider}: {source}")]
    Open {
        /// The artifact that could not be opened.
        artifact: String,
        /// The execution provider the session was being built on.
        provider: ExecutionProvider,
        /// What the builder reported, which in production is always an [`ort::Error`].
        #[source]
        source: Arc<dyn std::error::Error + Send + Sync>,
    },

    // The index is carried for the reason the driver carries it: a failure on the first tile of an image and a
    // failure on its four hundredth are different reports — one is a model that cannot run at all, the other is
    // something about that region.
    /// The model failed while running, on the tile named.
    #[error("{operation} failed on tile {tile}: {source}")]
    Run {
        /// The operation whose model failed, as a user would see it named.
        operation: String,
        /// Which tile it was, counting from zero in row-major order.
        tile: usize,
        // Erased rather than typed as one for the reason `SessionError::Build` erases its source: an `ort::Error`
        // cannot be constructed without a loaded runtime, and the whole of the arithmetic above this is required to be
        // exercisable on a runner that has none. Nothing downcasts it; the message and the chain both survive.
        /// What the run reported, which in production is always an [`ort::Error`].
        #[source]
        source: Arc<dyn std::error::Error + Send + Sync>,
    },

    // The name is tiling-flavoured and stays: it is public, and renaming it buys nothing the wording does not.
    // `imaging::TilingError::Untileable` is the one that really is about partitioning, and keeps its own message.
    /// The image has no area, so there is nothing to run.
    ///
    /// Produced at three depths and worded for all of them. The tiled path raises it because there is no partitioning
    /// of a zero-area image; face recovery and detection raise it because there is no face to restore and nothing to
    /// letterbox.
    #[error("a {width}x{height} image has no pixels to run")]
    Untileable {
        /// The width that was asked for.
        width: u32,
        /// The height that was asked for.
        height: u32,
    },

    // Raised rather than rendered. The fit's ridge makes its system positive-definite, so after partial pivoting a
    // pivot should never be zero — and *should* is not a guarantee in `f64`. Dividing by one fills every weight with
    // NaN, and evaluating those over a photograph produces a destroyed picture with nothing anywhere to say why; a
    // caller handed that has no way to tell it from a correction it dislikes.
    //
    // Not `Run`, which is the other thing a failure inside a pipeline could have been folded into: that variant
    // carries a tile index and an erased source from the runtime, and a fit that failed had no runtime involvement at
    // all.
    //
    // The column is carried because the fit has it in hand at the point the refusal is made, and because *which*
    // column went degenerate is the whole of what a report could act on.
    /// The colour mapping a run fits could not be solved, so there is no correction to apply.
    #[error("{operation} could not fit a colour mapping: the system is singular at column {column}")]
    ColourMapping {
        /// The operation whose fit failed, as a user would see it named.
        operation: String,
        /// Which column of the elimination had no pivot, counting from zero.
        column: usize,
    },

    // A refusal rather than a plain resample presented as a successful enhancement, which is what running zero passes
    // and then correcting the overshoot would produce.
    /// No sequence of native passes covers the requested scale, so there is nothing to run.
    #[error("no sequence of passes covers {scale}x for {operation}")]
    NoPasses {
        /// The operation that could not be covered, as a user would see it named.
        operation: String,
        /// The scale that was asked for.
        scale: f64,
    },

    /// The caller asked the run to stop, and it stopped between tiles.
    ///
    /// **Not [`Shutdown`](Self::Shutdown).** One is somebody closing a dialog and the other is a process on its way
    /// down; a front end that conflated them would tell a user their enhancement failed when they had just cancelled
    /// it themselves.
    #[error("the run was cancelled before it finished")]
    Cancelled,

    /// The async runtime shut down before the blocking work could run.
    #[error("the run was abandoned: the runtime shut down before the work could run")]
    Shutdown,
}

impl From<SessionError> for InferenceError {
    /// Folds a session failure in, keeping the session layer's distinction: one is retried, the other is a broken
    /// install.
    fn from(error: SessionError) -> Self {
        match error {
            // The runtime shutting down reaches here as `SessionError::Install(InitError::Cancelled)`, because that
            // is the install layer's own answer for it. It is unfolded rather than reported as a failed install: a
            // process on its way down did not break anything on disk, and this enum has a variant that says so.
            SessionError::Install(source) if matches!(*source, InitError::Cancelled) => Self::Shutdown,
            // A transfer stopped because every request waiting on it withdrew. Unfolded for the same reason and to
            // the other of the two: nothing is broken, and the caller asked for this — so it is the stop this enum
            // already distinguishes from a failure, not an install that went wrong.
            SessionError::Install(source) if matches!(*source, InitError::Stopped) => Self::Cancelled,
            SessionError::Install(source) => Self::Install(source),
            SessionError::Build { artifact, provider, source } => Self::Open { artifact, provider, source },
        }
    }
}

impl From<Cancelled> for InferenceError {
    fn from(_: Cancelled) -> Self {
        Self::Shutdown
    }
}

impl InferenceError {
    /// The error's stable name, which a failure is counted by: one snake-case name per variant, never its text.
    ///
    /// A failed install names its own root cause, and a model that would not open is `session_build`, as it was named
    /// in the session layer it was folded from.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Unsupported { .. } => "unsupported",
            Self::Install(source) => source.kind(),
            Self::Open { .. } => "session_build",
            Self::Run { .. } => "run",
            Self::Untileable { .. } => "untileable",
            Self::ColourMapping { .. } => "colour_mapping",
            Self::NoPasses { .. } => "no_passes",
            Self::Cancelled => "cancelled",
            Self::Shutdown => "shutdown",
        }
    }

    /// Why this ended the run, when the run was stopped rather than failed.
    ///
    /// `"cancelled"` for [`Cancelled`](Self::Cancelled) and `"shutdown"` for [`Shutdown`](Self::Shutdown); `None` for
    /// every real failure. Crate-private: it is what a record says, and a front end already matches on the variants.
    pub(crate) fn stop_reason(&self) -> Option<&'static str> {
        match self {
            Self::Cancelled => Some("cancelled"),
            Self::Shutdown => Some("shutdown"),
            _ => None,
        }
    }

    /// Folds a tiled run's failure in, naming the operation it was carrying out.
    pub(crate) fn from_tiling<E>(operation: &str, error: imaging::TilingError<E>) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        // Not a `From` implementation, because the driver is generic in the per-tile function's own error and names no
        // operation: which enhancement was running is the chain's to say. Generic in that error for the same reason
        // the driver is — production instantiates it at `ort::Error` and the tests at their own, which is what keeps
        // the mapping exercisable on a runner with no runtime.
        use imaging::TilingError;

        match error {
            TilingError::Untileable { width, height } => Self::Untileable { width, height },
            TilingError::Cancelled => Self::Cancelled,
            TilingError::Tile { index, source } => Self::run(operation, index)(source),
        }
    }

    /// Builds the [`InferenceError::Run`] for `operation` failing on `tile`.
    ///
    /// A curried constructor, so every site reads `.map_err(InferenceError::run(&self.name, 0))`. A pipeline that
    /// runs its graph once passes tile zero, so its failure reads in a log exactly like a tiled one.
    pub(crate) fn run<E>(operation: &str, tile: usize) -> impl FnOnce(E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        let operation = operation.to_string();

        move |source| Self::Run { operation, tile, source: Arc::new(source) }
    }
}

impl InitError {
    /// The error's stable name, which a failure is counted by: one snake-case name per variant, never its text.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::InvalidName { .. } => "invalid_name",
            Self::InvalidApp { .. } => "invalid_app",
            Self::AlreadyRunning { .. } => "already_running",
            Self::UnsupportedPlatform { .. } => "unsupported_platform",
            Self::UnpublishedModel { .. } => "unpublished_model",
            Self::UnknownProvider { .. } => "unknown_provider",
            Self::HashMismatch { .. } => "checksum",
            Self::Download { .. } => "download",
            Self::RuntimeLoad { .. } => "runtime_load",
            Self::RuntimeStart { .. } => "runtime_start",
            Self::RuntimeUnavailable { .. } => "runtime_unavailable",
            Self::Fs(_) => "fs",
            Self::Io { .. } => "io",
            Self::Cancelled => "cancelled",
            Self::Stopped => "stopped",
        }
    }

    /// Why this ended the work it came from, when that work was stopped rather than failed.
    ///
    /// `"cancelled"` for [`Stopped`](Self::Stopped) and `"shutdown"` for [`Cancelled`](Self::Cancelled) — the names
    /// are the reason a record gives, and they read the way [`InferenceError::stop_reason`] reads them one layer up.
    /// `None` for every real failure.
    pub(crate) fn stop_reason(&self) -> Option<&'static str> {
        match self {
            Self::Stopped => Some("cancelled"),
            Self::Cancelled => Some("shutdown"),
            _ => None,
        }
    }

    /// Builds the [`InitError::Io`] for a failure against `path`.
    ///
    /// A curried constructor, so every site reads `.map_err(InitError::io(&path))`.
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Self {
        // Rather than a closure at each site that has to name the source and clone the path itself. It exists because
        // there is no `#[from]` to reach for: each of these failures knows what it was working on, and this is what
        // makes saying so one call rather than four lines.
        |source| Self::Io { path: path.into(), source }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn a_refusal_names_the_holder_where_one_was_read() {
        let error = InitError::AlreadyRunning { holder: Some(Holder { app: crate::GUI.to_string(), pid: 4821 }) };

        // Printed unchanged by a terminal binary, so the whole sentence is pinned rather than a fragment of it.
        assert_eq!(error.to_string(), "Open Photo AI is already running (gui, pid 4821); close it and try again");
    }

    #[test]
    fn a_rejected_identity_is_quoted_back_with_the_rule_it_broke() {
        let error = InitError::InvalidApp { app: "my app".to_string(), reason: "it contains whitespace" };

        // Quoted, because the thing that was wrong with it is a character a user cannot see in an unquoted message.
        assert_eq!(error.to_string(), r#"application identity "my app" cannot be recorded: it contains whitespace"#);
    }

    #[test]
    fn a_refusal_with_no_readable_holder_still_reads_as_a_refusal() {
        let error = InitError::AlreadyRunning { holder: None };

        // Still a refusal, and still a complete sentence: what is missing is the holder, not the answer. No dangling
        // parenthesis and no "unknown" standing in for a name.
        assert_eq!(error.to_string(), "Open Photo AI is already running; close it and try again");
    }

    /// The library path every runtime error below is built against.
    const LIBRARY: &str = "/somewhere/opai/runtime/onnxruntime.1.26.0.dylib";

    /// An [`ort::LoadDynamicError`] built directly rather than by asking `ort` to load something.
    ///
    /// `ort`'s load global is process-wide and a process gets one first attempt at it, so no unit test may touch
    /// `ort::init_from`. The variants are public with public fields, which is what makes a real error of the right
    /// type constructible here without loading anything.
    fn load_error() -> ort::LoadDynamicError {
        ort::LoadDynamicError::MissingApi { path: PathBuf::from(LIBRARY) }
    }

    #[test]
    fn a_load_failure_names_the_library_it_was_attempted_against() {
        let error = InitError::RuntimeLoad { path: PathBuf::from(LIBRARY), source: load_error() };

        assert!(error.to_string().contains(LIBRARY), "the message did not name the library: {error}");
        // The chain reaches `ort`, so a front end that walks it sees what the runtime actually said — which for
        // `BadVersion` is the version string found on disk.
        assert!(error.source().is_some(), "the underlying ort error is not reachable from the chain");
    }

    #[test]
    fn a_retry_after_a_failed_load_names_the_path_and_what_the_first_attempt_reported() {
        // `RuntimeStart`'s own `Display` is asserted in the live test instead: every `ort::Error` constructor goes
        // through `CreateStatus`, so one cannot be built in a process with no runtime loaded.
        let error = InitError::RuntimeUnavailable { path: PathBuf::from(LIBRARY), reason: "dlopen failed".to_string() };

        let message = error.to_string();
        assert!(message.contains(LIBRARY), "the message did not name the library: {message}");
        assert!(message.contains("dlopen failed"), "the message did not carry the first failure: {message}");
    }

    #[test]
    fn an_io_failure_names_the_file_it_was_working_on() {
        // `std::io::Error` carries no path: on its own the message is `permission denied (os error 13)`, which tells
        // a user nothing about what could not be written.
        let path = PathBuf::from("/somewhere/opai/runtime/.manifest.json");
        let error =
            InitError::Io { path: path.clone(), source: std::io::Error::from(std::io::ErrorKind::PermissionDenied) };

        let message = error.to_string();
        assert!(
            message.contains("/somewhere/opai/runtime/.manifest.json"),
            "the message did not name the file: {message}"
        );
        // And the chain still reaches the OS error, so a front end that walks it sees what the OS actually said.
        let source = error.source().expect("the underlying io error is not reachable from the chain");
        let io = source
            .downcast_ref::<std::io::Error>()
            .expect("the source is not the io::Error it was built from");
        assert_eq!(io.kind(), std::io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn a_cancelled_initialization_is_told_apart_from_a_disk_failure() {
        // A shutting-down runtime is not a broken install: one is reported and retried, the other is the process on
        // its way down.
        let cancelled = InitError::Cancelled;
        let io = InitError::Io {
            path: PathBuf::from("/somewhere/opai/runtime"),
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        };

        assert!(matches!(cancelled, InitError::Cancelled));
        assert!(matches!(io, InitError::Io { .. }));
        assert_ne!(cancelled.to_string(), io.to_string());
    }

    #[test]
    fn the_runtime_failures_are_told_apart_rather_than_flattened() {
        // A load failure is recoverable by reinstalling; a process that has already failed one is not recoverable at
        // all until it restarts. A front end says different things, so the variants are different.
        let load = InitError::RuntimeLoad { path: PathBuf::from(LIBRARY), source: load_error() };
        let unavailable =
            InitError::RuntimeUnavailable { path: PathBuf::from(LIBRARY), reason: "dlopen failed".to_string() };

        assert!(matches!(load, InitError::RuntimeLoad { .. }));
        assert!(matches!(unavailable, InitError::RuntimeUnavailable { .. }));
        assert_ne!(load.to_string(), unavailable.to_string());
    }

    #[test]
    fn a_model_that_would_not_open_is_reported_naming_the_artifact_and_the_provider_being_attached() {
        // Both are in the message rather than only in the fields, because this is the one thing a user whose GPU is
        // broken can be told: which model declined, and on what. Nothing logs it, so the error string is where it is.
        let error = SessionError::Build {
            artifact: "up_kyoto_4x_fp16".to_string(),
            provider: ExecutionProvider::Cuda,
            source: Arc::new(std::io::Error::other("the provider failed to initialize")),
        };

        let message = error.to_string();
        assert!(message.contains("up_kyoto_4x_fp16"), "the message did not name the artifact: {message}");
        assert!(message.contains("CUDA"), "the message did not name the provider being attached: {message}");
        assert!(message.contains("failed to initialize"), "the message lost the builder's own text: {message}");
        assert!(error.source().is_some(), "the failure was not chained to what the builder reported");
    }

    #[test]
    fn a_model_that_could_not_be_put_on_disk_is_told_apart_from_one_that_would_not_open() {
        // What the CPU fallback decides on: an install failure fails the same way with nothing attached, so it is
        // never retried there.
        let install = SessionError::from(InitError::UnpublishedModel { artifact: "up_kyoto_4x_fp16".to_string() });
        let build = SessionError::Build {
            artifact: "up_kyoto_4x_fp16".to_string(),
            provider: ExecutionProvider::TensorRt,
            source: Arc::new(std::io::Error::other("the engine build failed")),
        };

        assert!(matches!(install, SessionError::Install(_)));
        assert!(matches!(build, SessionError::Build { .. }));
        // `transparent`, so an install failure reads exactly as the install reported it rather than being wrapped in
        // a second sentence about sessions.
        assert_eq!(
            install.to_string(),
            InitError::UnpublishedModel { artifact: "up_kyoto_4x_fp16".to_string() }.to_string()
        );
    }

    /// A session failure that could not open a model on disk, which is the half that is not an install.
    fn build_failure() -> SessionError {
        SessionError::Build {
            artifact: "up_kyoto_4x_fp16".to_string(),
            provider: ExecutionProvider::CoreMl,
            source: Arc::new(std::io::Error::other("the graph would not load")),
        }
    }

    #[test]
    fn a_cancelled_run_and_a_shutdown_runtime_are_different_failures() {
        // One is somebody closing a dialog and the other is a process on its way down. A front end that conflated
        // them would tell a user their enhancement failed when they had just cancelled it themselves.
        let cancelled = InferenceError::Cancelled;
        let shutdown = InferenceError::from(Cancelled);

        assert!(matches!(cancelled, InferenceError::Cancelled));
        assert!(matches!(shutdown, InferenceError::Shutdown));
        assert_ne!(cancelled.to_string(), shutdown.to_string());
    }

    #[test]
    fn a_model_that_could_not_be_installed_and_one_that_would_not_open_land_on_different_variants() {
        // The session layer's distinction, surviving to the public surface: one is retried, the other is a broken
        // install, and the two are reported to a user differently.
        let install = InferenceError::from(SessionError::from(InitError::UnpublishedModel {
            artifact: "up_kyoto_4x_fp16".to_string(),
        }));
        let open = InferenceError::from(build_failure());

        assert!(matches!(install, InferenceError::Install(_)), "an install failure landed on {install:?}");
        assert!(matches!(open, InferenceError::Open { .. }), "an open failure landed on {open:?}");
    }

    #[test]
    fn a_model_that_would_not_open_carries_the_provider_and_the_artifact_to_the_surface() {
        let InferenceError::Open { artifact, provider, source } = InferenceError::from(build_failure()) else {
            panic!("a build failure did not fold into Open");
        };

        assert_eq!(artifact, "up_kyoto_4x_fp16");
        assert_eq!(provider, ExecutionProvider::CoreMl);
        assert!(source.to_string().contains("the graph would not load"), "the source was replaced: {source}");
    }

    #[test]
    fn a_runtime_shutting_down_under_the_install_is_a_shutdown_rather_than_a_broken_install() {
        // It reaches the session layer as an install failure, because that is that layer's own answer for it.
        let error = InferenceError::from(SessionError::from(Cancelled));

        assert!(matches!(error, InferenceError::Shutdown), "a shutting-down runtime landed on {error:?}");
    }

    #[test]
    fn a_failed_tile_reaches_the_surface_naming_the_operation_and_the_tile() {
        let error = InferenceError::from_tiling(
            "Kyoto 4x (FP16)",
            imaging::TilingError::Tile { index: 37, source: std::io::Error::other("the model failed") },
        );

        let InferenceError::Run { operation, tile, source } = error else {
            panic!("a tile failure did not fold into Run");
        };

        assert_eq!(operation, "Kyoto 4x (FP16)");
        assert_eq!(tile, 37);
        assert!(source.to_string().contains("the model failed"));
    }

    #[test]
    fn the_drivers_own_two_refusals_keep_their_meaning_at_the_surface() {
        let untileable = InferenceError::from_tiling::<std::io::Error>(
            "Kyoto 4x (FP16)",
            imaging::TilingError::Untileable { width: 0, height: 480 },
        );
        let cancelled =
            InferenceError::from_tiling::<std::io::Error>("Kyoto 4x (FP16)", imaging::TilingError::Cancelled);

        assert!(matches!(untileable, InferenceError::Untileable { width: 0, height: 480 }));
        // Emphatically not `Shutdown`: the driver's cancellation is the caller's own.
        assert!(matches!(cancelled, InferenceError::Cancelled));
    }

    #[test]
    fn only_the_two_stops_of_an_initialization_have_a_stop_reason() {
        assert_eq!(InitError::Stopped.stop_reason(), Some("cancelled"));
        assert_eq!(InitError::Cancelled.stop_reason(), Some("shutdown"));

        let failures = [
            InitError::InvalidName { name: "a/b".to_string(), reason: "it contains a separator" },
            InitError::InvalidApp { app: "my app".to_string(), reason: "it contains whitespace" },
            InitError::AlreadyRunning { holder: None },
            InitError::UnsupportedPlatform { dependency: "runtime", os: "plan9", arch: "mips" },
            InitError::UnpublishedModel { artifact: "up_kyoto_4x_fp16".to_string() },
            InitError::UnknownProvider { name: "gpu".to_string() },
            InitError::HashMismatch { url: "u".to_string(), expected: "a".to_string(), actual: "b".to_string() },
            InitError::RuntimeLoad { path: PathBuf::from(LIBRARY), source: load_error() },
            InitError::RuntimeUnavailable { path: PathBuf::from(LIBRARY), reason: "dlopen failed".to_string() },
            InitError::Io { path: PathBuf::from(LIBRARY), source: std::io::Error::other("disk") },
        ];
        // `Download`, `Fs` and `RuntimeStart` are left out only because their sources cannot be built here; they take
        // the same wildcard arm as every failure above.
        for failure in failures {
            assert_eq!(failure.stop_reason(), None, "{failure:?} was read as a stop");
        }
    }

    #[test]
    fn every_initialization_failure_is_counted_by_a_pinned_name() {
        let path = || PathBuf::from(LIBRARY);
        let named = [
            (InitError::InvalidName { name: "a/b".to_string(), reason: "r" }, "invalid_name"),
            (InitError::InvalidApp { app: "my app".to_string(), reason: "r" }, "invalid_app"),
            (InitError::AlreadyRunning { holder: None }, "already_running"),
            (
                InitError::UnsupportedPlatform { dependency: "runtime", os: "plan9", arch: "mips" },
                "unsupported_platform",
            ),
            (InitError::UnpublishedModel { artifact: "a".to_string() }, "unpublished_model"),
            (InitError::UnknownProvider { name: "gpu".to_string() }, "unknown_provider"),
            (
                InitError::HashMismatch { url: "u".to_string(), expected: "a".to_string(), actual: "b".to_string() },
                "checksum",
            ),
            (InitError::Download { url: "u".to_string(), source: DownloadError::Cancelled }, "download"),
            (InitError::RuntimeLoad { path: path(), source: load_error() }, "runtime_load"),
            (InitError::RuntimeUnavailable { path: path(), reason: "r".to_string() }, "runtime_unavailable"),
            (InitError::Fs(FsError::NoConfigDir), "fs"),
            (InitError::Io { path: path(), source: std::io::Error::other("disk") }, "io"),
            (InitError::Cancelled, "cancelled"),
            (InitError::Stopped, "stopped"),
        ];
        // `RuntimeStart` is left out only because an `ort::Error` cannot be built without a runtime. The match in
        // `kind` names it `runtime_start`, and is exhaustive, so no variant goes without a name.
        for (error, kind) in named {
            assert_eq!(error.kind(), kind, "{error:?}");
        }
    }

    #[test]
    fn a_wrapped_failure_is_counted_by_its_root_cause() {
        let install =
            || Arc::new(InitError::HashMismatch { url: "u".into(), expected: "a".into(), actual: "b".into() });

        assert_eq!(SessionError::Install(install()).kind(), "checksum");
        assert_eq!(SessionError::from(InitError::Cancelled).kind(), "cancelled");
        assert_eq!(build_failure().kind(), "session_build");

        assert_eq!(InferenceError::Install(install()).kind(), "checksum");
        assert_eq!(InferenceError::from(build_failure()).kind(), "session_build");
    }

    #[test]
    fn every_run_failure_is_counted_by_a_pinned_name() {
        let named = [
            (
                InferenceError::Unsupported {
                    operation: "Kyoto".to_string(),
                    reason: UnsupportedReason::IncompleteGraphSet { missing: "vae" },
                },
                "unsupported",
            ),
            (InferenceError::from(build_failure()), "session_build"),
            (InferenceError::run("Kyoto", 3)(std::io::Error::other("the model failed")), "run"),
            (InferenceError::Untileable { width: 0, height: 480 }, "untileable"),
            (InferenceError::ColourMapping { operation: "Rio".to_string(), column: 2 }, "colour_mapping"),
            (InferenceError::NoPasses { operation: "Kyoto".to_string(), scale: 3.0 }, "no_passes"),
            (InferenceError::Cancelled, "cancelled"),
            (InferenceError::Shutdown, "shutdown"),
        ];
        for (error, kind) in named {
            assert_eq!(error.kind(), kind, "{error:?}");
        }
    }

    #[test]
    fn only_the_two_stops_of_a_run_have_a_stop_reason() {
        assert_eq!(InferenceError::Cancelled.stop_reason(), Some("cancelled"));
        assert_eq!(InferenceError::Shutdown.stop_reason(), Some("shutdown"));

        let failures = [
            InferenceError::Unsupported {
                operation: "Kyoto".to_string(),
                reason: UnsupportedReason::IncompleteGraphSet { missing: "vae" },
            },
            InferenceError::Install(Arc::new(InitError::UnpublishedModel { artifact: "a".to_string() })),
            InferenceError::from(build_failure()),
            InferenceError::run("Kyoto", 3)(std::io::Error::other("the model failed")),
            InferenceError::Untileable { width: 0, height: 480 },
            InferenceError::ColourMapping { operation: "Rio".to_string(), column: 2 },
            InferenceError::NoPasses { operation: "Kyoto".to_string(), scale: 3.0 },
        ];
        for failure in failures {
            assert_eq!(failure.stop_reason(), None, "{failure:?} was read as a stop");
        }
    }
}
