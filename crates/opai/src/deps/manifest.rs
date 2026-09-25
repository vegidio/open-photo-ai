//! The on-disk record of what a dependency installed.
//!
//! The record is both halves of a replacement: it says whether what is on disk is already the pinned version, and it
//! is the exact list of files to delete when it is not.

use std::collections::BTreeSet;
use std::io;
use std::path::{Component, Path, PathBuf};

use rust_sak::crypto::sha256_string;
use rust_sak::fs::{self, ListOptions};
use serde::{Deserialize, Serialize};

use crate::deps::release::Dependency;

/// The record a dependency writes into the directory it owns.
pub(crate) const MANIFEST_NAME: &str = ".manifest.json";

/// Versions the on-disk format. A record written under a different schema is treated as absent, which costs a
/// reinstall rather than a misread.
const MANIFEST_SCHEMA: u32 = 1;

// Declared together because they are a contract between the code that creates them and the two places that recognise
// them: `record_tree`, which keeps them out of a manifest, and `empty_dir`, which must not delete a large download
// that is still resumable.
//
// `.part.json` has to be listed in its own right. It ends in `.json`, not `.part`, so matching the latter alone misses
// it — and a sidecar left behind would be walked up and written into the manifest as installed content.
/// Suffixes marking a file as bookkeeping rather than installed content.
const TRANSIENT_SUFFIXES: &[&str] = &[".part", ".part.json", ".tmp"];

/// One installed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct File {
    /// Where it sits, relative to the install directory and always slash-separated, so a manifest written on Windows
    /// reads the same everywhere.
    pub(crate) path: String,
    /// Its size when it was recorded. Compared — not re-hashed — on every later launch.
    pub(crate) size: u64,
}

/// What a dependency put on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Manifest {
    /// The on-disk format version.
    pub(crate) schema: u32,
    /// The dependency's name, written for whoever opens the file rather than for this crate.
    pub(crate) name: String,
    /// The release tag, likewise for a human reader.
    pub(crate) version: String,
    /// What actually decides a reinstall — see [`fingerprint`].
    pub(crate) fingerprint: String,
    /// Every file the install produced.
    pub(crate) files: Vec<File>,
}

/// Derives a dependency's reinstall key from the inputs that define its content: the release tag, and every source's
/// URL and expected hash.
pub(crate) fn fingerprint(dependency: &Dependency) -> String {
    // The fingerprint rather than the version, because both kinds of change are then caught by one comparison — an
    // archive dependency moves when its tag is bumped, and a model moves when its published hash changes, which no tag
    // would have recorded.
    let mut input = dependency.version.clone();
    for source in &dependency.sources {
        // A separator that cannot occur in a URL or a hex digest, so no two different dependencies can hash the same
        // string by having their fields run together.
        input.push('\0');
        input.push_str(&source.url);
        input.push('\0');
        input.push_str(&source.sha256);
    }

    sha256_string(&input)
}

/// Reads the record in `dir`, or `None` when there is nothing there to trust.
///
/// Missing, unreadable, malformed and written under another schema all collapse to the same answer: each one costs a
/// reinstall rather than trusting a partial file list.
///
/// **Three of the four are recorded and one is not.** A record that is *there* and cannot be used — malformed, or
/// written under a schema this build does not know — is a `warn`. A record that is simply absent is the ordinary state
/// of a directory nothing has installed into yet, and says nothing.
pub(crate) fn read(dir: &Path) -> Option<Manifest> {
    // A `warn` because the reinstall an unusable record forces is a transfer a user pays for, and the file it read is
    // evidence a reader may want.
    let path = dir.join(MANIFEST_NAME);

    // `read` rather than `exists` first, so a record that is there and unreadable — a permission, a truncated
    // filesystem — is distinguished from one that is not there, which is the ordinary case.
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "an install record could not be read; reinstalling");
            return None;
        }
    };

    let manifest: Manifest = match serde_json::from_slice(&bytes) {
        Ok(manifest) => manifest,
        Err(error) => {
            tracing::warn!(path = %path.display(), %error, "an install record is malformed; reinstalling");
            return None;
        }
    };

    if manifest.schema != MANIFEST_SCHEMA {
        tracing::warn!(
            path = %path.display(),
            schema = manifest.schema,
            expected = MANIFEST_SCHEMA,
            "an install record was written under another schema; reinstalling"
        );
        return None;
    }

    Some(manifest)
}

