//! The published model listing: what each model file is called, how big it is, and the SHA-256 its bytes must match.
//!
//! Not to be confused with [`crate::deps::manifest`], which is the record an install writes into the directory it
//! owns. That one describes what *this* machine put on disk; this one describes what the project published, and is
//! the only thing a model's bytes are ever checked against.

use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use rust_sak::fetch::{Fetch, reqwest};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::OnceCell;

// Process-global rather than per-handle, which is the same rule the runtime library already follows: the first listing
// a process reads is the one it uses, so a second `Opai::initialize` under a different application name goes on using
// it rather than reading again. An `OnceCell` from `tokio` rather than `std`, because the initializer is `async` — and
// because that is what makes two concurrent first installs *coalesce* onto one read instead of racing into two.
//
// Behind an `Arc` because it is handed to every install and is immutable once resolved; there is nothing here to lock.
/// The listing resolved for this process, read at most once however many models are installed.
static RESOLVED: OnceCell<Arc<Listing>> = OnceCell::const_new();

// A tree listing rather than the repository's file list, because the tree is the only endpoint that reports each
// file's Git LFS object id — and that id is the hash, so it compares directly against a hash computed over the
// download.
/// The listing of the published models directory.
pub(crate) const TREE_URL: &str = "https://huggingface.co/api/models/vegidio/open-photo-ai/tree/main/models";

// The same JSON all three sources share, rather than a generated `.rs` file of `const` rows: a second representation
// of the same data that only the generator could write buys one avoided parse of a few hundred entries, once per
// process.
/// The listing compiled into the binary, covering every model this build can name that was published when it was
/// built.
///
/// It is produced by the generator in the test-only `super::generate` and checked in; see that module for when to re-run it.
const COMPILED_IN: &str = include_str!("published.json");

// A dotted name and a name of its own, so it cannot be confused with the per-install `.manifest.json` a model's own
// directory holds. It sits beside the per-model directories rather than inside one, which is what keeps it out of
// every install's file list — `record_tree` and `empty_dir` only ever see `models/<artifact-id>/`.
/// The copy of the listing kept from the last successful read, written directly into the models directory.
pub(crate) const CACHE_NAME: &str = ".listing.json";

/// How many times a failed listing request is retried before the read gives up and the next source is tried. The
/// backoff between attempts is `rust-sak`'s.
const READ_RETRIES: u32 = 2;

/// Bound on establishing a connection, so a black-holed host cannot hold an install open forever.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

// A page of this listing is a few tens of kilobytes, so unlike an archive transfer there is no legitimate reason for a
// long pause in the middle of one.
/// Bound on the silence between reads.
const READ_TIMEOUT: Duration = Duration::from_secs(15);

// The name is a base name rather than the repository path the listing reports, because that is what every consumer
// wants: it is the last segment of the download URL and the file name on disk. Reducing it here rather than at each
// use is what keeps the two from disagreeing.
/// One published file: the name it is served and stored under, its size, and the SHA-256 of its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Published {
    /// The file's base name: `up_kyoto_4x_fp32.onnx`, `up_osaka_fp16.onnx.data`.
    pub(crate) name: String,
    /// Its size in bytes, as published.
    pub(crate) size: u64,
    /// The SHA-256 of its contents, lowercase hex.
    pub(crate) sha256: String,
}

// All three sources a hash is resolved from deserialize into this one type through this one derive — the live listing
// is parsed into it, the cached copy is a verbatim copy of it, and the manifest compiled into the binary is the same
// JSON — so there is one format and one parser rather than three that can disagree.
/// Every published file, as one manifest.
///
/// `#[serde(transparent)]` so the serialized form is a bare array of [`Published`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Listing {
    files: Vec<Published>,
}

// A small struct over a large response on purpose: the endpoint also reports a git object id, a Xet hash and a pointer
// size, none of which this crate has any use for, and naming only what is read is what keeps a field added upstream
// from being a parse failure here.
/// One entry of a Hugging Face tree listing, reduced to the three fields that matter.
#[derive(Debug, Deserialize)]
struct Entry {
    /// The path within the repository: `models/up_kyoto_4x_fp32.onnx`.
    path: String,
    /// The size the listing reports. For an LFS-tracked file this is the content's size, not the pointer's.
    size: u64,
    /// The Git LFS record, absent for a file stored as an ordinary git blob and for a directory entry.
    #[serde(default)]
    lfs: Option<Lfs>,
}

/// The part of an entry's Git LFS record this crate reads.
#[derive(Debug, Deserialize)]
struct Lfs {
    /// The object id, which for LFS is the SHA-256 of the file's contents.
    oid: String,
}

