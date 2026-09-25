//! The install pipeline: fetch a dependency's archive, prove it is the published one, expand it, and record what it
//! wrote.

// The ordering is the load-bearing part. The previous record is deleted *before* the first byte of a new install is
// written and the new one is written *after* every file is in place, so an interruption anywhere in between leaves a
// directory that is explicitly mid-install. That is also why there is no staging directory — it would double peak disk
// for a multi-gigabyte archive to close a window this ordering already closes.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rust_sak::crypto::sha256_file;
use rust_sak::fetch::{DigestAlgorithm, DownloadMode, Fetch, RequestOptions};
use rust_sak::fs::{self, ExtractOptions, ExtractProgress};
use tokio_util::sync::CancellationToken;
use tracing::Instrument as _;

use crate::deps::ModelTrust;
use crate::deps::manifest::{self, Manifest};
use crate::deps::release::{Contents, Dependency, Source};
use crate::error::InitError;
use crate::progress::Reporter;
use crate::task::spawn_blocking;
use crate::telemetry::unit::{self, Unit, unit_span};

/// How many times a *failed* transfer is retried before the install gives up. The backoff between attempts is
/// `rust-sak`'s, and each attempt resumes from whatever is already on disk rather than restarting.
pub(crate) const TRANSFER_RETRIES: u32 = 3;

/// Bound on establishing a connection, so a black-holed host cannot hold initialization open forever.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

// A legitimate archive is a couple of hundred megabytes over whatever link the user has, so any bound on the transfer
// as a whole would be a bound on how large a dependency the app can install.
/// Bound on the silence between reads, not on the transfer. The timer resets on every successful read.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

// Returned rather than reported from inside the pipeline, because whether a skipped install is worth saying anything
// about is not a fact about *installing* — it is a fact about what is being installed, and only the two callers know
// which they have. A dependency's install says so, through the reporter the caller already holds; a model's does not.
/// What an install turned out to have to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// The install ran: the files were transferred, expanded where the dependency is an archive, and recorded.
    Installed,
    /// Nothing was done, because the recorded install was already the pinned one and every file it named was still
    /// there. Nothing was downloaded, nothing was extracted and nothing was re-hashed.
    AlreadyCurrent,
}

/// Brings a dependency on disk up to date and records what it put there.
///
/// `dir` is the dependency's own directory and must already exist. It is assumed to be exclusive: extraction produces
/// a file list nobody declared, and "everything under the directory" is only a correct answer to "what did this
/// install?" when nothing else writes there.
///
/// Returns which of the two it turned out to be — an install that ran, or one that was already current — which is
/// the caller's to act on: see [`Outcome`].
///
/// `cancel` stops the **transfer**, which is the part of an install that takes minutes; the bookkeeping either side
/// of it is milliseconds and runs to completion. A stopped transfer leaves its partial on disk, so the next install
/// of the same dependency resumes onto it.
///
/// # Errors
///
/// Returns [`InitError::Download`] if the transfer fails, [`InitError::HashMismatch`] if the bytes are not the
/// published ones, [`InitError::Fs`] if the archive cannot be expanded — including an entry refused for trying to
/// write outside `dir` — and [`InitError::Io`], naming the file it was against, for the bookkeeping around them.
/// [`InitError::Cancelled`] if the asynchronous runtime shut down before the blocking half of any of that could run,
/// and [`InitError::Stopped`] if `cancel` was cancelled while bytes were moving.
pub(crate) async fn install(
    dir: &Path,
    dependency: &Dependency,
    reporter: Arc<Reporter>,
    cancel: &CancellationToken,
) -> Result<Outcome, InitError> {
    let span = unit_span!("install", dependency = dependency.name, version = dependency.version);

    // Wrapped so that the failure record is written once, here, whichever of the half-dozen `?`s below produced it.
    // Started before the body, so a stop or a failure says how long the install had run.
    let started = Instant::now();
    let outcome = install_inner(dir, dependency, reporter, cancel).instrument(span.clone()).await;
    let duration = started.elapsed();

    match &outcome {
        // Its record is written inside the body, which knows whether anything had to move.
        Ok(_) => unit::ended(Unit::Install, &span, duration, unit::Outcome::Finished),
        // Inside the span, so the record is in the install's trace, and ended the way the record says it ended.
        Err(error) => span.in_scope(|| {
            record_ended(dependency, duration, error);

            let ending = match error.stop_reason() {
                Some(_) => unit::Outcome::Stopped,
                None => unit::Outcome::Failed { kind: error.kind(), error },
            };
            unit::ended(Unit::Install, &span, duration, ending);
        }),
    }

    outcome
}

/// Records how an install that did not finish ended: stopped, or failed.
fn record_ended(dependency: &Dependency, duration: Duration, error: &InitError) {
    // A stop is not a failure and is recorded as what it is: nobody is left waiting, or the process is on its way
    // down, and a `warn` here would put an install failure in the file of every user who changed their mind. `info`,
    // because the install opened at `info` and a bracket closes at the level it opened.
    if let Some(reason) = error.stop_reason() {
        tracing::info!(
            dependency = dependency.name,
            version = dependency.version,
            reason,
            ?duration,
            "an install stopped"
        );

        return;
    }

    // `warn` naming the dependency, which is the fact a bug report about an install that never finishes is missing.
    tracing::warn!(
        dependency = dependency.name,
        version = dependency.version,
        ?duration,
        %error,
        "installing a dependency failed"
    );
}

