//! A throwaway local HTTP/1.1 server for the pipeline tests.
//!
//! It serves the committed 7z fixture the way GitHub releases serve a real archive — `ETag`, `Content-Length`, and
//! `Range` requests answered with a `206` — so the tests exercise the real transfer, verification and expansion path
//! without reaching the network. What it adds on top is the ability to lie: to serve bytes that do not hash to what
//! was pinned, or to cut a response short, which is how the recovery paths become testable at all.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// The archive every pipeline test installs: three files, one of them nested, mirroring the shape of a real runtime
/// archive (a library plus its execution providers).
pub(crate) const FIXTURE: &[u8] = include_bytes!("testdata/tree.7z");

/// What the fixture expands to, in the order [`crate::deps::manifest::record_tree`] reports it.
///
/// It is shaped like a real runtime archive — one large shared library beside two small providers, one of them nested.
/// The library's 64 KiB are incompressible, which is what makes the archive bigger than a transfer's write buffer (so
/// an interruption leaves real bytes to resume onto) and gives the expansion one file long enough to report progress
/// from inside.
pub(crate) const EXTRACTED: &[(&str, u64)] = &[
    ("nested/onnxruntime_providers_cuda.so", 8),
    ("onnxruntime.1.26.0.dylib", 65_536),
    ("onnxruntime_providers_shared.so", 6),
];

/// The library name a runtime row carries, for the descriptors that stand in for one.
///
/// Only the runtime names a library — a GPU library is a directory on the search path, not a file the loader is opened
/// against — so the rule lives here rather than being re-derived by each test module that builds a descriptor.
pub(crate) fn fixture_lib(progress: &crate::progress::Dependency) -> Option<&'static str> {
    matches!(progress, crate::progress::Dependency::Runtime).then_some("onnxruntime.1.26.0.dylib")
}

/// The source that serves `asset` for `version` from `server`, pinned to the fixture's real hash and size.
pub(crate) fn fixture_source(server: &TestServer, version: &str, asset: &str) -> crate::deps::release::Source {
    crate::deps::release::Source {
        url: format!("{}/{version}/{asset}", server.base_url),
        sha256: rust_sak::crypto::sha256_bytes(FIXTURE),
        size: FIXTURE.len() as u64,
    }
}

/// A model listing publishing each of `files` with the served fixture's real hash and size, so an install against the
/// test server verifies for real rather than against a hash nothing wrote.
///
/// Here beside [`fixture_source`], which is the same helper for a release archive and exists for the same reason: the
/// rule that a published entry must be pinned to the bytes the server actually hands back is what makes these installs
/// verify at all, and a second copy of it is a second thing to move when the fixture changes.
pub(crate) fn fixture_listing(files: &[String]) -> crate::deps::model::manifest::Listing {
    crate::deps::model::manifest::Listing::new(
        files
            .iter()
            .map(|name| crate::deps::model::manifest::Published {
                name: name.clone(),
                size: FIXTURE.len() as u64,
                sha256: rust_sak::crypto::sha256_bytes(FIXTURE),
            })
            .collect(),
    )
}

/// Asserts the fixture's three files are on disk under `dir`, and that the archive that produced them is not.
pub(crate) fn assert_extracted(dir: &Path, archive: &str) {
    for (path, size) in EXTRACTED {
        let file = dir.join(crate::deps::manifest::from_slash(path));
        assert!(file.exists(), "{path} is missing");
        assert_eq!(std::fs::metadata(&file).unwrap().len(), *size, "{path}");
    }

    assert!(!dir.join(archive).exists(), "the archive was left behind");
    assert!(
        !crate::deps::install::part_path(&dir.join(archive)).exists(),
        "a finished transfer left a partial"
    );
}

/// What the server should do with one request.
///
/// `Clone` rather than `Copy`, because a listing page carries its own body: a test builds one at run time from the
/// artifact names it cares about, and requiring `&'static str` here would have every such test leak one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reply {
    /// Serve the fixture, honouring `Range`.
    Fixture,
    /// Serve bytes that are not the fixture, so the pinned hash cannot match.
    Corrupt,
    /// Serve only the first `n` bytes of the fixture and close, leaving a resumable partial behind. The declared
    /// `Content-Length` is still the whole archive, so the client knows it was cut short.
    Truncated(usize),
    /// Serve the first `n` bytes of the fixture and then **hold the connection open**, sending no more.
    ///
    /// What a test about stopping a transfer needs and [`Truncated`](Self::Truncated) cannot give it: a transfer
    /// that is still running. Against a reply that ends, a test that cancels races the download finishing, and the
    /// race is one it would usually lose — the fixture is 70 KiB over a loopback socket.
    Stalled(usize),
    /// Serve `body` as one page of a JSON listing, advertising a next page in a `Link` header when `more`.
    ///
    /// The link points back at this server, so a client following it lands on the next scripted reply — which is how
    /// a listing spanning two responses is served without the test knowing the port in advance.
    Listing {
        /// The page's JSON body.
        body: String,
        /// Whether a further page is advertised.
        more: bool,
    },
}

