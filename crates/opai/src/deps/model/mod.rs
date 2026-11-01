//! Getting a named model's files onto disk and proving they are the published ones.
//!
//! Nothing here opens a graph. This is the seam between `models/`, which knows what a model file is *called*, and
//! `deps/`, which knows how to put a verified file on disk — so code that runs inference takes the weights as already
//! present rather than as something it has to fetch.
//!
//! The installing is `rust-sak`'s file installer: a model is every file of the published `models` folder whose name
//! starts with `<artifact>.onnx` (a model too large for the 2 GB protobuf limit keeps its weights in a sibling
//! `.onnx.data`), installed into `models/<artifact>/` and verified against the hash the live listing publishes for each.
//! What a provider compiled from it, `engines/<artifact>/`, is emptied whenever its files are replaced.
//!
//! The listing is the only source of a model's hashes. It is read **lazily** — the first install that needs it, never
//! during [`crate::Opai::initialize`] — and memoized for the process, so a chain of installs costs one round trip and
//! `initialize` has no network dependency. Where it cannot be read, a model that is installed and intact is used as it
//! is, and one that is not fails as [`InitError::UnlistedModel`] with nothing transferred. An artifact the listing does
//! not name fails as [`InitError::UnpublishedModel`]. There is no path here that places a model file on disk without
//! checking it.

// The reference implementation composes the URL anyway and installs the file with an empty expected hash when the
// listing cannot be read, and reaches that from an ordinary timeout. Refusing costs only a first install on a machine
// that cannot reach the listing — and the files are on the same host, so it could not download them either.

pub(crate) mod paths;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::Instant;

use rust_sak::ai::hf::{self, HfError, Hub};
use rust_sak::ai::{self, Event, Failure, Files, Folder, InstallError, Listing, Outcome, Select, Trust};
use tokio::sync::OnceCell;
use tracing::Instrument as _;

use crate::config;
use crate::deps::model::paths::{engine_dir, model_dir};
use crate::deps::{Installing, ModelTrust, records};
use crate::error::InitError;
use crate::models::ArtifactId;
use crate::progress::{Dependency as Which, Reporter};
use crate::telemetry::unit::{self, Unit, unit_span};

/// Where the project publishes its models.
pub(crate) const FOLDER_URL: &str = "https://huggingface.co/vegidio/open-photo-ai/tree/main/models";

/// A published models folder, and what this process has learned of its listing.
///
/// One per process for the project's own folder ([`ModelFolder::published`]); a test builds its own, pointed at a
/// local server, so that no test inherits the listing another one read.
pub(crate) struct ModelFolder {
    folder: Folder,
    /// The listing, once a read of it succeeded. Never refreshed within a process: it pins the commit, so every model
    /// one process installs comes from one revision, and a republish reaches a long-lived process on its next launch.
    listing: OnceCell<Arc<Listing>>,
    /// A read of the listing failed in this process. Set once and never cleared.
    offline: AtomicBool,
    /// The record that installed models are being used without the listing was written.
    warned: AtomicBool,
    /// Retries of a transient failure, for the listing and the downloads.
    retries: u32,
}

impl ModelFolder {
    /// The project's published models folder.
    pub(crate) fn published() -> &'static Self {
        static PUBLISHED: LazyLock<ModelFolder> = LazyLock::new(|| {
            ModelFolder::new(Folder::parse(FOLDER_URL).expect("the published folder URL parses"), hf::RETRIES)
        });
        &PUBLISHED
    }

    /// The models folder at `folder`, retrying nothing, as a test of a failing server wants.
    #[cfg(test)]
    pub(crate) fn at(folder: Folder) -> Self {
        Self::new(folder, 0)
    }

    fn new(folder: Folder, retries: u32) -> Self {
        Self {
            folder,
            listing: OnceCell::new(),
            offline: AtomicBool::new(false),
            warned: AtomicBool::new(false),
            retries,
        }
    }

    /// The listing, read now if no read of it has succeeded in this process, or `None` where it cannot be read.
    async fn listing(&self) -> Option<Arc<Listing>> {
        let read = self
            .listing
            .get_or_try_init(|| async {
                let listing = Listing::read_retrying(&self.folder, self.retries).await?;
                // `debug`: it happens once per process that acquires a model, and says only that nothing went wrong.
                tracing::debug!(
                    url = %self.folder,
                    entries = listing.files().len(),
                    "read the published model listing"
                );
                Ok::<_, HfError>(Arc::new(listing))
            })
            .await;

        match read {
            Ok(listing) => Some(Arc::clone(listing)),
            Err(error) => {
                self.offline.store(true, Ordering::Release);
                // `debug`: what a reader needs is the `warn` an installed model used without it writes, once per run,
                // or the error a model that is not installed fails with.
                tracing::debug!(url = %self.folder, %error, "the published model listing could not be read");
                None
            }
        }
    }
}