/// [`install`]'s body, with the failure record left to its caller.
async fn install_inner(
    dir: &Path,
    dependency: &Dependency,
    reporter: Arc<Reporter>,
    cancel: &CancellationToken,
) -> Result<Outcome, InitError> {
    // Whether this install is the trusted one: the process declared it, and **every** file backing the dependency is
    // already on disk. Both halves, because the declaration says "what is here is what I want measured" and not
    // "fetch me whatever is missing and ask no questions" — a partially present model is not something anyone placed
    // by hand, and completing it from the published sources is both what the operator wants and the only safe
    // reading.
    let trusted = dependency.trust == ModelTrust::LocalFiles && sources_present(dir, dependency);

    // The dependency's identity, which is the one value everything below hangs off. On the ordinary path it is the
    // published fingerprint; on the trusted one there is no published hash to compare against, so it is stamped from
    // the files as they sit on disk instead. The rest of this function does not branch again: the same comparison,
    // the same emptying, the same discard of what a provider compiled, the same record.
    let want = if trusted { stamp(dir, dependency) } else { manifest::fingerprint(dependency) };

    // The steady-state path: the recorded identity is the pinned one and every recorded file is still there at its
    // recorded size.
    //
    // On a blocking thread because it is neither cheap nor rare. `intact` is one `symlink_metadata` per recorded file,
    // a CUDA or TensorRT tree is thousands of them, and this runs for every dependency on *every* launch rather than
    // only the first — so on the launch where there is nothing to do, this is the whole of the work. Left inline it
    // would be thousands of blocking syscalls on the async runtime, which in the GUI is the event loop, at exactly the
    // moment the window is coming up.
    //
    // **`intact` is skipped on the trusted path**, and that is required rather than an optimisation. A trusted
    // install records no files (below), and `Manifest::intact` reads an empty list as "not installed" — so leaving it
    // in would report "not current" on every single launch, and the derived-cache discard the stamp exists to gate
    // would fire every time: a multi-minute engine rebuild on every launch. The trusted comparison is therefore the
    // fingerprint alone; the ordinary one keeps both halves.
    let (previous, current) = {
        let dir = dir.to_path_buf();
        let want = want.clone();
        spawn_blocking::<_, InitError, _>(move || {
            let previous = manifest::read(&dir);
            let current = previous.as_ref().is_some_and(|old| old.fingerprint == want && (trusted || old.intact(&dir)));
            (previous, current)
        })
        .await?
    };

    // Every time the path is taken, not once per process, and before the early return below — a model whose
    // verification was skipped is the first thing to suspect in a report of wrong output, and a launch that found the
    // stamp unmoved is exactly the launch where it would otherwise go unsaid. `warn`, so it is in the file a user
    // attaches without being asked to reproduce anything with the logging turned up.
    if trusted {
        tracing::warn!(
            dependency = dependency.name,
            files = dependency.sources.len(),
            path = %dir.display(),
            "every file of this model is already on disk and this process trusts them; verification skipped"
        );
    }

    if current {
        // `debug`: it is what every dependency does on every launch after the first, so at the default level it is
        // four lines per session saying nothing happened.
        tracing::debug!(
            dependency = dependency.name,
            version = dependency.version,
            "the dependency is already present and complete"
        );
        return Ok(Outcome::AlreadyCurrent);
    }

    tracing::info!(
        dependency = dependency.name,
        version = dependency.version,
        sources = dependency.sources.len(),
        path = %dir.display(),
        "installing a dependency"
    );
    let started = Instant::now();

    // From here on an install is certain, and the record goes first: an interruption after this point is always
    // detected as mid-install rather than mistaken for a finished one.
    //
    // The manifest helpers answer in `io::Result` because they are about files and know nothing about this crate's
    // error type; the directory they were working on is named here, where it is in hand.
    manifest::clear(dir).map_err(InitError::io(dir))?;

    // The emptying is what the previous version's files are removed by, and it is exactly what the trusted path must
    // not do: the files it is about to record are the ones already sitting in this directory, and deleting them is
    // deleting the thing the operator put there to be measured.
    if !trusted {
        match &previous {
            // Exactly the files the previous version recorded, so two versions' shared libraries never sit side by
            // side for the loader to choose between.
            Some(old) if !old.files.is_empty() => old.remove(dir).map_err(InitError::io(dir))?,
            // A directory whose record describes no files was populated by something that kept none — either nothing
            // at all, or a **trusted** install, which deliberately records an empty list so that this process does
            // not accept its files. Either way the contents cannot be described, so extracting or downloading over
            // them would merge two versions: the previous install's files would survive as a resume point, and the
            // transfer would continue onto bytes it never wrote. Partial downloads survive the emptying.
            //
            // The same rule `Manifest::intact` already holds — a record naming no files is not an install to trust —
            // read from the other end. Without it, "a later process reinstalls from the published sources" would be
            // true of the request and false of the file that came back.
            _ => manifest::empty_dir(dir).map_err(InitError::io(dir))?,
        }
    }

    // Whatever an execution provider compiled from the version being replaced, discarded here rather than by a later
    // step that is trusted to remember: an interruption between the two would leave new weights beside an engine
    // built from the old ones, which is the one pairing that must never exist. Reached only on the path above — an
    // install that is already current returned before it, so it discards nothing, which is the point when a rebuild
    // costs minutes.
    //
    // A directory that is not there is nothing to empty rather than a failure: only the first install of a model
    // creates one, and this runs before that model has ever been built.
    for derived in &dependency.derived {
        if derived.is_dir() {
            manifest::empty_dir(derived).map_err(InitError::io(derived))?;
        }
    }

    // Nothing is transferred for a trusted model and no progress is reported for it: every file it needs is the file
    // already on disk, which is the whole of what the declaration asked for.
    for source in if trusted { &[][..] } else { &dependency.sources } {
        let file = acquire(dir, source, &reporter, cancel).await?;

        // The pipeline's one branch, and everything around it is shared: the fingerprint skip above, the resume and
        // the single hash-mismatch retry inside `acquire`, and the bookkeeping below.
        if dependency.contents == Contents::Archive {
            expand(&file, dir, Arc::clone(&reporter)).await?;

            // Before the tree is read back, so the record never names a file this install itself removes.
            std::fs::remove_file(&file).map_err(InitError::io(&file))?;
        }
    }

    // **A trusted install records no files, and that is what stops trust escaping this process.**
    // `Manifest::intact` reads an empty list as "not installed", so the next process — one that did not declare
    // trust, and whose `current` therefore still asks `intact` — finds the record not current and installs the model
    // from the published sources, verified. Without it, one debugging session would silently downgrade every later
    // session on this machine: unverified weights recorded as though they were the published ones, and every
    // subsequent launch reading that record, finding it current, and running the unpublished model believing it was
    // the published one.
    //
    // The fingerprint written is the stamp, which is what the comparison at the top of this function reads back — so
    // a launch that changed nothing discards nothing, and a file that moved discards the engine compiled from it.
    let files = if trusted { Vec::new() } else { manifest::record_tree(dir).map_err(InitError::io(dir))? };
    let count = files.len();
    manifest::write(dir, &Manifest { fingerprint: want, ..Manifest::new(dependency, files) })
        .map_err(InitError::io(dir))?;

    tracing::info!(
        dependency = dependency.name,
        version = dependency.version,
        files = count,
        duration = ?started.elapsed(),
        "the dependency is ready"
    );

    // Last, and only once every file is in place — including the record itself, so a front end never sees 100% for an
    // install that has not finished.
    reporter.finish();

    Ok(Outcome::Installed)
}

/// Whether every file backing `dependency` is already present in `dir`.
///
/// A directory entry that is not a regular file is not a present source.
fn sources_present(dir: &Path, dependency: &Dependency) -> bool {
    dependency.sources.iter().all(|source| {
        // `symlink_metadata` rather than `metadata`, matching `Manifest::intact`: a link is the file that is there,
        // not an invitation to follow it somewhere this install never described. And a regular file only: a directory
        // named `up_kyoto_4x_fp32.onnx` is not a model, and reading it as one would be trusting something no operator
        // placed.
        std::fs::symlink_metadata(dir.join(source.file_name())).is_ok_and(|meta| meta.is_file())
    })
}

