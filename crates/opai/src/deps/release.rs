//! Turning a pinned release into the descriptor the install pipeline works from.

use std::path::PathBuf;

use crate::deps::ModelTrust;
use crate::deps::artifact::Release;
use crate::error::InitError;
use crate::progress::{Dependency as Which, Expansion};
use crate::providers::Provider;

/// Where the project publishes its dependency archives. The tag is a path segment of its own, so a URL reads
/// `.../download/runtime/1.26.0/onnx_darwin_arm64.7z`.
pub(crate) const RELEASE_BASE_URL: &str = "https://github.com/vegidio/open-photo-ai/releases/download";

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

// On the descriptor rather than sniffed from the file, so the pipeline's one branch is driven by what the dependency
// *is* rather than by a guess about what arrived — a guess that would be made against bytes that had just been
// downloaded from the network.
/// What the install does with a source once it has been acquired and verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Contents {
    /// A release archive: expanded into the directory, then deleted, so what is recorded is what it produced.
    Archive,
    /// A file that is already what it will be on disk. A model graph is a bare `.onnx` with nothing to expand, so it
    /// stays where the transfer put it and is recorded as itself.
    Bare,
}

/// An installable set of files described by one manifest.
#[derive(Debug, Clone)]
pub(crate) struct Dependency {
    // Owned rather than `&'static str`, because a model's is composed from an `ArtifactId` at run time rather than
    // read off a `const` row. The pinned table itself stays `&'static` - only the descriptor built from it owns its
    // copy.
    /// How the dependency identifies itself in logs and in its manifest: `onnx-runtime`, or a model's artifact name.
    pub(crate) name: String,
    /// The release tag the sources came from. Written into the manifest for whoever opens it; the fingerprint is what
    /// any decision is actually made on.
    pub(crate) version: String,
    // Carried from the pinned row - or composed from an artifact name - so the caller never has to pair a descriptor
    // with a destination itself. Owned for the same reason `name` is; why each model has a directory of its own is on
    // `descriptor::model_dir`.
    /// The subdirectory of the application's configuration directory this installs into: `runtime`, `libs/cuda`,
    /// `models/up_kyoto_4x_fp32`.
    pub(crate) dir: String,
    /// Which dependency this descriptor's progress reports name themselves as, carried from the same row.
    pub(crate) progress: Which,
    // Carried from the same row, for the same reason `dir` is: the alternative is a second resolution of the platform
    // at the call site, which can disagree with the one the install actually ran from.
    /// The shared library inside the archive the loader has to be pointed at, or `None` where the dependency is a
    /// directory of files rather than one loadable file.
    pub(crate) lib: Option<&'static str>,
    /// The execution provider installing this unlocks, `None` for one that unlocks none of its own. Carried from the
    /// same row.
    pub(crate) provides: Option<Provider>,
    // Not relative like `dir`, because the install pipeline is handed one already-resolved directory and has no
    // application directory to resolve a second one against.
    /// Directories holding things **computed from** this dependency rather than downloaded with it, emptied when its
    /// files are replaced.
    ///
    /// Absolute, and empty for every pinned release archive. A model points at the directory an execution provider
    /// compiles into ([`crate::deps::model::descriptor::engine_dir`]). Whoever resolves `dir` resolves these.
    pub(crate) derived: Vec<PathBuf>,
    /// The artifacts to fetch. One for a release archive; one per file backing a model, which is several for a model
    /// whose weights sit in a sibling `.onnx.data`.
    pub(crate) sources: Vec<Source>,
    /// What the install does with each source once it is verified.
    pub(crate) contents: Contents,
    // **Here rather than as a second argument to the install**, so that `install::install_inner` reads one field
    // without knowing what kind of dependency it has — and so that "model files only" is enforced by where the code
    // can reach rather than by a branch that has to remember. The only place this is ever set to anything but the
    // default is `model::descriptor::descriptor_at`, where a model's descriptor is composed; `from_release_at` builds
    // every pinned row — the runtime, CUDA, cuDNN, TensorRT — and has no argument that could carry it.
    /// Whether this dependency's files may be used as they sit on disk rather than checked against what is published.
    pub(crate) trust: ModelTrust,
}