/// Installs every published file backing `id` from `folder` and returns the directory they landed in.
///
/// See the module documentation for where the hashes come from and what happens without them.
///
/// # Errors
///
/// [`InitError::UnpublishedModel`] when the listing names no file for `id`, and [`InitError::UnlistedModel`] when the
/// listing cannot be read and `id` is not installed, both with nothing transferred. [`InitError::Download`] if the
/// network or the server failed, [`InitError::HashMismatch`] if a file's bytes are not the published ones,
/// [`InitError::Io`] or [`InitError::Fs`] for the directories and files around them, and [`InitError::Stopped`] if
/// `installing.cancel` was cancelled while the install ran.
pub(crate) async fn install(
    folder: &ModelFolder,
    app_dir: &Path,
    name: &str,
    id: &ArtifactId,
    trust: ModelTrust,
    installing: &Installing,
) -> Result<PathBuf, InitError> {
    let dir = config::sub_dir(app_dir, name, &model_dir(id))?;
    let files = Files {
        name: id.as_str().to_owned().into(),
        folder: folder.folder.clone(),
        select: Select::Prefix(format!("{id}.onnx")),
        dir: dir.clone(),
        // What a provider compiled from these weights, which a replacement of them must discard: new weights beside an
        // engine built from the old ones produce a wrong image with nothing reporting it.
        derived: vec![app_dir.join(engine_dir(id))],
        // The one place in this crate where the declaration reaches an install.
        trust: match trust {
            ModelTrust::Published => Trust::Published,
            ModelTrust::LocalFiles => Trust::LocalFiles,
        },
    };

    // Once a read of the listing has failed in this process, an installed model does not wait on it again: a machine
    // with no network acquiring several models would otherwise sit through the retries for each one. A model that is
    // not installed tries it again, since nothing else can install it.
    let listing = if folder.offline.load(Ordering::Acquire) && ai::files_present(&files) {
        None
    } else {
        folder.listing().await
    };

    // A model has no release tag, so what identifies *which* published bytes are installed is the graph's own hash.
    let graph = format!("{id}.onnx");
    let version = listing
        .as_ref()
        .and_then(|listing| listing.files().iter().find(|file| file.file_name() == graph))
        .map_or_else(|| "unlisted".to_owned(), |file| file.hash.hex().to_owned());

    let span = unit_span!("install", dependency = id.as_str(), version = version.as_str());

    // Wrapped so that the failure record is written once, here, whichever of the failures below produced it. Started
    // before the body, so a stop or a failure says how long the install had run.
    let started = Instant::now();
    let outcome = run(folder, &files, id, listing.as_deref(), &version, installing).instrument(span.clone()).await;
    let duration = started.elapsed();

    match &outcome {
        Ok(()) => unit::ended(Unit::Install, &span, duration, unit::Outcome::Finished),
        // Inside the span, so the record is in the install's trace, and ended the way the record says it ended.
        Err(error) => span.in_scope(|| {
            records::record_ended(id.as_str(), &version, duration, error);

            let ending = match error.stop_reason() {
                Some(_) => unit::Outcome::Stopped,
                None => unit::Outcome::Failed { kind: error.kind(), error },
            };
            unit::ended(Unit::Install, &span, duration, ending);
        }),
    }

    outcome.map(|()| dir)
}