/// Writes the record into `dir` atomically, so it is only ever observed complete: an interrupted write leaves the
/// previous record or none at all, and both mean "reinstall".
///
/// # Errors
///
/// Returns an [`io::Error`] if the record cannot be serialized, written or renamed into place.
pub(crate) fn write(dir: &Path, manifest: &Manifest) -> io::Result<()> {
    let json = serde_json::to_vec_pretty(manifest).map_err(io::Error::other)?;

    // Beside the target rather than in a temp directory, so the rename is a same-filesystem operation and therefore
    // atomic. The suffix is one `record_tree` treats as transient, so a crash mid-write cannot leave a file that the
    // next install records as installed content.
    let temporary = dir.join(format!("{MANIFEST_NAME}.tmp"));
    std::fs::write(&temporary, json)?;
    std::fs::rename(&temporary, dir.join(MANIFEST_NAME))
}

/// Deletes the record, treating "it was not there" as success.
///
/// # Errors
///
/// Returns an [`io::Error`] if a record exists but cannot be removed.
pub(crate) fn clear(dir: &Path) -> io::Result<()> {
    // A permission problem must not be mistaken for "already gone", or an interrupted install would be reported as
    // finished.
    remove_file(&dir.join(MANIFEST_NAME))
}

impl Manifest {
    /// Builds the record for what `dependency` just installed.
    pub(crate) fn new(dependency: &Dependency, files: Vec<File>) -> Self {
        Self {
            schema: MANIFEST_SCHEMA,
            name: dependency.name.clone(),
            version: dependency.version.clone(),
            fingerprint: fingerprint(dependency),
            files,
        }
    }

    /// Reports whether every recorded file is still present at its recorded size.
    ///
    /// A record naming no files is not intact.
    pub(crate) fn intact(&self, dir: &Path) -> bool {
        // An install that produced nothing is not one to trust.
        if self.files.is_empty() {
            return false;
        }

        // The steady-state check that runs on every launch, so it is deliberately a stat per file rather than a hash:
        // re-reading gigabytes on every start would cost seconds to catch a corruption that install-time verification
        // against the pinned hash already ruled out.
        self.files.iter().all(|file| {
            // `symlink_metadata`, not `metadata`: the Linux archives carry `libfoo.so -> libfoo.so.N` chains, which
            // were recorded as links and would be read as corruption on every launch if the target were followed.
            match std::fs::symlink_metadata(dir.join(from_slash(&file.path))) {
                Ok(meta) => !meta.is_dir() && meta.len() == file.size,
                Err(_) => false,
            }
        })
    }

    /// Deletes exactly the recorded files, then prunes the subdirectories that emptied. `dir` itself always survives.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if a recorded file exists but cannot be removed.
    pub(crate) fn remove(&self, dir: &Path) -> io::Result<()> {
        for file in &self.files {
            remove_file(&dir.join(from_slash(&file.path)))?;
        }

        // Deepest first, so a nested directory is gone before its parent is tried. Removing a directory that still
        // holds something fails, which is exactly the wanted behaviour — so the error is the stop condition rather
        // than a failure.
        //
        // `dir` itself is never pruned: on Linux it is on `LD_LIBRARY_PATH` from before the process re-execed, and the
        // loader permanently skips a search path it finds missing.
        for sub in subdirectories(&self.files) {
            let _ = std::fs::remove_dir(dir.join(from_slash(&sub)));
        }

        Ok(())
    }
}

