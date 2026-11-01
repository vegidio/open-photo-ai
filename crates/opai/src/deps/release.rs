//! The descriptor the install pipeline works from: a model's published files and where they install.

use std::path::PathBuf;

use crate::deps::ModelTrust;
use crate::progress::Dependency as Which;

/// One remote artifact a dependency is installed from.
#[derive(Debug, Clone)]
pub(crate) struct Source {
    /// The absolute address to download from. Its base name is the file name written to disk.
    pub(crate) url: String,
    /// The expected SHA-256 of the bytes as downloaded.
    pub(crate) sha256: String,
    /// The expected size in bytes, used to spread a progress report before the response declares a length.
    pub(crate) size: u64,
}

impl Source {
    /// The file name these bytes are written to disk under: the base name of the URL.
    pub(crate) fn file_name(&self) -> &str {
        // A method rather than three call sites deriving it, because the rule has to hold for all of them at once —
        // whatever the download writes, the verification hashes and the cleanup deletes must be the same file.
        self.url.rsplit('/').next().unwrap_or(&self.url)
    }
}

/// An installable set of files described by one manifest.
#[derive(Debug, Clone)]
pub(crate) struct Dependency {
    // Owned rather than `&'static str`, because a model's is composed from an `ArtifactId` at run time.
    /// How the dependency identifies itself in logs and in its manifest: a model's artifact name.
    pub(crate) name: String,
    /// The published hash of the model's graph. Written into the manifest for whoever opens it; the fingerprint is what
    /// any decision is actually made on.
    pub(crate) version: String,
    // Composed from the artifact name, so the caller never has to pair a descriptor
    // with a destination itself. Owned for the same reason `name` is; why each model has a directory of its own is on
    // `descriptor::model_dir`.
    /// The subdirectory of the application's configuration directory this installs into, as in
    /// `models/up_kyoto_4x_fp32`.
    pub(crate) dir: String,
    /// Which dependency this descriptor's progress reports name themselves as.
    pub(crate) progress: Which,
    // Not relative like `dir`, because the install pipeline is handed one already-resolved directory and has no
    // application directory to resolve a second one against.
    /// Directories holding things **computed from** this dependency rather than downloaded with it, emptied when its
    /// files are replaced.
    ///
    /// Absolute. A model points at the directory an execution provider
    /// compiles into ([`crate::deps::model::descriptor::engine_dir`]). Whoever resolves `dir` resolves these.
    pub(crate) derived: Vec<PathBuf>,
    /// The artifacts to fetch: one per file backing a model, which is several for a model whose weights sit in a
    /// sibling `.onnx.data`.
    pub(crate) sources: Vec<Source>,
    // **Here rather than as a second argument to the install**, so that `install::install_inner` reads one field
    // without knowing what kind of dependency it has — and so that "model files only" is enforced by where the code
    // can reach rather than by a branch that has to remember. The only place this is ever set to anything but the
    // default is `model::descriptor::descriptor_at`, where a model's descriptor is composed.
    /// Whether this dependency's files may be used as they sit on disk rather than checked against what is published.
    pub(crate) trust: ModelTrust,
}

impl Dependency {
    /// The published size of the files backing this dependency: the sum of its sources' published sizes.
    ///
    /// Known before anything is requested, which is what makes it reportable for a dependency that has not started and
    /// for one that never will because it is already installed.
    pub(crate) fn published_size(&self) -> u64 {
        // One answer rather than the same fold written at each of its call sites — the progress bar this install is
        // spread over, and the plan row a front end draws before it starts.
        self.sources.iter().map(|source| source.size).sum()
    }
}