/// [`install`]'s body, with the failure record left to its caller.
async fn run(
    folder: &ModelFolder,
    files: &Files,
    id: &ArtifactId,
    listing: Option<&Listing>,
    version: &str,
    installing: &Installing,
) -> Result<(), InitError> {
    // One reporter per model: two installs sharing a reporter interleave into a figure no front end could present.
    let total = listing.map(|listing| {
        listing
            .files()
            .iter()
            .filter(|file| files.select.matches(file.file_name()))
            .map(|file| file.size)
            .sum()
    });
    let reporter = Reporter::for_dependency(Which::Model(id.clone()), total, installing.on_progress.clone());

    // Recorded on the first event rather than before the call: only `rust-sak` knows whether anything has to move, and
    // an install that is already current says nothing at the default level.
    let started = Instant::now();
    let mut announced = false;
    let on_event = |event: Event| {
        if !announced {
            announced = true;
            records::started(id.as_str(), version, &files.dir);
        }
        if let Event::Progress { bytes, total, fraction, .. } = event {
            reporter.installing(bytes, total, fraction);
        }
    };

    // The stop is dropping the install: `rust-sak` then cancels the transfer, keeps its partial to resume onto, and
    // releases the folder's lock.
    let outcome = tokio::select! {
        outcome = ai::install_files_retrying(files, listing, folder.retries, on_event) => outcome,
        () = installing.cancel.cancelled() => return Err(InitError::Stopped),
    };

    match outcome.map_err(|error| failed(folder, files, id, error))? {
        Outcome::Installed | Outcome::Adopted => {
            records::ready(id.as_str(), version, started.elapsed());
            // Last, and only once every file and the record are in place, so a front end never sees 100% for an
            // install that has not finished.
            reporter.finish();
        }
        // `debug`: it is what every model does on every acquisition after the first.
        Outcome::Current => {
            tracing::debug!(dependency = id.as_str(), version, "the dependency is already present and complete");
        }
        Outcome::Offline => {
            // Once per run, not once per model: what a reader needs is that the listing failed and that every model
            // from here on ran as it was installed, which is the one way a republished model goes unnoticed.
            if !folder.warned.swap(true, Ordering::AcqRel) {
                tracing::warn!(
                    url = %folder.folder,
                    "the published model listing could not be read; installed models are used as they are, without \
                     checking for a newer version"
                );
            }
            tracing::debug!(dependency = id.as_str(), "an installed model is used without the published listing");
        }
        // Every time the path is taken, not once per process — a model whose verification was skipped is the first
        // thing to suspect in a report of wrong output, and a launch that found the stamp unmoved is exactly the launch
        // where it would otherwise go unsaid. `warn`, so it is in the file a user attaches without being asked to
        // reproduce anything with the logging turned up.
        Outcome::Trusted => tracing::warn!(
            dependency = id.as_str(),
            path = %files.dir.display(),
            "every file of this model is already on disk and this process trusts them; verification skipped"
        ),
    }

    Ok(())
}

/// What `rust-sak`'s `error` installing `id` means here.
fn failed(folder: &ModelFolder, files: &Files, id: &ArtifactId, error: InstallError) -> InitError {
    let artifact = id.as_str().to_owned();
    match error.failure {
        Failure::NotListed => {
            // **A refusal, and the divergence is deliberate.** The reference *warns* here and then downloads the
            // unknown model unverified; `model-install`'s specification fixes this project's refusal instead. The
            // level is still a `warn`: the refusal is returned to the caller, and whether it is fatal is the caller's
            // to decide (see `logging` for the rule).
            tracing::warn!(
                artifact = %id,
                "no published source names this model; refusing to install it rather than transferring it unverified"
            );
            InitError::UnpublishedModel { artifact }
        }
        Failure::Unlisted => InitError::UnlistedModel { artifact },
        Failure::Damaged { path, expected, actual } => InitError::HashMismatch {
            url: Hub::for_folder(&folder.folder).url(folder.folder.revision(), &path),
            expected,
            actual,
        },
        Failure::DiskFull => {
            InitError::Io { path: files.dir.clone(), source: std::io::Error::from(std::io::ErrorKind::StorageFull) }
        }
        Failure::Io(reason) => InitError::Io { path: files.dir.clone(), source: std::io::Error::other(reason) },
        failure => InitError::Download { url: folder.folder.to_string(), source: failure },
    }
}

#[cfg(test)]
mod tests;