/// Records every file under `dir`, which is how an archive's contents are captured: extraction produces a list nobody
/// declared, so it is read back off the disk afterwards. Only the path and size are recorded.
///
/// # Errors
///
/// Returns an [`io::Error`] if the directory cannot be walked or a file cannot be stat'd.
pub(crate) fn record_tree(dir: &Path) -> io::Result<Vec<File>> {
    // Enumerated rather than diffed before and after: an archive overwriting a file already present would be missing
    // from a diff, and the manifest has to name every file it owns for the next uninstall to be exact.
    //
    // No hashes: hashing each extracted file would be a second full read of the expanded tree to produce a value
    // nothing reads — `Manifest::intact` compares sizes, and the bytes were already verified against the pinned hash
    // of the archive they came out of.
    let paths = fs::list_path(dir, &ListOptions::new().recursive(true)).map_err(io::Error::other)?;

    let mut files = Vec::with_capacity(paths.len());
    for path in paths {
        let Some(relative) = to_slash(dir, &path) else {
            continue;
        };
        if relative == MANIFEST_NAME || is_transient(&relative) {
            continue;
        }

        // `symlink_metadata` for the same reason `intact` uses it: what is recorded has to be the size of the entry
        // itself, since that is what the check compares against.
        let meta = std::fs::symlink_metadata(&path)?;
        files.push(File { path: relative, size: meta.len() });
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// Deletes everything in `dir` except the bookkeeping of an in-flight download, keeping `dir` itself.
///
/// Partial downloads survive, so a large interrupted transfer is still resumable.
///
/// # Errors
///
/// Returns an [`io::Error`] if the directory cannot be read or an entry cannot be removed.
pub(crate) fn empty_dir(dir: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        if is_transient(&name) {
            continue;
        }

        // `file_type` comes from the directory entry and does not follow links, so a symlink to a directory is
        // unlinked rather than having its target's contents removed.
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            remove_file(&path)?;
        }
    }

    Ok(())
}

/// Reports whether a name is bookkeeping rather than installed content: a partial download, the record of what that
/// download is of, or a JSON document mid-write.
fn is_transient(name: &str) -> bool {
    let base = name.rsplit('/').next().unwrap_or(name);
    TRANSIENT_SUFFIXES.iter().any(|suffix| base.ends_with(suffix))
}

/// Every directory the recorded files live in, deepest first.
fn subdirectories(files: &[File]) -> Vec<String> {
    // Each `/` in a recorded path marks the end of one ancestor, so the ancestors are prefixes of the path already
    // held — borrowed rather than rebuilt, which is what keeps a tree of several thousand files from allocating a
    // `String` per file per directory level only to have the set discard nearly all of them as duplicates.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for file in files {
        for (end, _) in file.path.rmatch_indices('/') {
            seen.insert(&file.path[..end]);
        }
    }

    let mut dirs: Vec<String> = seen.into_iter().map(str::to_string).collect();
    dirs.sort_by_key(|dir| std::cmp::Reverse(dir.matches('/').count()));
    dirs
}

/// Turns a recorded, slash-separated path back into one for this platform.
pub(crate) fn from_slash(path: &str) -> PathBuf {
    path.split('/').collect()
}

/// Turns `path` into a slash-separated path relative to `dir`, or `None` if it is not under it.
///
/// Any component that is not a plain name makes it `None` rather than something to record: a manifest is a delete
/// list, and a `..` in it would delete outside the directory it describes.
fn to_slash(dir: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(dir).ok()?;

    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?.to_string()),
            _ => return None,
        }
    }

    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Removes `path`, treating "it was not there" as success.