/// The identity of a dependency read off the files themselves: each source's name, size and modification time.
///
/// What stands in for the published fingerprint where there is no published hash to compare against. Editing or
/// replacing a file moves it; touching nothing leaves it where it was.
///
/// A file that cannot be stat'd contributes a marker rather than being skipped, so "one of them vanished between the
/// presence check and here" moves the stamp instead of silently producing the one the complete model had.
fn stamp(dir: &Path, dependency: &Dependency) -> String {
    // The two halves of the contract pull in opposite directions:
    //
    // - Moving on an edit or replacement discards whatever an execution provider compiled from the previous weights,
    //   through the same emptying every other identity change goes through. That matters most in exactly this
    //   workflow, since re-exporting a model is what changes weights most often, and a TensorRT engine reused against
    //   weights it was never built for produces an image nothing reports as wrong.
    // - Staying put when nothing is touched spares an iteration loop a multi-minute engine rebuild on every launch. A
    //   launch that changed nothing has to discard nothing, or the declaration is unusable for the work it exists for.
    //
    // Size and mtime rather than a hash of the bytes. This runs on every acquisition of the model, the weights run to
    // gigabytes, and `symlink_metadata` is the call `Manifest::intact` already makes per file on every launch. A hash
    // would be strictly more precise about a file edited in place within one filesystem timestamp tick — which is not
    // a threat model, since an operator who replaced a file and wants it measured is not trying to defeat their own
    // stamp — at the cost of re-reading every byte of every model on every run.
    let mut input = String::new();

    for source in &dependency.sources {
        // The same separator `manifest::fingerprint` uses, and for the same reason: it cannot occur in a file name or
        // a decimal, so no two different stamps can collide by having their fields run together.
        input.push('\0');
        input.push_str(source.file_name());
        input.push('\0');

        match std::fs::symlink_metadata(dir.join(source.file_name())) {
            Ok(meta) => {
                input.push_str(&meta.len().to_string());
                input.push('\0');

                // Nanoseconds since the epoch where the platform gives them, which is what makes NTFS's 100 ns and
                // APFS's and ext4's coarser resolutions all usable through one spelling. A file whose time cannot be
                // read contributes the marker below.
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or_else(|| "?".to_string(), |since| since.as_nanos().to_string());

                input.push_str(&modified);
            }
            // Not reachable through `sources_present`, which runs first — but a stamp that answered the same thing
            // for "gone" as for "zero bytes" would be one a later caller could reach it through.
            Err(_) => input.push_str("absent"),
        }
    }

    rust_sak::crypto::sha256_string(&input)
}

/// Downloads one source into `dir` and proves it is the published artifact, returning where it landed.
///
/// A mismatch is retried exactly once, as a clean run, and only when there were bytes on disk to resume onto; a
/// transfer that started from nothing and hashed wrong fails immediately.
///
/// # Errors
///
/// Returns [`InitError::Download`] if the transfer fails and [`InitError::HashMismatch`] if the bytes are wrong after
/// that one retry.
async fn acquire(
    dir: &Path,
    source: &Source,
    reporter: &Reporter,
    cancel: &CancellationToken,
) -> Result<PathBuf, InitError> {
    let fetch = Fetch::new()
        .retries(TRANSFER_RETRIES)
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT);
    let path = dir.join(source.file_name());

    // Read before the transfer, because that is the only point at which it is knowable. `rust-sak` decides for itself
    // whether the partial belongs to this request and says nothing about which it chose. Over-reporting a resume
    // costs one extra clean pass on an install that was going to fail anyway; under-reporting would throw away the
    // recovery this exists for.
    let resumable = tokio::fs::try_exists(part_path(&path)).await.unwrap_or(false);

    // The modes this attempt is entitled to, in order. A transfer that started from nothing and still hashed wrong is
    // not going to hash differently the second time, so it gets the one pass; a resume has a stale prefix to blame,
    // so it earns the clean one behind it.
    let modes: &[DownloadMode] = if resumable {
        &[DownloadMode::Resume, DownloadMode::Overwrite]
    } else {
        &[DownloadMode::Resume]
    };

    let mut actual = String::new();

    for &mode in modes {
        // The record the reference's four separate mismatch warnings each say a piece of: this is the one place that
        // knows a partial existed, which pass is running, and therefore *why* the bytes are being transferred again.
        // `warn`, because a user is about to pay for a second transfer of several hundred megabytes.
        if mode != modes[0] {
            tracing::warn!(
                artifact = source.file_name(),
                expected = source.sha256,
                actual,
                "what a resumed transfer produced does not match the published hash; transferring again from nothing"
            );
        }

        actual = transfer(&fetch, source, &path, mode, reporter, cancel).await?;
        if actual == source.sha256 {
            return Ok(path);
        }

        // Known-bad bytes must not survive as a resume point for the next attempt, or for the next run.
        discard(&path).await?;
    }

    Err(mismatch(source, actual))
}

/// Runs one transfer to completion and returns the SHA-256 of what arrived.
///
/// # Errors
///
/// Returns [`InitError::Download`] if the transfer fails, [`InitError::Stopped`] if `cancel` was cancelled while it
/// was running, or [`InitError::Io`] if the fallback hash cannot be read.
async fn transfer(
    fetch: &Fetch,
    source: &Source,
    path: &Path,
    mode: DownloadMode,
    reporter: &Reporter,
    cancel: &CancellationToken,
) -> Result<String, InitError> {
    let options = RequestOptions::new()
        .download_mode(mode)
        // The pinned hash, so a partial recorded against a different release is discarded before a byte moves rather
        // than caught a whole transfer later by the comparison below.
        .resume_key(source.sha256.clone())
        .digest(DigestAlgorithm::Sha256);

    let mut download = fetch.download_with_options(source.url.clone(), path, options);

    // The stop, and it is **not** a matter of dropping this future: `rust-sak` runs the transfer on a task of its
    // own, so a download whose handle goes away goes on writing to the disk with nobody waiting for it. Asking it to
    // stop is the whole difference between a cancel that means cancel and one that only stops the waiting.
    //
    // The tracking future is confined to the block below so that the borrow it takes of `download` has ended by the
    // time the handle is asked to stop.
    let tracked = {
        let tracking = download.track(|total, downloaded, _| reporter.downloading(downloaded, total));
        tokio::pin!(tracking);

        tokio::select! {
            outcome = &mut tracking => Some(outcome),
            () = cancel.cancelled() => None,
        }
    };

    let Some(outcome) = tracked else {
        download.cancel();

        // Awaited rather than left to stop on its own, so that when this returns the transfer really is over: the
        // partial's handle is closed, and the next install of this dependency resumes onto it without two writers
        // ever having been open on one file. What comes back is the cancellation this just asked for.
        let _ = download.join().await;

        return Err(InitError::Stopped);
    };

    outcome.map_err(|err| InitError::Download { url: source.url.clone(), source: err })?;

    match download.digest() {
        Some(digest) => Ok(digest),
        // The transfer short-circuited on a file already at the target path — a previous run killed between the
        // rename and the extraction — so it reported completion without contacting the server and no bytes passed the
        // hasher. Verification stays unconditional: the file on disk is hashed instead.
        None => {
            let failed = path.to_path_buf();
            let path = path.to_path_buf();
            spawn_blocking::<_, InitError, _>(move || sha256_file(&path))
                .await?
                .map_err(InitError::io(failed))
        }
    }
}