impl Dependency {
    /// The published size of the files backing this dependency: the sum of its sources' pinned sizes.
    ///
    /// Known before anything is requested, which is what makes it reportable for a dependency that has not started and
    /// for one that never will because it is already installed.
    pub(crate) fn published_size(&self) -> u64 {
        // One answer rather than the same fold written at each of its call sites — the progress bar this install is
        // spread over, and the plan row a front end draws before it starts.
        self.sources.iter().map(|source| source.size).sum()
    }

    /// Whether this install has an expansion phase, which is what its progress report is spread over.
    pub(crate) fn expansion(&self) -> Expansion {
        // Derived from the descriptor rather than decided beside it, so the reporter a caller builds and the branch the
        // pipeline takes cannot disagree about whether an expansion is coming — and a bar cannot sit at 80% waiting
        // for an expansion that was never going to happen.
        match self.contents {
            Contents::Archive => Expansion::Follows,
            Contents::Bare => Expansion::None,
        }
    }

    /// Describes the archive `release` publishes for one platform, against one base URL.
    ///
    /// # Errors
    ///
    /// Returns [`InitError::UnsupportedPlatform`] when the release publishes nothing for this platform.
    pub(crate) fn from_release_at(
        base_url: &str,
        release: &Release,
        os: &'static str,
        arch: &'static str,
    ) -> Result<Self, InitError> {
        // Everything that varies between releases comes from the pinned table — the tag the URL is built from, the
        // hash the download is checked against, the size the progress report is spread over — so a version bump edits
        // that table and nothing else. Neither this function nor its callers name a release.
        //
        // Both the platform and the base URL are arguments rather than constants so the suite can check every pinned
        // platform's URL from whichever runner it is on, and point a real install at a local server.
        let pinned = release.pinned_for(os, arch)?;

        Ok(Self {
            name: release.name.to_string(),
            version: pinned.tag.to_string(),
            dir: release.dir.to_string(),
            progress: release.progress.clone(),
            lib: pinned.artifact.lib,
            provides: release.provides,
            // Nothing is computed from a runtime or a GPU library: what an execution provider compiles is compiled
            // from a model's weights, and only a model's descriptor names one.
            derived: Vec::new(),
            sources: vec![Source {
                url: format!("{}/{}/{}", base_url.trim_end_matches('/'), pinned.tag, pinned.asset),
                sha256: pinned.artifact.hash.to_string(),
                size: pinned.artifact.size,
            }],
            contents: Contents::Archive,
            // A pinned release archive is never used unchecked. There is deliberately no parameter here that could
            // make it one: an operator hand-placing a ~175 MB ONNX Runtime tree or a CUDA library is not a workflow
            // this exists for, and those are the downloaded archives of native code the specification holds apart.
            trust: ModelTrust::Published,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps::artifact::{CUDA, CUDNN, ONNX_RUNTIME, TENSORRT};

    #[test]
    fn each_pinned_platform_builds_its_own_release_url() {
        let cases = [
            ("macos", "aarch64", "runtime/1.26.0/onnx_darwin_arm64.7z"),
            ("linux", "x86_64", "runtime/1.26.0/onnx_linux_amd64.7z"),
            ("linux", "aarch64", "runtime/1.26.0/onnx_linux_arm64.7z"),
            ("windows", "x86_64", "runtime/1.26.0/onnx_windows_amd64.7z"),
            ("windows", "aarch64", "runtime/1.26.0/onnx_windows_arm64.7z"),
        ];

        for (os, arch, tail) in cases {
            let dependency = Dependency::from_release_at(RELEASE_BASE_URL, &ONNX_RUNTIME, os, arch).unwrap();
            let source = &dependency.sources[0];

            assert_eq!(source.url, format!("{RELEASE_BASE_URL}/{tail}"), "{os}/{arch}");
            assert_eq!(dependency.name, "onnx-runtime");
            assert_eq!(dependency.version, "runtime/1.26.0");
            // The row owns its destination and its progress name, so a caller cannot pair a descriptor with the
            // wrong directory — which is how one dependency's bump would delete another's files.
            assert_eq!(dependency.dir, "runtime");
            assert_eq!(dependency.progress, Which::Runtime);
            assert_eq!(source.sha256.len(), 64);
            assert!(source.size > 0);
        }
    }

    #[test]
    fn the_runtime_descriptor_carries_its_platform_library_name() {
        // The name the loader is pointed at, resolved by the same lookup the install ran from rather than by a second
        // `match` on the platform at the call site — which is what could disagree with what was actually installed.
        let cases = [
            ("macos", "aarch64", "onnxruntime.1.26.0.dylib"),
            ("linux", "x86_64", "onnxruntime.so.1.26.0"),
            ("linux", "aarch64", "onnxruntime.so.1.26.0"),
            ("windows", "x86_64", "onnxruntime-1.26.0.dll"),
            ("windows", "aarch64", "onnxruntime-1.26.0.dll"),
        ];

        for (os, arch, lib) in cases {
            let dependency = Dependency::from_release_at(RELEASE_BASE_URL, &ONNX_RUNTIME, os, arch).unwrap();
            assert_eq!(dependency.lib, Some(lib), "{os}/{arch}");
        }

        assert_eq!(cases.len(), ONNX_RUNTIME.archives.len(), "a pinned platform is untested");
    }

    #[test]
    fn a_gpu_library_descriptor_names_no_library_at_all() {
        // None of the three has a single library the loader is pointed at: the archive is a tree of several hundred
        // files, and it is the directory that goes on the search path.
        for release in [&CUDA, &CUDNN, &TENSORRT] {
            for (os, arch) in [("linux", "x86_64"), ("linux", "aarch64"), ("windows", "x86_64")] {
                let dependency = Dependency::from_release_at(RELEASE_BASE_URL, release, os, arch).unwrap();
                assert_eq!(dependency.lib, None, "{} on {os}/{arch}", release.name);
            }
        }
    }

    #[test]
    fn a_descriptor_carries_the_provider_its_row_unlocks() {
        // From the row the install actually ran from, so the provider report derived downstream is a report of what
        // was installed rather than of what a second table elsewhere claims.
        let cases = [
            (&ONNX_RUNTIME, None),
            (&CUDA, Some(Provider::Cuda)),
            (&CUDNN, None),
            (&TENSORRT, Some(Provider::TensorRt)),
        ];

        for (release, provides) in cases {
            let dependency = Dependency::from_release_at(RELEASE_BASE_URL, release, "linux", "x86_64").unwrap();
            assert_eq!(dependency.provides, provides, "{}", release.name);
        }
    }

    #[test]
    fn a_descriptor_built_from_a_pinned_release_row_is_never_trusted() {
        // `model-install`'s "the runtime and the GPU libraries are verified in every process regardless", held by the
        // shape of the code rather than by a branch: there is no argument to this function that could say otherwise,
        // whatever the process declared at initialization. A model's descriptor is the only one with a field to set,
        // and it is set where a model's descriptor is built.
        for release in [&ONNX_RUNTIME, &CUDA, &CUDNN, &TENSORRT] {
            for (os, arch) in [("linux", "x86_64"), ("linux", "aarch64"), ("windows", "x86_64")] {
                let Ok(dependency) = Dependency::from_release_at(RELEASE_BASE_URL, release, os, arch) else {
                    continue;
                };

                assert_eq!(
                    dependency.trust,
                    ModelTrust::Published,
                    "{} on {os}/{arch} could be installed unverified",
                    release.name
                );
            }
        }
    }

    #[test]
    fn a_source_names_the_file_it_is_written_to() {
        let dependency = Dependency::from_release_at(RELEASE_BASE_URL, &ONNX_RUNTIME, "macos", "aarch64").unwrap();
        assert_eq!(dependency.sources[0].file_name(), "onnx_darwin_arm64.7z");
    }

    #[test]
    fn an_unpublished_platform_yields_no_descriptor_at_all() {
        // Not a descriptor with an empty hash: there is nothing here to install unverified against.
        let error = Dependency::from_release_at(RELEASE_BASE_URL, &ONNX_RUNTIME, "macos", "x86_64").unwrap_err();
        assert!(matches!(error, InitError::UnsupportedPlatform { .. }), "got {error:?}");
    }

    #[test]
    fn the_base_url_is_overridable_without_changing_the_rest_of_the_path() {
        // What lets the pipeline tests serve real archives from a local listener.
        let dependency =
            Dependency::from_release_at("http://127.0.0.1:9/base/", &ONNX_RUNTIME, "linux", "x86_64").unwrap();

        assert_eq!(dependency.sources[0].url, "http://127.0.0.1:9/base/runtime/1.26.0/onnx_linux_amd64.7z");
    }
}