// Crate-internal and deliberately not a variant of `InitError`: this failure is never reported to a caller. It is what
// makes the resolution fall through to the cached copy and then to the manifest compiled into the binary, and only the
// refusal at the end of that chain is something a caller can receive. A public variant nobody could be handed would be
// the wrong shape for the one thing that is actually reportable.
/// Why a live read of the published listing failed.
#[derive(Debug, Error)]
pub(crate) enum ReadError {
    /// The request itself failed — a transport error, a timeout, or a status the endpoint refused with.
    #[error("requesting the model listing at {url} failed: {source}")]
    Request {
        /// The page being requested, which the underlying error does not carry.
        url: String,
        /// What `rust-sak` reported.
        #[source]
        source: reqwest::Error,
    },

    /// The response arrived and is not a tree listing.
    #[error("the model listing at {url} is not a tree listing: {source}")]
    Malformed {
        /// The page whose body could not be read.
        url: String,
        /// What the parse reported.
        #[source]
        source: serde_json::Error,
    },
}

impl Listing {
    /// Builds a listing from entries already reduced to published files.
    pub(crate) fn new(files: Vec<Published>) -> Self {
        Self { files }
    }

    // Only the generator and the suite read the whole list; an install asks for one artifact's files through
    // `files_for`, so this is not compiled into a release.
    /// Every file the listing names.
    #[cfg(test)]
    pub(crate) fn files(&self) -> &[Published] {
        &self.files
    }

    /// The listing compiled into the binary, parsed once per process.
    ///
    /// # Panics
    ///
    /// Panics if the checked-in file is not a listing.
    pub(crate) fn compiled_in() -> &'static Arc<Self> {
        // Held as an `Arc` rather than as a bare listing because the fallback path below hands it to every caller in
        // the process: a `clone` of the listing itself would deep-copy several hundred rows, three heap allocations
        // each, to produce a value nothing ever mutates.
        static PARSED: OnceLock<Arc<Listing>> = OnceLock::new();