/// Expands `archive` into `dir`, reporting the bytes it writes.
///
/// # Errors
///
/// Returns [`InitError::Fs`] if the archive cannot be read or an entry is refused — including one naming a path that
/// would resolve outside `dir`, which `rust-sak` rejects rather than writes.
async fn expand(archive: &Path, dir: &Path, reporter: Arc<Reporter>) -> Result<(), InitError> {
    // Forced out before the expansion starts, so the phase switch is visible immediately rather than after the first
    // coalesced tick — which on a large archive is a noticeable pause with the bar still reading "downloading".
    reporter.extracting(0, None);

    tracing::info!(archive = %archive.display(), path = %dir.display(), "extracting an archive");

    // Whether any tick carried an uncompressed total. An archive whose format does not record one leaves the bar
    // indeterminate for the length of the expansion, and that is worth being able to look up rather than guess at from
    // a user's screenshot.
    //
    // Read back **out here** rather than recorded from inside the callback: the callback runs on the blocking thread,
    // and it would otherwise fire once per tick.
    let sized = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let dir = dir.to_path_buf();

    // `fs::extract` is synchronous and CPU-bound for a minute on a large archive, so it runs on a blocking thread: on
    // the async runtime it would stall every other task in the process, which in the GUI is the whole event loop.
    let outcome = {
        let sized = Arc::clone(&sized);
        let archive = archive.to_path_buf();
        spawn_blocking::<_, InitError, _>(move || {
            let options = ExtractOptions::new().on_progress(move |progress: &ExtractProgress| {
                if progress.total_bytes.is_some() {
                    sized.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                reporter.extracting(progress.bytes, progress.total_bytes);
            });
            fs::extract(&archive, &dir, &options)
        })
        .await?
    };

    if !sized.load(std::sync::atomic::Ordering::Relaxed) {
        // `debug`: it costs an indeterminate progress bar and nothing else, and it is a property of the archive
        // rather than of this machine.
        tracing::debug!(
            archive = %archive.display(),
            "the archive reports no uncompressed size, so extraction progress is indeterminate"
        );
    }

    outcome.map_err(InitError::Fs)?;

    Ok(())
}

/// The mismatch error, carrying both hashes so a caller can see what it got as well as what it wanted.
fn mismatch(source: &Source, actual: String) -> InitError {
    InitError::HashMismatch { url: source.url.clone(), expected: source.sha256.clone(), actual }
}

/// Deletes an artifact and every trace of the transfer that produced it, so nothing is left for a later run to resume
/// onto or to mistake for a finished download.
async fn discard(path: &Path) -> Result<(), InitError> {
    for path in [path.to_path_buf(), part_path(path), sidecar_path(path)] {
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(InitError::Io { path, source }),
        }
    }

    Ok(())
}

/// Where `rust-sak` keeps a transfer's partial bytes: the target path with `.part` appended.
pub(crate) fn part_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".part");
    PathBuf::from(name)
}