fn remove_file(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps::release::Source;
    use crate::logging;
    use crate::progress::Dependency as Which;

    /// A dependency descriptor with one source, for the fingerprint tests.
    fn dependency(version: &str, url: &str, sha256: &str) -> Dependency {
        Dependency {
            name: "onnx-runtime".to_string(),
            version: version.to_string(),
            dir: "runtime".to_string(),
            progress: Which::Runtime,
            // Neither is part of the fingerprint — which is version and sources — so they only have to be
            // plausible.
            lib: Some("onnxruntime.1.26.0.dylib"),
            derived: Vec::new(),
            provides: None,
            sources: vec![Source { url: url.to_string(), sha256: sha256.to_string(), size: 10 }],
            contents: crate::deps::release::Contents::Archive,
            trust: crate::deps::ModelTrust::Published,
        }
    }

    /// Creates `dir/relative`, with its parents, holding `contents`.
    fn seed(dir: &Path, relative: &str, contents: &[u8]) -> PathBuf {
        let path = dir.join(from_slash(relative));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn the_fingerprint_moves_when_the_tag_the_url_or_the_hash_does() {
        let base = dependency("runtime/1.26.0", "https://example.com/a.7z", "aa");
        let same = fingerprint(&base);

        assert_eq!(fingerprint(&dependency("runtime/1.26.0", "https://example.com/a.7z", "aa")), same);
        assert_ne!(fingerprint(&dependency("runtime/1.27.0", "https://example.com/a.7z", "aa")), same);
        assert_ne!(fingerprint(&dependency("runtime/1.26.0", "https://example.com/b.7z", "aa")), same);
        assert_ne!(fingerprint(&dependency("runtime/1.26.0", "https://example.com/a.7z", "bb")), same);
    }

    #[test]
    fn fields_cannot_run_together_into_the_same_fingerprint() {
        // Without a separator the tag and the URL would concatenate, so two different dependencies could hash the
        // same string.
        let split = dependency("runtime/1.26.0", "https://example.com/a.7z", "aa");
        let joined = dependency("runtime/1.26.0https://example.com/a.7z", "", "aa");

        assert_ne!(fingerprint(&split), fingerprint(&joined));
    }

    #[test]
    fn a_record_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = Manifest::new(
            &dependency("runtime/1.26.0", "https://example.com/a.7z", "aa"),
            vec![File { path: "lib/onnxruntime.so".to_string(), size: 42 }],
        );

        write(dir.path(), &manifest).unwrap();

        assert_eq!(read(dir.path()), Some(manifest));
        assert!(
            !dir.path().join(format!("{MANIFEST_NAME}.tmp")).exists(),
            "the atomic write left its temporary behind"
        );
    }

    #[test]
    fn a_missing_malformed_or_wrong_schema_record_all_read_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read(dir.path()), None, "missing");

        std::fs::write(dir.path().join(MANIFEST_NAME), b"not json at all").unwrap();
        assert_eq!(read(dir.path()), None, "malformed");

        std::fs::write(
            dir.path().join(MANIFEST_NAME),
            br#"{"schema":99,"name":"onnx-runtime","version":"runtime/1.26.0","fingerprint":"aa","files":[]}"#,
        )
        .unwrap();
        assert_eq!(read(dir.path()), None, "another schema");
    }

    #[test]
    fn clearing_a_record_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();

        clear(dir.path()).unwrap();
        std::fs::write(dir.path().join(MANIFEST_NAME), b"{}").unwrap();
        clear(dir.path()).unwrap();

        assert!(!dir.path().join(MANIFEST_NAME).exists());
    }

    #[test]
    fn intact_compares_presence_and_size() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "lib/a.so", b"0123456789");
        let manifest = Manifest::new(
            &dependency("runtime/1.26.0", "https://example.com/a.7z", "aa"),
            vec![File { path: "lib/a.so".to_string(), size: 10 }],
        );

        assert!(manifest.intact(dir.path()));

        seed(dir.path(), "lib/a.so", b"truncated");
        assert!(!manifest.intact(dir.path()), "a different size must force a reinstall");

        std::fs::remove_file(dir.path().join("lib").join("a.so")).unwrap();
        assert!(!manifest.intact(dir.path()), "a missing file must force a reinstall");
    }

    #[test]
    fn a_record_naming_no_files_is_not_intact() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = Manifest::new(&dependency("runtime/1.26.0", "https://example.com/a.7z", "aa"), Vec::new());

        assert!(!manifest.intact(dir.path()));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_chain_is_not_read_as_corruption() {
        // The Linux runtime archives carry `libfoo.so -> libfoo.so.N`. Following the link would compare the target's
        // size against the link's recorded size, and read every launch as corrupt.
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "libonnxruntime.so.1.26.0", b"the real library");
        std::os::unix::fs::symlink("libonnxruntime.so.1.26.0", dir.path().join("libonnxruntime.so")).unwrap();

        let files = record_tree(dir.path()).unwrap();
        let manifest = Manifest::new(&dependency("runtime/1.26.0", "https://example.com/a.7z", "aa"), files);

        assert_eq!(manifest.files.len(), 2);
        assert!(manifest.intact(dir.path()), "a recorded symlink must read as present, not as corruption");
    }

    #[test]
    fn record_tree_captures_the_whole_tree_and_skips_bookkeeping() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "b.txt", b"bb");
        seed(dir.path(), "nested/deep/a.txt", b"a");
        seed(dir.path(), MANIFEST_NAME, b"{}");
        seed(dir.path(), "onnx.7z.part", b"partial bytes");
        seed(dir.path(), "onnx.7z.part.json", b"{}");
        seed(dir.path(), format!("{MANIFEST_NAME}.tmp").as_str(), b"{}");

        let files = record_tree(dir.path()).unwrap();

        assert_eq!(
            files,
            vec![
                File { path: "b.txt".to_string(), size: 2 },
                File { path: "nested/deep/a.txt".to_string(), size: 1 },
            ],
            "the record must name installed content only, sorted and slash-separated"
        );
    }

    #[test]
    fn remove_deletes_the_recorded_files_and_prunes_what_emptied() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "nested/deep/a.txt", b"a");
        seed(dir.path(), "nested/kept/b.txt", b"b");

        let manifest = Manifest::new(
            &dependency("runtime/1.26.0", "https://example.com/a.7z", "aa"),
            vec![File { path: "nested/deep/a.txt".to_string(), size: 1 }],
        );
        manifest.remove(dir.path()).unwrap();

        assert!(!dir.path().join("nested").join("deep").exists(), "an emptied directory survived");
        assert!(
            dir.path().join("nested").join("kept").join("b.txt").exists(),
            "a directory still holding something was pruned"
        );
        assert!(dir.path().exists(), "the install directory itself must survive");
    }

    #[test]
    fn remove_keeps_the_install_directory_even_when_it_empties_completely() {
        // On Linux this directory is on LD_LIBRARY_PATH from before the process re-execed, and the loader permanently
        // skips a search path it finds missing.
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "a.so", b"a");

        let manifest = Manifest::new(
            &dependency("runtime/1.26.0", "https://example.com/a.7z", "aa"),
            vec![File { path: "a.so".to_string(), size: 1 }],
        );
        manifest.remove(dir.path()).unwrap();

        assert!(dir.path().is_dir());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn remove_tolerates_a_recorded_file_that_is_already_gone() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = Manifest::new(
            &dependency("runtime/1.26.0", "https://example.com/a.7z", "aa"),
            vec![File { path: "never/existed.so".to_string(), size: 1 }],
        );

        manifest.remove(dir.path()).unwrap();
    }

    #[test]
    fn emptying_an_unrecorded_directory_spares_a_resumable_download() {
        let dir = tempfile::tempdir().unwrap();
        seed(dir.path(), "stale.so", b"old version");
        seed(dir.path(), "nested/also-stale.so", b"old version");
        seed(dir.path(), "onnx.7z.part", b"half a gigabyte, notionally");
        seed(dir.path(), "onnx.7z.part.json", b"{}");

        empty_dir(dir.path()).unwrap();

        assert!(!dir.path().join("stale.so").exists());
        assert!(!dir.path().join("nested").exists());
        assert!(dir.path().join("onnx.7z.part").exists(), "a resumable partial was deleted");
        assert!(dir.path().join("onnx.7z.part.json").exists(), "a partial's sidecar was deleted");
    }

    #[test]
    fn subdirectories_are_listed_deepest_first() {
        let files =
            vec![File { path: "a/b/c/x.txt".to_string(), size: 1 }, File { path: "a/y.txt".to_string(), size: 1 }];

        assert_eq!(subdirectories(&files), vec!["a/b/c", "a/b", "a"]);
    }

    #[test]
    fn the_transient_suffixes_match_what_the_install_actually_creates() {
        // The comment on `TRANSIENT_SUFFIXES` calls this a contract between the code that creates these files and the
        // two places that recognise them. Asserted here rather than only narrated: if `rust-sak` renames its partial or
        // its sidecar and `install::part_path` follows, the mismatch has to fail here — the alternative is `empty_dir`
        // deleting a multi-gigabyte resumable download on every launch, and `record_tree` writing the leftovers into a
        // manifest as installed content.
        let target = Path::new("/somewhere/opai/runtime/onnx_linux_amd64.7z");

        for path in [crate::deps::install::part_path(target), crate::deps::install::sidecar_path(target)] {
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(is_transient(name), "`{name}` is created by the install but is not recognised as bookkeeping");
        }
    }

    #[test]
    fn transient_names_are_recognised_including_the_sidecar() {
        assert!(is_transient("onnx.7z.part"));
        // Ends in `.json`, not `.part`: matching the latter alone misses it.
        assert!(is_transient("onnx.7z.part.json"));
        assert!(is_transient(".manifest.json.tmp"));
        assert!(is_transient("nested/onnx.7z.part"));
        assert!(!is_transient("onnxruntime.so.1.26.0"));
        assert!(!is_transient("partial-name.so"));
    }

    #[test]
    fn a_record_that_is_there_and_cannot_be_used_says_so_and_a_missing_one_does_not() {
        // The distinction that decides whether anything is written: a record that is absent is the ordinary state of
        // a directory nothing has installed into, while one that is present and unusable costs a transfer.
        let dir = tempfile::tempdir().unwrap();

        let (missing, record) = logging::records_of_blocking("debug", || read(dir.path()));
        assert_eq!(record, None);
        assert!(!missing.contains("msg="), "an absent record wrote a line:\n{missing}");

        std::fs::write(dir.path().join(MANIFEST_NAME), b"not json at all").unwrap();
        let (malformed, record) = logging::records_of_blocking("info", || read(dir.path()));
        assert_eq!(record, None);
        let line = malformed
            .lines()
            .find(|line| line.contains("an install record is malformed"))
            .unwrap_or_else(|| panic!("{malformed}"));
        assert!(line.contains("level=WARN"), "{line}");
        assert!(line.contains(MANIFEST_NAME), "the record does not name the file: {line}");

        std::fs::write(
            dir.path().join(MANIFEST_NAME),
            br#"{"schema":99,"name":"onnx-runtime","version":"runtime/1.26.0","fingerprint":"aa","files":[]}"#,
        )
        .unwrap();
        let (wrong_schema, record) = logging::records_of_blocking("info", || read(dir.path()));
        assert_eq!(record, None);
        let line = wrong_schema
            .lines()
            .find(|line| line.contains("an install record was written under another schema"))
            .unwrap_or_else(|| panic!("{wrong_schema}"));
        assert!(line.contains("level=WARN"), "{line}");
        assert!(line.contains("schema=99") && line.contains(&format!("expected={MANIFEST_SCHEMA}")), "{line}");
    }
}