/// A local server that answers a scripted sequence of requests.
///
/// Each connection consumes the next [`Reply`]; once the script runs out the fixture is served, so a test only has to
/// describe the responses it actually cares about.
#[derive(Debug)]
pub(crate) struct TestServer {
    /// The base URL to point a dependency descriptor at.
    pub(crate) base_url: String,
    /// How many requests have been answered, which is how "it resumed rather than restarting" is distinguished from
    /// "it never asked at all".
    requests: Arc<AtomicUsize>,
    /// Every `Range` header seen, in order — empty for a request that carried none.
    ranges: Arc<tokio::sync::Mutex<Vec<String>>>,
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl TestServer {
    /// Starts a server on an ephemeral port that answers `script` in order, then serves the fixture forever.
    pub(crate) async fn start(script: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());

        let requests = Arc::new(AtomicUsize::new(0));
        let ranges = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let (served, seen) = (Arc::clone(&requests), Arc::clone(&ranges));
        // The responder builds its `Link` header against this, so a paginated listing can point at itself.
        let base = base_url.clone();

        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };

                let request = read_request(&mut stream).await;
                let index = served.fetch_add(1, Ordering::SeqCst);
                seen.lock().await.push(range_of(&request));

                let reply = script.get(index).cloned().unwrap_or(Reply::Fixture);
                let stalled = matches!(reply, Reply::Stalled(_));
                respond(&mut stream, reply, range_start(&request), &base).await;

                if stalled {
                    // Held open by a task of its own rather than here, so the client stays mid-transfer while this
                    // loop goes on answering the requests that follow — the resume after a stop is one of them.
                    tokio::spawn(async move {
                        std::future::pending::<()>().await;
                        drop(stream);
                    });
                }
            }
        });

        Self { base_url, requests, ranges, handle }
    }

    /// How many requests the server has answered.
    pub(crate) fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }

    /// The `Range` header of each request, in order; an empty string where there was none.
    pub(crate) async fn ranges(&self) -> Vec<String> {
        self.ranges.lock().await.clone()
    }
}

/// Reads a request up to the blank line terminating its headers.
async fn read_request(stream: &mut TcpStream) -> String {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];

    while let Ok(n) = stream.read(&mut chunk).await {
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }

    String::from_utf8_lossy(&buf).into_owned()
}

/// The request's `Range` header value, or an empty string.
fn range_of(request: &str) -> String {
    request
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("range:"))
        .map(|line| line[6..].trim().to_string())
        .unwrap_or_default()
}

/// The first byte position a `Range: bytes=N-` header asked for, or `0`.
fn range_start(request: &str) -> u64 {
    range_of(request)
        .strip_prefix("bytes=")
        .and_then(|value| value.split('-').next())
        .and_then(|start| start.trim().parse().ok())
        .unwrap_or(0)
}

/// Writes the scripted response, honouring `offset` for the two replies that serve real fixture bytes.
async fn respond(stream: &mut TcpStream, reply: Reply, offset: u64, base_url: &str) {
    // A listing is served whole rather than ranged: it is a JSON document a client reads in one request, and the only
    // thing about it worth scripting is whether it advertises a page after this one.
    if let Reply::Listing { body, more } = &reply {
        let link = more.then(|| format!("Link: <{base_url}/tree/next>; rel=\"next\"\r\n")).unwrap_or_default();
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{link}Connection: close\r\n\r\n",
            body.len()
        );

        let _ = stream.write_all(head.as_bytes()).await;
        let _ = stream.write_all(body.as_bytes()).await;
        let _ = stream.flush().await;
        return;
    }

    let body: Vec<u8> = match reply {
        Reply::Fixture => FIXTURE.to_vec(),
        // Same length as the fixture, so only the hash gives it away — which is the point: a length check would have
        // caught a shorter body without ever exercising the verification.
        Reply::Corrupt => vec![b'x'; FIXTURE.len()],
        Reply::Truncated(n) => FIXTURE[..n].to_vec(),
        // The whole archive as far as the headers are concerned — the client is mid-transfer, not short-changed.
        Reply::Stalled(_) => FIXTURE.to_vec(),
        // Answered above, before the fixture bodies are built.
        Reply::Listing { .. } => unreachable!("a listing is served before this point"),
    };

    let total = FIXTURE.len() as u64;
    let offset = (offset as usize).min(body.len());
    let tail = &body[offset..];

    let head = if offset > 0 {
        format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {offset}-{}/{total}\r\nContent-Length: {}\r\nETag: \"fixture\"\r\nConnection: close\r\n\r\n",
            total - 1,
            total as usize - offset,
        )
    } else {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nETag: \"fixture\"\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n"
        )
    };

    // A stalled reply sends its opening bytes and stops there. The rest of the body is never written and the
    // connection is never closed, so the client sits in the middle of a transfer for as long as the test needs.
    let tail = match reply {
        Reply::Stalled(n) => &tail[..n.min(tail.len())],
        _ => tail,
    };

    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(tail).await;
    let _ = stream.flush().await;

    // A truncated reply closes here having sent fewer bytes than it declared, which is what leaves a resumable
    // partial on the client's disk.
}