        PARSED.get_or_init(|| {
            Arc::new(serde_json::from_str(COMPILED_IN).expect("the checked-in model listing is generated and tested"))
        })
    }

    /// Reads the published listing from `url`, following its pagination to exhaustion.
    ///
    /// A page that advertises a target already read ends the walk rather than being followed again.
    ///
    /// # Errors
    ///
    /// Returns [`ReadError::Request`] if a page cannot be fetched and [`ReadError::Malformed`] if one is not a tree
    /// listing, each naming the page it was against.
    pub(crate) async fn read_live(url: &str) -> Result<Self, ReadError> {
        // The endpoint pages at 50 entries by default and advertises the next page as `Link: <url>; rel="next"`, with
        // no cursor in the body — so the pages are followed by that header until there is none. **No `limit` is passed
        // and no page count is assumed**, which is what keeps the number of published models from being a constant
        // anywhere in this project: the reference implementation's `?limit=1000` is a bound that is eventually wrong,
        // and under the verification rule its failure is an install refused rather than one merely truncated.
        let fetch = Fetch::new()
            .retries(READ_RETRIES)
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .header("Accept", "application/json");

        let mut files = Vec::new();
        let mut read: HashSet<String> = HashSet::new();
        let mut next = Some(url.to_string());

        while let Some(url) = next {
            // A cycle guard, not a bound on the collection: a server whose `next` points back into pages already seen
            // would otherwise be followed forever.
            if !read.insert(url.clone()) {
                break;
            }

            let page = fetch
                .text_response(&url)
                .await
                .map_err(|source| ReadError::Request { url: url.clone(), source })?;

            files.extend(Self::parse_tree_page(&page.body).map_err(|source| ReadError::Malformed { url, source })?);

            // The one place a further page is named. Copied out before the response is dropped, since the header it
            // is borrowed from lives on it.
            next = page.link("next").map(str::to_string);
        }

        Ok(Self::new(files))
    }

    /// The listing this process resolves against, reading it at most once.
    ///
    /// `url` is the listing to read and `models_dir` the directory the cached copy lives in; production passes
    /// [`TREE_URL`] and the application's `models` directory.
    ///
    /// **Only the first call in a process reads anything.** A later call — including one under a different `url` —
    /// is handed what the first resolved, which is what keeps a chain of installs to one round trip and writes the
    /// cached copy once.
    pub(crate) async fn resolve(url: &str, models_dir: &Path) -> Arc<Self> {
        // Both are arguments rather than constants so the suite can point a real resolution at a local server and a
        // temporary directory.
        Arc::clone(RESOLVED.get_or_init(|| async { Self::resolve_once(url, models_dir).await }).await)
    }

    /// Resolves the listing from the first of the three sources that answers: the live listing, the copy cached from
    /// the last successful read of it, then the manifest compiled into the binary.
    ///
    /// **This cannot fail.** The compiled-in manifest is part of the binary, so there is always a listing to resolve
    /// against — what can fail is finding a *particular* artifact in it, and that refusal belongs to the lookup rather
    /// than here. A source that fails is a fall-through, so the worst case is an older set of hashes rather than an
    /// unverified install.
    ///
    /// The failure of the live read is dropped as a *value* but **recorded**, and so is which of the three sources
    /// answered: the live read is a `debug` and the two fallbacks are `warn`s.
    async fn resolve_once(url: &str, models_dir: &Path) -> Arc<Self> {
        // Nothing above this can act on the live read's failure, since the fallback has already happened by the time it
        // would be seen — so the records are the point. A run working from a listing compiled in months ago behaves
        // almost identically to one working from the current listing, right up to the moment it cannot find a model
        // that exists, and by then the fallback is long past. The log is where the two are distinguishable. The
        // fallbacks are `warn`s because falling back is a degradation a user did not choose, and the successful case
        // is the ordinary one.
        let failure = match Self::read_live(url).await {
            Ok(listing) => {
                tracing::debug!(url, entries = listing.files.len(), "read the published model listing");

                // Written for the *next* run rather than for this one, which is why a failure here is not a failed
                // resolution: the listing is already in hand, and the only thing lost is a fallback later.
                if let Err(error) = listing.write_cached(models_dir) {
                    tracing::warn!(
                        path = %models_dir.display(),
                        %error,
                        "the model listing could not be written back to the cache; a later run has one fewer fallback"
                    );
                }

                return Arc::new(listing);
            }
            Err(error) => error,
        };

        match Self::read_cached(models_dir) {
            Some(listing) => {
                tracing::warn!(
                    url,
                    error = %failure,
                    entries = listing.files.len(),
                    "the published model listing could not be read; using the copy kept from a previous run"
                );
                Arc::new(listing)
            }
            None => {
                let listing = Self::compiled_in();
                tracing::warn!(
                    url,
                    error = %failure,
                    entries = listing.files.len(),
                    "neither the published model listing nor a kept copy could be read; using the one compiled into this build"
                );
                Arc::clone(listing)
            }
        }
    }

    /// Every published file backing the artifact named `id`, in the order the listing names them.
    ///
    /// Matched on the `<id>.onnx` prefix rather than on `<id>`, which is what groups a split model's graph and its
    /// external-weights blob under one artifact while keeping a future `up_osaka_fp16_v2` out of `up_osaka_fp16`'s
    /// file set.
    ///
    /// Empty where the listing names the artifact nowhere, which is the refusal the install reports.
    pub(crate) fn files_for(&self, id: &str) -> Vec<&Published> {
        let prefix = format!("{id}.onnx");

        self.files.iter().filter(|file| file.name.starts_with(&prefix)).collect()
    }

    /// Reads the copy cached from the last successful live read, or `None` when there is nothing there to use.
    ///
    /// Missing, unreadable, half-written, undecodable and empty all collapse to the same answer, exactly as the
    /// on-disk install record's own reader does: each one costs a fall-through to the manifest compiled into the
    /// binary rather than an error, because a failed cache read is not a reason to refuse an install that the next
    /// source can still verify.
    ///
    /// **An empty cache counts as absent.** A listing naming no files would answer every lookup with "no published
    /// hash", turning a cache written by some earlier failure into a blanket refusal.
    pub(crate) fn read_cached(dir: &Path) -> Option<Self> {
        let bytes = std::fs::read(dir.join(CACHE_NAME)).ok()?;
        let listing: Self = serde_json::from_slice(&bytes).ok()?;

        (!listing.files.is_empty()).then_some(listing)
    }

    /// Writes this listing into `dir` as the cached copy, atomically.
    ///
    /// # Errors
    ///
    /// Returns an [`io::Error`] if the listing cannot be serialized, written or renamed into place.
    pub(crate) fn write_cached(&self, dir: &Path) -> io::Result<()> {
        let json = serde_json::to_vec(self).map_err(io::Error::other)?;

        // Atomic because the read above has no way to tell a truncated JSON document from a corrupt one and treats
        // both as absent: a write interrupted in place would silently cost the fallback it exists to provide. The
        // temporary sits beside the target so the rename is a same-filesystem operation, and therefore atomic.
        let temporary = dir.join(format!("{CACHE_NAME}.tmp"));
        std::fs::write(&temporary, json)?;
        std::fs::rename(&temporary, dir.join(CACHE_NAME))
    }

    /// Parses one page of a Hugging Face tree listing, dropping every entry that publishes no hash.
    ///
    /// # Errors
    ///
    /// Returns the [`serde_json::Error`] if the body is not a JSON array of tree entries. A page that parses to no
    /// usable entry at all is not an error.
    pub(crate) fn parse_tree_page(body: &str) -> Result<Vec<Published>, serde_json::Error> {
        let entries: Vec<Entry> = serde_json::from_str(body)?;

        // Possibly empty, and not an error: no usable entry is what a directory of unpublished files looks like, and it
        // is the resolution over the three sources that decides what to do about an artifact nobody named.
        Ok(entries
            .into_iter()
            .filter_map(|entry| {
                // An entry with no LFS object id is **dropped rather than admitted with an empty hash**. Hugging Face
                // stores a small file as an ordinary git blob, where the `lfs` object is simply absent, and a directory
                // entry carries none either; the reference implementation admits both and builds a source with an
                // empty expected hash, which is a silent unverified install. Dropping them here is what makes such a
                // file fall through to the remaining sources and, failing those, be refused.
                let oid = entry.lfs.map(|lfs| lfs.oid).filter(|oid| !oid.is_empty())?;
                // The base name, because the path is a repository path and every consumer wants a file name. `None`
                // for a path that ends in a separator, which names no file.
                let name = entry.path.rsplit('/').next().filter(|name| !name.is_empty())?;

                Some(Published { name: name.to_string(), size: entry.size, sha256: oid })
            })
            .collect())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::deps::test_server::{Reply, TestServer};
    use crate::logging;

    // Recorded rather than written by hand so the parse is exercised against the shape the endpoint actually serves:
    // the suite catches a change to it at upgrade time instead of leaving it to be discovered at run time.
    /// The recorded `models/` listing, as the live endpoint returned it — reformatted for legibility and otherwise
    /// unaltered.
    pub(crate) const TREE_MODELS: &str = include_str!("testdata/tree_models.json");

    /// The recorded listing of the repository's root, which is where the entries that publish *no* hash actually live:
    /// two directories and three files kept as ordinary git blobs.
    pub(crate) const TREE_ROOT: &str = include_str!("testdata/tree_root.json");

    /// The published files the recorded `models/` listing reduces to.
    pub(crate) fn published() -> Vec<Published> {
        Listing::parse_tree_page(TREE_MODELS).unwrap()
    }

    /// One page of a listing, naming `files` as `models/<name>` with a plausible hash apiece.
    fn page(files: &[&str]) -> String {
        let entries: Vec<String> = files
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let oid = format!("{:064x}", index + 1);
                format!(r#"{{"type":"file","size":{},"path":"models/{name}","lfs":{{"oid":"{oid}"}}}}"#, index + 1)
            })
            .collect();

        format!("[{}]", entries.join(","))
    }

    #[test]
    fn the_listing_endpoint_names_the_published_models_directory() {
        // Not a `limit` in sight, unlike the bound the reference implementation passes.
        assert_eq!(TREE_URL, "https://huggingface.co/api/models/vegidio/open-photo-ai/tree/main/models");
        assert!(!TREE_URL.contains("limit"), "the listing URL carries a bound on how many models exist");
    }

    #[tokio::test]
    async fn a_listing_spanning_two_responses_is_read_to_exhaustion() {
        // The two halves of one 8x Kyoto, split across pages: an artifact named only in the second has to resolve as
        // readily as one named in the first, or an operation would install half of itself.
        let first = page(&["up_kyoto_4x_fp32.onnx"]);
        let second = page(&["up_kyoto_2x_fp32.onnx"]);
        let server = TestServer::start(vec![
            Reply::Listing { body: first, more: true },
            Reply::Listing { body: second, more: false },
        ])
        .await;

        let listing = Listing::read_live(&format!("{}/tree/main/models", server.base_url)).await.unwrap();

        let named: Vec<&str> = listing.files.iter().map(|file| file.name.as_str()).collect();
        assert_eq!(named, vec!["up_kyoto_4x_fp32.onnx", "up_kyoto_2x_fp32.onnx"]);
        assert_eq!(server.requests(), 2, "the second page was not requested");
    }

    #[tokio::test]
    async fn a_listing_that_fits_in_one_response_is_read_in_one_request() {
        let only = page(&["dn_stockholm_fp32.onnx"]);
        let server = TestServer::start(vec![Reply::Listing { body: only, more: false }]).await;

        let listing = Listing::read_live(&server.base_url).await.unwrap();

        assert_eq!(listing.files.len(), 1);
        assert_eq!(server.requests(), 1, "a page advertising no successor was followed anyway");
    }

    #[tokio::test]
    async fn a_page_advertising_one_already_read_ends_the_walk_rather_than_looping() {
        // A server whose `next` points back at a page already read. Not a bound on how many models exist — the walk
        // ends because the target repeats, at whatever length that happens.
        let body = page(&["sh_moscow_fp32.onnx"]);
        let server = TestServer::start(vec![
            Reply::Listing { body: body.clone(), more: true },
            Reply::Listing { body: body.clone(), more: true },
            Reply::Listing { body, more: true },
        ])
        .await;

        let listing = Listing::read_live(&format!("{}/tree/next", server.base_url)).await.unwrap();

        // The first page and the one it advertises, which is itself — so the walk stops there.
        assert_eq!(server.requests(), 1);
        assert_eq!(listing.files.len(), 1);
    }

    #[tokio::test]
    async fn a_page_that_cannot_be_read_fails_naming_the_page_rather_than_returning_a_short_listing() {
        // A partial read must not look like a complete one: a listing missing its tail reports a published model as
        // having no known hash, which refuses an install that should have succeeded.
        let first = page(&["up_kyoto_4x_fp32.onnx"]);
        let server = TestServer::start(vec![
            Reply::Listing { body: first, more: true },
            Reply::Listing { body: "not a tree listing".to_string(), more: false },
        ])
        .await;
        let url = format!("{}/tree/main/models", server.base_url);

        let error = Listing::read_live(&url).await.unwrap_err();

        match error {
            ReadError::Malformed { url: named, .. } => assert!(named.ends_with("/tree/next"), "{named}"),
            other => panic!("expected Malformed, got {other:?}"),
        }
    }

    #[test]
    fn the_compiled_in_listing_parses_through_the_same_deserializer_as_the_live_one() {
        // Same type, same derive, same format: the point is that there is one parser rather than three that can
        // disagree. Reached through `compiled_in`, which is the accessor the resolution uses.
        let listing = Listing::compiled_in();

        assert!(!listing.files.is_empty(), "the compiled-in listing names nothing, so it can verify nothing");
        for file in &listing.files {
            assert_eq!(file.sha256.len(), 64, "{} has no full SHA-256", file.name);
            assert!(file.size > 0, "{} is compiled in as empty", file.name);
        }
    }

    #[test]
    fn the_compiled_in_listing_is_parsed_once_however_often_it_is_asked_for() {
        // Held behind a `OnceLock`, so a few hundred entries are not re-parsed per install.
        assert!(std::ptr::eq(Listing::compiled_in(), Listing::compiled_in()));
    }

    #[test]
    fn every_artifact_the_compiled_in_listing_names_is_one_this_build_can_ask_for() {
        // The drift check: a variant renamed in `models/` orphans an entry here, and an orphaned entry is a hash for
        // a file nothing can request. **The converse is deliberately not asserted** - `models/` can name artifacts
        // that were never published, a precision a variant does not ship, and refusing those is what the resolution's
        // last step is for.
        let nameable = crate::models::test_support::every_artifact_name();

        for file in &Listing::compiled_in().files {
            let artifact = file.name.split_once(".onnx").map_or(file.name.as_str(), |(stem, _)| stem);

            assert!(
                nameable.contains(artifact),
                "the compiled-in listing names {}, which no operation in this build can ask for",
                file.name
            );
        }
    }

    #[test]
    fn the_compiled_in_listing_covers_both_halves_of_a_split_model() {
        // An artifact published as a graph plus a weights blob has to have a hash for *each*: the compiled-in copy is
        // what an offline first run verifies against, and a missing blob there is an install that cannot complete.
        let named: Vec<&str> = Listing::compiled_in().files.iter().map(|file| file.name.as_str()).collect();

        assert!(named.contains(&"up_osaka_fp16.onnx"));
        assert!(named.contains(&"up_osaka_fp16.onnx.data"));
    }

    #[tokio::test]
    async fn a_reachable_listing_is_the_one_used_and_is_cached_for_a_later_run() {
        let body = page(&["up_kyoto_4x_fp32.onnx"]);
        let server = TestServer::start(vec![Reply::Listing { body, more: false }]).await;
        let dir = tempfile::tempdir().unwrap();

        let listing = Listing::resolve_once(&server.base_url, dir.path()).await;

        assert_eq!(listing.files_for("up_kyoto_4x_fp32").len(), 1, "the live listing was not the one used");
        assert_eq!(
            Listing::read_cached(dir.path()),
            Some((*listing).clone()),
            "a successful read left nothing for a run that cannot reach the listing"
        );
    }

    #[tokio::test]
    async fn an_unreachable_listing_falls_back_to_the_cached_copy() {
        // The case the cache exists for: the second source answers, and the install proceeds verified against hashes
        // that were published rather than against nothing.
        let dir = tempfile::tempdir().unwrap();
        let cached = Listing::new(vec![Published {
            name: "dn_stockholm_fp32.onnx".to_string(),
            size: 11,
            sha256: "bb".to_string(),
        }]);
        cached.write_cached(dir.path()).unwrap();

        let listing = Listing::resolve_once("http://127.0.0.1:9/tree", dir.path()).await;

        assert_eq!(*listing, cached);
    }

    #[tokio::test]
    async fn a_first_run_with_no_network_at_all_falls_back_to_the_manifest_compiled_in() {
        // The third source, and the reason there is no fourth: a machine that has never reached the listing still
        // verifies every model published when the binary was built.
        let dir = tempfile::tempdir().unwrap();

        let listing = Listing::resolve_once("http://127.0.0.1:9/tree", dir.path()).await;

        assert_eq!(&listing, Listing::compiled_in());
        assert_eq!(listing.files_for("up_kyoto_4x_fp32").len(), 1);
    }

    #[tokio::test]
    async fn an_artifact_no_source_names_resolves_to_no_files_at_all() {
        // The fourth outcome. Not a source with an empty hash - nothing to install against, which is what the install
        // refuses on rather than transferring unverified.
        let dir = tempfile::tempdir().unwrap();

        let listing = Listing::resolve_once("http://127.0.0.1:9/tree", dir.path()).await;

        assert!(listing.files_for("dn_nowhere_fp32").is_empty());
    }

    #[tokio::test]
    async fn an_unreadable_listing_and_an_undecodable_cache_still_reach_the_compiled_in_manifest() {
        // Both of the first two sources failing is not an error: each is a fall-through, so the worst case is an older
        // set of hashes rather than an unverified install.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(CACHE_NAME), b"half a document").unwrap();

        let listing = Listing::resolve_once("http://127.0.0.1:9/tree", dir.path()).await;

        assert_eq!(&listing, Listing::compiled_in());
    }

    #[tokio::test]
    async fn two_concurrent_first_installs_read_the_listing_once() {
        // The memoized path, and the only test that touches it: `RESOLVED` is process-global, so a second test
        // resolving through it would be handed whatever this one resolved. Everything else above drives
        // `resolve_once`, which is the same resolution without the memoization.
        let body = page(&["up_kyoto_4x_fp32.onnx"]);
        let server = TestServer::start(vec![Reply::Listing { body, more: false }]).await;
        let dir = tempfile::tempdir().unwrap();

        let (first, second) = tokio::join!(
            Listing::resolve(&server.base_url, dir.path()),
            Listing::resolve(&server.base_url, dir.path())
        );

        assert_eq!(server.requests(), 1, "the listing was read twice");
        assert!(Arc::ptr_eq(&first, &second), "two installs were handed two copies of the listing");
        assert_eq!(first.files_for("up_kyoto_4x_fp32").len(), 1);

        // And a later install reuses it rather than reading again — including one naming a different listing, which is
        // what "the first listing a process reads is the one it uses" means.
        let later = Listing::resolve("http://127.0.0.1:9/tree", dir.path()).await;
        assert_eq!(server.requests(), 1, "a later install read the listing again");
        assert!(Arc::ptr_eq(&first, &later));
    }

    #[test]
    fn a_cached_listing_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let listing = Listing::new(published());

        listing.write_cached(dir.path()).unwrap();

        assert_eq!(Listing::read_cached(dir.path()), Some(listing));
        assert!(
            !dir.path().join(format!("{CACHE_NAME}.tmp")).exists(),
            "the atomic write left its temporary behind"
        );
    }

    #[test]
    fn a_cache_written_beside_the_per_model_directories_is_not_inside_one() {
        // What keeps it out of every install's recorded file list: `record_tree` and `empty_dir` are only ever handed
        // `models/<artifact-id>/`, so a file at the models directory's own level is untouched by an install.
        assert!(!CACHE_NAME.contains('/'), "the cache would sit inside a model's own directory");
        assert_ne!(CACHE_NAME, crate::deps::manifest::MANIFEST_NAME, "the two records share a name");
    }

    #[test]
    fn a_missing_half_written_or_undecodable_cache_all_read_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Listing::read_cached(dir.path()), None, "missing");

        // Exactly what a process killed mid-write would leave if the write were not atomic: valid JSON up to the cut.
        let whole = serde_json::to_string(&Listing::new(published())).unwrap();
        std::fs::write(dir.path().join(CACHE_NAME), &whole[..whole.len() / 2]).unwrap();
        assert_eq!(Listing::read_cached(dir.path()), None, "half written");

        std::fs::write(dir.path().join(CACHE_NAME), b"not json at all").unwrap();
        assert_eq!(Listing::read_cached(dir.path()), None, "undecodable");

        // A document of the wrong shape, which is what a cache written by an older format would be.
        std::fs::write(dir.path().join(CACHE_NAME), br#"{"files":[]}"#).unwrap();
        assert_eq!(Listing::read_cached(dir.path()), None, "another shape");
    }

    #[test]
    fn an_empty_cache_reads_as_absent_rather_than_as_a_manifest_naming_nothing() {
        // The difference matters: absent falls through to the manifest compiled into the binary, while a listing
        // naming no files would answer every lookup with "no published hash" and refuse every install.
        let dir = tempfile::tempdir().unwrap();

        std::fs::write(dir.path().join(CACHE_NAME), b"[]").unwrap();
        assert_eq!(Listing::read_cached(dir.path()), None, "an empty array");

        std::fs::write(dir.path().join(CACHE_NAME), b"").unwrap();
        assert_eq!(Listing::read_cached(dir.path()), None, "an empty file");
    }

    #[test]
    fn writing_the_cache_again_replaces_the_previous_copy() {
        // A later read's listing is the one kept: the cache is a copy of the last successful read, not an accumulation
        // of every read.
        let dir = tempfile::tempdir().unwrap();
        Listing::new(published()).write_cached(dir.path()).unwrap();

        let replacement = Listing::new(vec![Published {
            name: "dn_stockholm_fp32.onnx".to_string(),
            size: 7,
            sha256: "aa".to_string(),
        }]);
        replacement.write_cached(dir.path()).unwrap();

        assert_eq!(Listing::read_cached(dir.path()), Some(replacement));
    }

    #[tokio::test]
    async fn a_listing_that_cannot_be_reached_at_all_fails_naming_the_url() {
        // Port 9 is the discard port: nothing accepts, so this is the no-network case rather than a bad response.
        let url = "http://127.0.0.1:9/tree/main/models";

        let error = Listing::read_live(url).await.unwrap_err();

        match error {
            ReadError::Request { url: named, .. } => assert_eq!(named, url),
            other => panic!("expected Request, got {other:?}"),
        }
    }

    #[test]
    fn a_nested_path_is_reduced_to_the_file_name_it_is_stored_under() {
        let files = published();

        assert!(!files.is_empty(), "the recorded listing parsed to nothing, so this test would pass vacuously");
        for file in &files {
            assert!(!file.name.contains('/'), "{} is still a repository path rather than a file name", file.name);
        }
        assert!(
            files.iter().any(|file| file.name == "up_kyoto_4x_fp32.onnx"),
            "the recorded listing named no artifact this project can ask for"
        );
    }

    #[test]
    fn every_published_file_carries_a_size_and_a_sha256() {
        for file in published() {
            assert_eq!(file.sha256.len(), 64, "{} has no full SHA-256", file.name);
            assert!(file.sha256.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{}", file.name);
            assert!(file.size > 0, "{} is published as empty", file.name);
        }
    }

    #[test]
    fn a_models_external_weights_blob_is_published_in_its_own_right() {
        // A model too large for the 2 GB protobuf limit keeps its weights beside the graph. Both are entries of their
        // own with hashes of their own, which is what lets every file backing an artifact be verified rather than
        // only the one the naming convention predicts.
        let files = published();
        let named = |name: &str| files.iter().any(|file| file.name == name);

        assert!(named("up_osaka_fp16.onnx"), "the graph half of a split model is missing");
        assert!(named("up_osaka_fp16.onnx.data"), "the weights half of a split model is missing");
    }

    #[test]
    fn an_entry_with_no_lfs_object_id_is_dropped_rather_than_admitted_with_an_empty_hash() {
        // The root listing really does contain both shapes: `.gitattributes`, `.gitignore` and `README.md` are stored
        // as ordinary git blobs, and `models` and `testdata` are directories. Neither publishes an LFS object id, so
        // neither may become a source with an empty expected hash.
        let files = Listing::parse_tree_page(TREE_ROOT).unwrap();

        assert!(files.is_empty(), "an entry publishing no hash was admitted: {files:?}");
    }

    #[test]
    fn an_empty_object_id_counts_as_no_hash_at_all() {
        // The same rule one step further in: an `lfs` object that is present but whose `oid` is empty is what a
        // verified install would be reduced to comparing against nothing.
        let body = r#"[{"type":"file","size":10,"path":"models/dn_nowhere_fp32.onnx","lfs":{"oid":""}}]"#;

        assert!(Listing::parse_tree_page(body).unwrap().is_empty());
    }

    #[test]
    fn an_unknown_field_upstream_is_not_a_parse_failure() {
        // The parse names the three fields it reads and ignores the rest, so a field added to the endpoint's response
        // cannot turn a working install into a refusal.
        let body = r#"[{"type":"file","size":10,"path":"models/a.onnx","lfs":{"oid":"aa","size":10},"newField":7}]"#;

        assert_eq!(
            Listing::parse_tree_page(body).unwrap(),
            vec![Published { name: "a.onnx".to_string(), size: 10, sha256: "aa".to_string() }]
        );
    }

    #[test]
    fn a_body_that_is_not_a_tree_listing_is_an_error_rather_than_an_empty_manifest() {
        // The distinction the resolution rests on: an empty manifest means "nothing is published", which is a hash
        // refusal, while an unparseable body means the source failed and the next one should be tried.
        for body in ["not json at all", r#"{"error":"Repository not found"}"#, ""] {
            assert!(Listing::parse_tree_page(body).is_err(), "{body:?} parsed as a listing");
        }
    }

    #[test]
    fn a_listing_round_trips_through_the_one_format_all_three_sources_share() {
        let listing = Listing::new(published());

        let json = serde_json::to_string(&listing).unwrap();
        assert!(json.starts_with('['), "the serialized form is not the bare array all three sources use");
        assert_eq!(serde_json::from_str::<Listing>(&json).unwrap(), listing);
    }

    #[tokio::test]
    async fn the_source_the_listing_resolved_from_is_recorded_whenever_it_was_not_the_first_choice() {
        // Three resolutions, one per source. The successful read is a `debug` and the two fallbacks are `warn`s: a
        // run working from a listing compiled in months ago behaves exactly like a current one right up to the point
        // where it cannot find a model that exists, and this is where the two are told apart.

        // The live listing.
        // Two scripted replies, because the resolution below is driven twice — once at `debug` and once at `info`.
        let served = || Reply::Listing { body: page(&["up_kyoto_4x_fp32.onnx"]), more: false };
        let server = TestServer::start(vec![served(), served()]).await;
        let dir = tempfile::tempdir().unwrap();

        let (log, listing) =
            logging::records_of("debug", || async { Listing::resolve_once(&server.base_url, dir.path()).await }).await;

        let live = log
            .lines()
            .find(|line| line.contains("read the published model listing"))
            .unwrap_or_else(|| panic!("the live read was not recorded:\n{log}"));
        assert!(live.contains("level=DEBUG"), "the ordinary case is above debug: {live}");
        assert!(live.contains(&format!("entries={}", listing.files.len())), "{live}");

        // At the default level the ordinary case is silent, and nothing claims a fallback.
        let (ordinary, _) =
            logging::records_of("info", || async { Listing::resolve_once(&server.base_url, dir.path()).await }).await;
        assert!(!ordinary.contains("read the published model listing"), "{ordinary}");
        assert!(!ordinary.contains("using the copy kept"), "{ordinary}");

        // The cached copy, which is what the previous read left behind.
        let (cached_log, _) = logging::records_of("info", || async {
            Listing::resolve_once("http://127.0.0.1:9/tree", dir.path()).await
        })
        .await;

        let cached = cached_log
            .lines()
            .find(|line| line.contains("using the copy kept from a previous run"))
            .unwrap_or_else(|| panic!("the cached fallback was not recorded:\n{cached_log}"));
        assert!(cached.contains("level=WARN"), "a fallback is not a warning: {cached}");
        assert!(cached.contains("error="), "the record does not say what failed: {cached}");

        // The compiled-in listing, reached with no network and an undecodable cache.
        let empty = tempfile::tempdir().unwrap();
        std::fs::write(empty.path().join(CACHE_NAME), b"half a document").unwrap();

        let (compiled_log, compiled) = logging::records_of("info", || async {
            Listing::resolve_once("http://127.0.0.1:9/tree", empty.path()).await
        })
        .await;
        assert_eq!(&compiled, Listing::compiled_in());

        let built_in = compiled_log
            .lines()
            .find(|line| line.contains("using the one compiled into this build"))
            .unwrap_or_else(|| panic!("the compiled-in fallback was not recorded:\n{compiled_log}"));
        assert!(built_in.contains("level=WARN"), "{built_in}");
        assert!(built_in.contains(&format!("entries={}", compiled.files.len())), "{built_in}");
    }

    #[tokio::test]
    async fn a_listing_that_cannot_be_cached_is_a_warning_and_not_a_failed_resolution() {
        // The fallback it costs is a *later* run's, so this run is unaffected — which is why it is a warning beside a
        // successful resolution rather than an error.
        let body = page(&["up_kyoto_4x_fp32.onnx"]);
        let server = TestServer::start(vec![Reply::Listing { body, more: false }]).await;
        let root = tempfile::tempdir().unwrap();

        // A models directory that is not one, so the write and the rename both fail on every platform.
        let models = root.path().join("models");
        std::fs::write(&models, b"not a directory").unwrap();

        let (log, listing) =
            logging::records_of("info", || async { Listing::resolve_once(&server.base_url, &models).await }).await;

        assert_eq!(listing.files_for("up_kyoto_4x_fp32").len(), 1, "the live listing was not the one used");

        let uncached = log
            .lines()
            .find(|line| line.contains("could not be written back to the cache"))
            .unwrap_or_else(|| panic!("the failed write-back was not recorded:\n{log}"));
        assert!(uncached.contains("level=WARN"), "{uncached}");
        assert!(uncached.contains("error="), "{uncached}");
    }
}