/// Where `rust-sak` records what those partial bytes are.
pub(crate) fn sidecar_path(path: &Path) -> PathBuf {
    let mut name = part_path(path).into_os_string();
    name.push(".json");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps::test_server::{self, EXTRACTED, FIXTURE, Reply, TestServer};
    use crate::logging::{self, field, records};
    use crate::progress::{self, Dependency as Which, Expansion, Phase, Progress};
    use rust_sak::crypto::sha256_bytes;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// The file name the fixture is served and stored under.
    const ARCHIVE: &str = "onnx_test.7z";

    /// Stops `cancel` once the transfer has actually written something to `part`.
    fn stop_once_transferring(part: PathBuf, cancel: CancellationToken) -> tokio::task::JoinHandle<()> {
        // Waiting on the server having read the request is not enough, and the difference is the whole reason this is
        // a helper rather than three lines at each call site: the server reading a request says nothing about whether
        // the client has received a byte of the answer, so a stop timed off it lands *before* the first chunk about as
        // often as after it — and what the tests below are about is what a transfer leaves behind when it is stopped
        // mid-flight.
        tokio::spawn(async move {
            for _ in 0..5_000 {
                if std::fs::metadata(&part).is_ok_and(|meta| meta.len() > 0) {
                    cancel.cancel();
                    return;
                }

                tokio::time::sleep(Duration::from_millis(1)).await;
            }

            panic!("the transfer never wrote anything for a stop to leave behind");
        })
    }

    // Comfortably past the 8 KiB the transfer buffers its writes through, so what a stopped transfer leaves on disk is
    // bytes rather than an empty file — which is the difference between a resume that asks for a range and one that
    // silently starts again from nothing.
    /// How much of the fixture a stalled reply serves before it stops.
    const STALL_AFTER: usize = 32 * 1024;

    /// [`super::install`] with nothing to stop it, which is what every test here but the ones about stopping wants.
    async fn install(dir: &Path, dependency: &Dependency, reporter: Arc<Reporter>) -> Result<Outcome, InitError> {
        // Shadowing the function under test rather than threading a never-cancelled token through two dozen call
        // sites: a token no test cancels is noise at every one of them, and the tests that *do* stop an install say so
        // by calling `super::install` with a token of their own.
        super::install(dir, dependency, reporter, &CancellationToken::new()).await
    }

    /// A descriptor pointing at `server` and pinned to the fixture's real hash, unless `hash` overrides it.
    fn descriptor(server: &TestServer, version: &str, hash: Option<&str>) -> Dependency {
        let mut source = test_server::fixture_source(server, version, ARCHIVE);
        if let Some(hash) = hash {
            source.sha256 = hash.to_string();
        }

        Dependency {
            name: "onnx-runtime".to_string(),
            version: version.to_string(),
            dir: "runtime".to_string(),
            progress: Which::Runtime,
            lib: test_server::fixture_lib(&Which::Runtime),
            provides: None,
            // Empty, which is what proves the branch reading it changes nothing for the dependencies that derive
            // nothing.
            derived: Vec::new(),
            sources: vec![source],
            contents: Contents::Archive,
            trust: ModelTrust::Published,
        }
    }

    /// A reporter collecting what it is told, returned with the reports.
    fn recording() -> (Arc<Reporter>, Arc<Mutex<Vec<Progress>>>) {
        let (on_progress, seen) = progress::recording();
        (
            Arc::new(Reporter::new(Which::Runtime, Some(on_progress), Some(FIXTURE.len() as u64), Expansion::Follows)),
            seen,
        )
    }

    /// A reporter that discards its reports, for the tests that are not about progress.
    fn silent() -> Arc<Reporter> {
        Arc::new(Reporter::new(Which::Runtime, None, Some(FIXTURE.len() as u64), Expansion::Follows))
    }

    fn install_dir() -> TempDir {
        tempfile::tempdir().unwrap()
    }

    /// A script that cuts every attempt of one transfer short — the initial request plus each of `rust-sak`'s own
    /// retries — so the install gives up and the partial survives the call.
    fn truncated_throughout() -> Vec<Reply> {
        vec![Reply::Truncated(FIXTURE.len() / 3); TRANSFER_RETRIES as usize + 1]
    }

    /// Asserts the fixture's three files are on disk, and that the archive that produced them is not.
    fn assert_extracted(dir: &Path) {
        test_server::assert_extracted(dir, ARCHIVE);
    }

    #[tokio::test]
    async fn a_first_install_downloads_expands_and_records() {
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);
        let (reporter, seen) = recording();

        install(dir.path(), &dependency, reporter).await.unwrap();

        assert_extracted(dir.path());

        let recorded = manifest::read(dir.path()).unwrap();
        assert_eq!(recorded.name, "onnx-runtime");
        assert_eq!(recorded.version, "runtime/1.26.0");
        assert_eq!(recorded.fingerprint, manifest::fingerprint(&dependency));
        assert_eq!(
            recorded.files.iter().map(|f| (f.path.as_str(), f.size)).collect::<Vec<_>>(),
            EXTRACTED.to_vec(),
            "the record must name every extracted file and nothing else"
        );

        let reports = seen.lock().unwrap().clone();
        assert!(reports.iter().any(|r| r.phase == Phase::Downloading), "no report arrived during the transfer");
        assert!(
            reports.iter().any(|r| r.phase == Phase::Extracting),
            "the expansion was reported as a pause rather than as work"
        );
        assert_eq!(reports.last().unwrap().fraction, 1.0);
        assert!(reports.windows(2).all(|w| w[0].fraction <= w[1].fraction));
    }

    #[tokio::test]
    async fn a_second_launch_with_nothing_changed_installs_nothing() {
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);

        install(dir.path(), &dependency, silent()).await.unwrap();
        let after_first = server.requests();

        let (reporter, seen) = recording();
        install(dir.path(), &dependency, reporter).await.unwrap();

        assert_eq!(server.requests(), after_first, "the second launch contacted the server");
        assert!(seen.lock().unwrap().is_empty(), "a skipped install reported work");
        assert_extracted(dir.path());
    }

    #[tokio::test]
    async fn a_deleted_file_forces_a_reinstall() {
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);

        install(dir.path(), &dependency, silent()).await.unwrap();
        let after_first = server.requests();
        std::fs::remove_file(dir.path().join("onnxruntime.1.26.0.dylib")).unwrap();

        install(dir.path(), &dependency, silent()).await.unwrap();

        assert!(server.requests() > after_first, "a missing file did not force a reinstall");
        assert_extracted(dir.path());
    }

    #[tokio::test]
    async fn a_bumped_pin_removes_the_previous_versions_files() {
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();

        install(dir.path(), &descriptor(&server, "runtime/1.26.0", None), silent()).await.unwrap();

        // A file only the old version installed. The point of the record is that a replacement deletes it rather than
        // leaving two versions' libraries side by side for the loader to choose between.
        let stale = dir.path().join("onnxruntime.1.25.0.dylib");
        std::fs::write(&stale, b"the previous release").unwrap();
        let mut recorded = manifest::read(dir.path()).unwrap();
        recorded.files.push(manifest::File { path: "onnxruntime.1.25.0.dylib".to_string(), size: 20 });
        manifest::write(dir.path(), &recorded).unwrap();

        install(dir.path(), &descriptor(&server, "runtime/1.27.0", None), silent()).await.unwrap();

        assert!(!stale.exists(), "the previous version's file survived the bump");
        assert_extracted(dir.path());

        let after = manifest::read(dir.path()).unwrap();
        assert_eq!(after.version, "runtime/1.27.0");
        assert_eq!(
            after.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            EXTRACTED.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
            "the new record must list only what the new archive produced"
        );
    }

    #[tokio::test]
    async fn an_install_interrupted_after_the_record_was_removed_reinstalls() {
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);

        install(dir.path(), &dependency, silent()).await.unwrap();
        let after_first = server.requests();

        // Exactly what a process killed mid-extraction leaves: the files are there, the record is not.
        manifest::clear(dir.path()).unwrap();
        assert!(dir.path().join("onnxruntime.1.26.0.dylib").exists());

        install(dir.path(), &dependency, silent()).await.unwrap();

        assert!(
            server.requests() > after_first,
            "a directory with no record was mistaken for a finished install"
        );
        assert_extracted(dir.path());
        assert!(manifest::read(dir.path()).is_some());
    }

    #[tokio::test]
    async fn an_unrecorded_directory_is_emptied_rather_than_merged_into() {
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();

        // Populated by a version of the app that kept no record. Its contents cannot be described, so extracting over
        // them would leave the previous release's libraries behind.
        std::fs::write(dir.path().join("onnxruntime.1.25.0.dylib"), b"undescribed").unwrap();

        install(dir.path(), &descriptor(&server, "runtime/1.26.0", None), silent()).await.unwrap();

        assert!(
            !dir.path().join("onnxruntime.1.25.0.dylib").exists(),
            "an undescribed file survived the install"
        );
        assert_extracted(dir.path());
    }

    #[tokio::test]
    async fn an_interrupted_download_resumes_rather_than_restarting() {
        // Every attempt of the first install is cut short, so it gives up leaving a partial behind. `rust-sak` retries
        // within one transfer, so the script has to outlast those retries for the interruption to survive the call.
        let server = TestServer::start(truncated_throughout()).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);

        let interrupted = install(dir.path(), &dependency, silent()).await;
        assert!(interrupted.is_err(), "a truncated response was accepted as complete");
        assert!(part_path(&dir.path().join(ARCHIVE)).exists(), "an interrupted transfer left nothing to resume");
        let before = server.ranges().await.len();
        install(dir.path(), &dependency, silent()).await.unwrap();

        let ranges = server.ranges().await[before..].to_vec();
        assert!(
            ranges.first().is_some_and(|range| range.starts_with("bytes=") && range != "bytes=0-"),
            "the next install restarted from zero instead of resuming, ranges: {ranges:?}"
        );
        assert_extracted(dir.path());
    }

    #[tokio::test]
    async fn a_transfer_stops_where_it_stands_when_it_is_no_longer_wanted() {
        // The half of a cancellation that has to reach the network. `rust-sak` runs a transfer on a task of its own,
        // so stopping the *waiting* leaves it writing to the disk with nobody left to want what it produces — which
        // is a user who cancelled a multi-gigabyte model download and went on paying for it.
        let server = TestServer::start(vec![Reply::Stalled(STALL_AFTER)]).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);
        let cancel = CancellationToken::new();

        // The server is holding the connection open, so the transfer cannot finish and the stop lands mid-flight.
        let stopping = stop_once_transferring(part_path(&dir.path().join(ARCHIVE)), cancel.clone());

        let (log, outcome) =
            logging::records_of("info", || async { super::install(dir.path(), &dependency, silent(), &cancel).await })
                .await;
        stopping.await.unwrap();

        assert!(matches!(outcome, Err(InitError::Stopped)), "a stopped transfer reported {outcome:?}");

        // Closed at the level it opened, as a stop rather than a failure.
        let stopped = records(&log, "an install stopped");
        assert_eq!(stopped.len(), 1, "{log}");
        assert!(stopped[0].contains("level=INFO"), "{}", stopped[0]);
        assert_eq!(field(stopped[0], "reason"), Some("cancelled"));
        assert!(field(stopped[0], "duration").is_some(), "{}", stopped[0]);
        assert!(!log.contains("level=WARN"), "a stopped install was recorded as a failure:\n{log}");
        assert!(
            !dir.path().join("onnxruntime.1.26.0.dylib").exists(),
            "an install that was stopped mid-transfer went on to expand the archive"
        );
        assert!(
            part_path(&dir.path().join(ARCHIVE)).exists(),
            "a stopped transfer left nothing on disk for the next one to resume onto"
        );
    }

    #[tokio::test]
    async fn an_install_that_follows_a_stopped_one_resumes_onto_what_it_left() {
        // What makes stopping affordable: the bytes already paid for are still there, so a user who changes their
        // mind twice does not pay for the model twice. The second install is the one the window makes when the
        // enhancement is added again.
        let server = TestServer::start(vec![Reply::Stalled(STALL_AFTER)]).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);
        let cancel = CancellationToken::new();

        let stopping = stop_once_transferring(part_path(&dir.path().join(ARCHIVE)), cancel.clone());

        let stopped = super::install(dir.path(), &dependency, silent(), &cancel).await;
        stopping.await.unwrap();
        assert!(matches!(stopped, Err(InitError::Stopped)));

        let before = server.ranges().await.len();

        install(dir.path(), &dependency, silent()).await.unwrap();

        let ranges = server.ranges().await[before..].to_vec();
        assert!(
            ranges.first().is_some_and(|range| range.starts_with("bytes=") && range != "bytes=0-"),
            "the install after a stop started again from zero instead of resuming, ranges: {ranges:?}"
        );
        assert_extracted(dir.path());
    }

    #[tokio::test]
    async fn a_mismatch_after_a_resumed_transfer_recovers_on_a_clean_second_pass() {
        // Leave a partial, then answer its resume with bytes that do not hash — a stale prefix's symptom. The clean
        // restart is what recovers.
        let mut script = truncated_throughout();
        script.push(Reply::Corrupt);
        let server = TestServer::start(script).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);

        let _ = install(dir.path(), &dependency, silent()).await;
        assert!(part_path(&dir.path().join(ARCHIVE)).exists());

        install(dir.path(), &dependency, silent()).await.unwrap();

        assert_extracted(dir.path());
        assert!(manifest::read(dir.path()).is_some());
    }

    #[tokio::test]
    async fn a_mismatch_that_does_not_recover_fails_with_both_hashes_and_leaves_no_bytes() {
        let server = TestServer::start(vec![Reply::Corrupt]).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);

        let error = install(dir.path(), &dependency, silent()).await.unwrap_err();

        match error {
            InitError::HashMismatch { expected, actual, .. } => {
                assert_eq!(expected, sha256_bytes(FIXTURE));
                assert_eq!(actual, sha256_bytes(&vec![b'x'; FIXTURE.len()]));
            }
            other => panic!("expected HashMismatch, got {other:?}"),
        }

        assert!(!dir.path().join(ARCHIVE).exists(), "the bad bytes were left on disk");
        assert!(
            !part_path(&dir.path().join(ARCHIVE)).exists(),
            "the bad bytes were left where a later run could resume onto them"
        );
        assert!(manifest::read(dir.path()).is_none(), "a failed install left a record");
        assert!(!dir.path().join("onnxruntime.1.26.0.dylib").exists(), "bad bytes were extracted");
    }

    #[tokio::test]
    async fn a_transfer_that_started_from_zero_and_mismatched_is_not_retried() {
        // Nothing to resume onto means the prefix cannot be the cause, so a second identical transfer would only be
        // retrying the clean download that just failed.
        let server = TestServer::start(vec![Reply::Corrupt]).await;
        let dir = install_dir();

        let _ = install(dir.path(), &descriptor(&server, "runtime/1.26.0", None), silent()).await;

        assert_eq!(server.requests(), 1, "a fresh mismatch was retried");
    }

    // The next two exercise `acquire` rather than `install`, because `install` never reaches this state through its
    // own front door: an archive at the target path is undescribed content, and the directory is emptied before the
    // transfer starts. The short-circuit is still worth covering — it is the one path the in-stream digest cannot
    // reach, and the fallback is what keeps verification unconditional.

    #[tokio::test]
    async fn a_file_already_at_the_target_path_is_hashed_rather_than_trusted() {
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);
        std::fs::write(dir.path().join(ARCHIVE), FIXTURE).unwrap();

        let archive = acquire(dir.path(), &dependency.sources[0], &silent(), &CancellationToken::new()).await.unwrap();

        assert_eq!(server.requests(), 0, "an archive already on disk was downloaded again");
        assert_eq!(archive, dir.path().join(ARCHIVE));
    }

    #[tokio::test]
    async fn a_file_already_at_the_target_path_that_is_not_the_artifact_is_rejected() {
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);
        std::fs::write(dir.path().join(ARCHIVE), b"not the published archive").unwrap();

        let error = acquire(dir.path(), &dependency.sources[0], &silent(), &CancellationToken::new())
            .await
            .unwrap_err();

        assert!(matches!(error, InitError::HashMismatch { .. }), "got {error:?}");
        assert!(!dir.path().join(ARCHIVE).exists(), "the wrong bytes were left on disk");
    }

    #[tokio::test]
    async fn a_pin_bumped_while_a_partial_was_on_disk_restarts_from_zero() {
        // The sidecar half of the rule, which the test below cannot reach: these partial bytes are
        // identifiable, they are simply a prefix of the *previous* release. The pinned hash is the resume key and
        // the tag is in the URL, so both halves of the identity moved - and the failure must be caught before a
        // byte is appended rather than left for the hash comparison a whole transfer later.
        let server = TestServer::start(truncated_throughout()).await;
        let dir = install_dir();

        // A real interrupted transfer at the old pin, leaving a partial and the sidecar naming it.
        let old = descriptor(&server, "runtime/1.26.0", None);
        assert!(install(dir.path(), &old, silent()).await.is_err());
        assert!(part_path(&dir.path().join(ARCHIVE)).exists());
        let before = server.ranges().await.len();

        install(dir.path(), &descriptor(&server, "runtime/1.27.0", None), silent()).await.unwrap();

        let ranges = server.ranges().await[before..].to_vec();
        assert_eq!(
            ranges.first().map(String::as_str),
            Some(""),
            "the previous release's bytes were resumed onto, ranges: {ranges:?}"
        );
        assert_extracted(dir.path());
    }

    #[tokio::test]
    async fn a_partial_with_no_sidecar_is_discarded_rather_than_appended_to() {
        // Bytes whose origin is unknowable. Guessing that they belong to this request is exactly the failure the
        // sidecar exists to prevent, so they are dropped rather than resumed onto.
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();
        std::fs::write(part_path(&dir.path().join(ARCHIVE)), b"the previous release").unwrap();

        install(dir.path(), &descriptor(&server, "runtime/1.26.0", None), silent()).await.unwrap();

        let ranges = server.ranges().await;
        assert_eq!(ranges, vec![""], "a partial with no sidecar was resumed onto, ranges: {ranges:?}");
        assert_extracted(dir.path());
    }

    #[tokio::test]
    async fn progress_advances_during_the_expansion_before_it_finishes() {
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();
        let (reporter, seen) = recording();

        install(dir.path(), &descriptor(&server, "runtime/1.26.0", None), reporter).await.unwrap();

        let reports = seen.lock().unwrap().clone();
        let extracting: Vec<_> = reports.iter().filter(|r| r.phase == Phase::Extracting).collect();

        assert!(extracting.len() > 1, "the expansion reported only its own end");
        assert!(
            extracting.iter().any(|r| r.fraction > 0.0 && r.fraction < 1.0),
            "the expansion jumped straight from the start to the end"
        );
        assert!(extracting.iter().any(|r| r.bytes > 0), "no report carried bytes written by the expansion");
    }

    // The no-expansion branch. A model is a bare file, so nothing above or below the branch changes: the same
    // fingerprint skip, the same resume, the same one-retry hash recovery, the same record written last.

    /// The name a bare model file is served and stored under.
    const MODEL: &str = "up_kyoto_4x_fp32.onnx";

    /// A bare-file descriptor serving the fixture's bytes as a model from `server`.
    fn bare(server: &TestServer, version: &str) -> Dependency {
        // The fixture's bytes stand in for a graph: what is being driven is the pipeline's branch, not anything about
        // ONNX — nothing here opens the file.
        Dependency {
            name: "up_kyoto_4x_fp32".to_string(),
            version: version.to_string(),
            dir: "models/up_kyoto_4x_fp32".to_string(),
            progress: Which::Model(crate::models::ArtifactId::new(
                crate::models::Family::Upscale,
                "kyoto",
                Some(4),
                crate::models::Precision::Fp32,
            )),
            lib: None,
            provides: None,
            derived: Vec::new(),
            sources: vec![test_server::fixture_source(server, version, MODEL)],
            contents: Contents::Bare,
            trust: ModelTrust::Published,
        }
    }

    /// A reporter for a bare install, collecting what it is told.
    fn recording_bare() -> (Arc<Reporter>, Arc<Mutex<Vec<Progress>>>) {
        let (on_progress, seen) = progress::recording();
        let dependency = Which::Model(crate::models::ArtifactId::new(
            crate::models::Family::Upscale,
            "kyoto",
            Some(4),
            crate::models::Precision::Fp32,
        ));

        (
            Arc::new(Reporter::new(dependency, Some(on_progress), Some(FIXTURE.len() as u64), Expansion::None)),
            seen,
        )
    }

    /// A reporter for a bare install that discards its reports.
    fn silent_bare() -> Arc<Reporter> {
        Arc::new(Reporter::new(Which::Cuda, None, Some(FIXTURE.len() as u64), Expansion::None))
    }

    #[tokio::test]
    async fn a_bare_file_lands_unexpanded_and_is_recorded_as_itself() {
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();
        let dependency = bare(&server, "abc123");
        let (reporter, seen) = recording_bare();

        install(dir.path(), &dependency, reporter).await.unwrap();

        // The file itself, at its published size, still there — not expanded and not deleted as an archive would be.
        let model = dir.path().join(MODEL);
        assert!(model.is_file(), "the model file is not on disk");
        assert_eq!(std::fs::metadata(&model).unwrap().len(), FIXTURE.len() as u64);
        assert!(!part_path(&model).exists(), "a finished transfer left a partial");

        // And it is what the record names, so replacing this model later deletes exactly this.
        let recorded = manifest::read(dir.path()).unwrap();
        assert_eq!(recorded.name, "up_kyoto_4x_fp32");
        assert_eq!(
            recorded.files.iter().map(|f| (f.path.as_str(), f.size)).collect::<Vec<_>>(),
            vec![(MODEL, FIXTURE.len() as u64)]
        );
        assert_eq!(recorded.fingerprint, manifest::fingerprint(&dependency));

        // Nothing was reported as an expansion, and the transfer alone carried the bar home.
        let reports = seen.lock().unwrap().clone();
        assert!(reports.iter().any(|r| r.phase == Phase::Downloading));
        assert!(
            reports.iter().all(|r| r.phase != Phase::Extracting),
            "an install with nothing to expand reported an expansion"
        );
        assert!(reports.windows(2).all(|w| w[0].fraction <= w[1].fraction));
        assert_eq!(reports.last().unwrap().fraction, 1.0);
    }

    #[tokio::test]
    async fn a_second_install_of_a_bare_file_transfers_nothing() {
        // The fingerprint skip, unchanged by the branch: the recorded identity is the published one and the recorded
        // file is still there at its recorded size.
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();
        let dependency = bare(&server, "abc123");

        install(dir.path(), &dependency, silent_bare()).await.unwrap();
        let after_first = server.requests();

        let (reporter, seen) = recording_bare();
        install(dir.path(), &dependency, reporter).await.unwrap();

        assert_eq!(server.requests(), after_first, "the second install contacted the server");
        assert!(seen.lock().unwrap().is_empty(), "a skipped install reported work");
        assert!(dir.path().join(MODEL).is_file());
    }

    #[tokio::test]
    async fn a_bare_file_whose_bytes_are_wrong_fails_as_a_mismatch_and_leaves_nothing() {
        // The verification is the same code on both branches, so a bare file gets it too — which is the whole point:
        // there is no path that puts an unverified model on disk.
        let server = TestServer::start(vec![Reply::Corrupt]).await;
        let dir = install_dir();

        let error = install(dir.path(), &bare(&server, "abc123"), silent_bare()).await.unwrap_err();

        assert!(matches!(error, InitError::HashMismatch { .. }), "got {error:?}");
        assert!(!dir.path().join(MODEL).exists(), "the wrong bytes were left where a graph would be opened");
        assert!(manifest::read(dir.path()).is_none(), "a failed install left a record");
    }

    #[tokio::test]
    async fn an_interrupted_bare_transfer_resumes_and_the_fraction_does_not_go_backwards() {
        let server = TestServer::start(truncated_throughout()).await;
        let dir = install_dir();
        let dependency = bare(&server, "abc123");

        assert!(install(dir.path(), &dependency, silent_bare()).await.is_err());
        assert!(part_path(&dir.path().join(MODEL)).exists(), "an interrupted transfer left nothing to resume");
        let before = server.ranges().await.len();

        let (reporter, seen) = recording_bare();
        install(dir.path(), &dependency, reporter).await.unwrap();

        let ranges = server.ranges().await[before..].to_vec();
        assert!(
            ranges.first().is_some_and(|range| range.starts_with("bytes=") && range != "bytes=0-"),
            "the resumed install restarted from zero, ranges: {ranges:?}"
        );

        let reports = seen.lock().unwrap().clone();
        assert!(reports.windows(2).all(|w| w[0].fraction <= w[1].fraction), "the fraction went backwards");
        assert_eq!(reports.last().unwrap().fraction, 1.0);
        assert!(dir.path().join(MODEL).is_file());
    }

    #[tokio::test]
    async fn a_bare_install_with_no_recipient_behaves_identically() {
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();

        install(dir.path(), &bare(&server, "abc123"), silent_bare()).await.unwrap();

        assert!(dir.path().join(MODEL).is_file());
        assert!(manifest::read(dir.path()).is_some());
    }

    #[tokio::test]
    async fn an_install_with_no_callback_behaves_identically() {
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();

        install(dir.path(), &descriptor(&server, "runtime/1.26.0", None), silent()).await.unwrap();

        assert_extracted(dir.path());
        assert!(manifest::read(dir.path()).is_some());
    }

    #[tokio::test]
    async fn an_install_accounts_for_itself_and_a_second_one_says_it_did_nothing() {
        let server = TestServer::start(vec![]).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);

        let (first, ()) = logging::records_of("debug", || async {
            install(dir.path(), &dependency, silent()).await.unwrap();
        })
        .await;

        let installing = records(&first, "installing a dependency");
        assert_eq!(installing.len(), 1, "{first}");
        assert!(installing[0].contains("level=INFO"), "{}", installing[0]);
        assert_eq!(field(installing[0], "dependency"), Some("onnx-runtime"));
        assert_eq!(field(installing[0], "version"), Some("runtime/1.26.0"));

        let extracting = records(&first, "extracting an archive");
        assert_eq!(extracting.len(), 1, "{first}");
        assert!(extracting[0].contains("level=INFO"), "{}", extracting[0]);

        let ready = records(&first, "the dependency is ready");
        assert_eq!(ready.len(), 1, "{first}");
        assert!(ready[0].contains("level=INFO"), "{}", ready[0]);
        assert!(field(ready[0], "duration").is_some(), "the record does not say how long it took: {}", ready[0]);
        let files: usize = field(ready[0], "files").unwrap().parse().unwrap();
        assert_eq!(files, EXTRACTED.len(), "the record does not say what was put on disk: {}", ready[0]);

        // A second install transfers nothing, and says so below the default level — it is what every dependency does
        // on every launch after the first.
        let (second, ()) = logging::records_of("debug", || async {
            install(dir.path(), &dependency, silent()).await.unwrap();
        })
        .await;

        let present = records(&second, "the dependency is already present and complete");
        assert_eq!(present.len(), 1, "{second}");
        assert!(present[0].contains("level=DEBUG"), "{}", present[0]);
        assert!(
            records(&second, "installing a dependency").is_empty(),
            "a no-op install claimed to install:\n{second}"
        );

        // And at the default level a launch with nothing to do writes nothing at all about this dependency.
        let (ordinary, ()) = logging::records_of("info", || async {
            install(dir.path(), &dependency, silent()).await.unwrap();
        })
        .await;
        assert!(!ordinary.contains("msg="), "a no-op install wrote to an ordinary session's file:\n{ordinary}");
    }

    #[tokio::test]
    async fn a_repeated_transfer_says_why_it_is_happening() {
        // The record the reference splits across four warnings: this one knows a partial existed, so it can say that
        // the clean pass is a stale prefix being ruled out rather than an unexplained second download.
        let mut script = truncated_throughout();
        script.push(Reply::Corrupt);
        let server = TestServer::start(script).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);

        let _ = install(dir.path(), &dependency, silent()).await;
        assert!(part_path(&dir.path().join(ARCHIVE)).exists());

        let (log, ()) = logging::records_of("info", || async {
            install(dir.path(), &dependency, silent()).await.unwrap();
        })
        .await;

        let repeated = records(
            &log,
            "what a resumed transfer produced does not match the published hash; transferring again from nothing",
        );
        assert_eq!(repeated.len(), 1, "the repeat was recorded {} times:\n{log}", repeated.len());
        assert!(repeated[0].contains("level=WARN"), "a repeated transfer is not a warning: {}", repeated[0]);
        assert_eq!(field(repeated[0], "artifact"), Some(ARCHIVE));
        assert!(
            field(repeated[0], "expected").is_some() && field(repeated[0], "actual").is_some(),
            "{}",
            repeated[0]
        );

        // And the install still finished, which is why this is a warning rather than the failure record below.
        assert!(records(&log, "installing a dependency failed").is_empty(), "{log}");
        assert_eq!(records(&log, "the dependency is ready").len(), 1, "{log}");
    }

    #[tokio::test]
    async fn an_install_that_fails_names_the_dependency_and_the_reason() {
        let server = TestServer::start(vec![Reply::Corrupt]).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);

        let (log, outcome) =
            logging::records_of("info", || async { install(dir.path(), &dependency, silent()).await }).await;
        assert!(matches!(outcome, Err(InitError::HashMismatch { .. })), "{outcome:?}");

        let failed = records(&log, "installing a dependency failed");
        assert_eq!(failed.len(), 1, "the failure was recorded {} times:\n{log}", failed.len());
        assert!(failed[0].contains("level=WARN"), "{}", failed[0]);
        assert_eq!(field(failed[0], "dependency"), Some("onnx-runtime"));
        assert!(field(failed[0], "error").is_some(), "the failure does not say why: {}", failed[0]);

        // The install that began is accounted for at both ends, so a reader is never left at a beginning.
        assert_eq!(records(&log, "installing a dependency").len(), 1, "{log}");
        assert!(
            records(&log, "the dependency is ready").is_empty(),
            "a failed install claimed to be ready:\n{log}"
        );
    }

    #[tokio::test]
    async fn a_failed_install_marks_its_span_failed_and_counts_the_failure_by_kind() {
        use crate::telemetry::metrics::FAILURES;

        let server = TestServer::start(vec![Reply::Corrupt]).await;
        let dir = install_dir();
        let dependency = descriptor(&server, "runtime/1.26.0", None);

        // `>=`: a parallel test may fail an install on a checksum too.
        let failures = || FAILURES.value(&[("unit", "install"), ("kind", "checksum")]);
        let before = failures();
        let (_, observed, outcome) =
            logging::traced_of("info", || async { install(dir.path(), &dependency, silent()).await }).await;
        assert!(matches!(outcome, Err(InitError::HashMismatch { .. })), "{outcome:?}");

        let span = observed.only("install");
        assert_eq!(span.field("dependency"), Some("onnx-runtime"));
        assert_eq!(span.field("version"), Some("runtime/1.26.0"));
        assert_eq!(span.field("outcome"), Some("failed"));
        assert!(span.field("error").is_some_and(|error| error.contains("hashed to")), "{span:?}");
        assert!(failures() > before, "the failure was not counted");

        let failed = observed
            .event("installing a dependency failed")
            .expect("the failure did not reach the collector");
        assert_eq!(failed.span, Some(span.index), "the failure record is not in the install's trace");
    }

    #[test]
    fn an_install_ended_by_a_shutdown_is_recorded_as_a_stop_rather_than_a_failure() {
        // Through the recorder rather than a real shutdown: `InitError::Cancelled` is what `spawn_blocking` answers
        // when the runtime is dropped under it, and a test cannot drop the runtime it is running on and still read
        // its own records.
        let dependency = Dependency {
            name: "onnx-runtime".to_string(),
            version: "runtime/1.26.0".to_string(),
            dir: "runtime".to_string(),
            progress: Which::Runtime,
            lib: None,
            provides: None,
            derived: Vec::new(),
            sources: Vec::new(),
            contents: Contents::Archive,
            trust: ModelTrust::Published,
        };

        let (log, ()) = logging::records_of_blocking("info", || {
            record_ended(&dependency, Duration::from_millis(250), &InitError::Cancelled);
        });

        let stopped = records(&log, "an install stopped");
        assert_eq!(stopped.len(), 1, "{log}");
        assert!(stopped[0].contains("level=INFO"), "{}", stopped[0]);
        assert_eq!(field(stopped[0], "reason"), Some("shutdown"));
        assert_eq!(field(stopped[0], "duration"), Some("250ms"));
        assert!(!log.contains("level=WARN"), "a shutdown was recorded as a failed install:\n{log}");
    }
}
