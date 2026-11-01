//! A local stand-in for the Hugging Face Hub, for the initialization and model suites: the `revision`, `tree` and
//! `resolve` requests `rust-sak`'s installers make, over plain HTTP/1.1 on an ephemeral port, each one recorded.
//!
//! Every `deps/<name>/<version>` folder publishes [`FIXTURE`] as this platform's archive. The `models` folder publishes
//! what [`HfServer::put_model`] put there, with ranged downloads, and the switches the model suites need: bytes other
//! than the listing describes, a download that stalls, and an API that is down.

use std::collections::BTreeMap;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rust_sak::ai::{Folder, Source};

use crate::progress::Dependency as Which;

/// The fixture every folder publishes: a runtime library named for each platform, beside `nested/libcudart.so.13`.
pub(crate) const FIXTURE: &[u8] = include_bytes!("testdata/runtime.7z");

/// Bytes standing in for a model file, large enough (70 KiB) for an interrupted transfer to leave a partial: nothing
/// here opens them.
pub(crate) const MODEL: &[u8] = include_bytes!("testdata/tree.7z");

/// The files the fixture installs, by path relative to its folder.
pub(crate) const INSTALLED: [&str; 4] = [
    "libonnxruntime.1.30.0.dylib",
    "libonnxruntime.so.1.30.0",
    "nested/libcudart.so.13",
    "onnxruntime.dll",
];

const REPO: &str = "test/general";
const COMMIT: &str = "501d7b94e6d22762667e213994b0242ccdb04588";

#[derive(Default)]
struct State {
    requests: Vec<String>,
    /// Folder names (`cuda`, `onnx`) whose listing holds an archive for another platform only.
    unpublished: Vec<String>,
    /// Folder names whose archive is served as other bytes than the listing describes.
    corrupt: Vec<String>,
    /// The `models` folder, by file name.
    models: BTreeMap<String, Vec<u8>>,
    /// Model files served as these bytes instead of what the listing describes.
    models_instead: BTreeMap<String, Vec<u8>>,
    /// Model files whose download sends this many bytes, then holds the connection open.
    stalled: BTreeMap<String, usize>,
    /// Every API request answers 503.
    api_down: bool,
}

/// The server. It runs until the process ends.
#[derive(Clone)]
pub(crate) struct HfServer {
    base: String,
    state: Arc<Mutex<State>>,
    /// The dependency root the plans of [`plan`](Self::plan) install under, removed with the last clone.
    root: Arc<tempfile::TempDir>,
}

impl HfServer {
    /// Serve [`FIXTURE`] as the archive for this platform in every `deps/<name>/<version>` folder, and an empty
    /// `models` folder.
    pub(crate) fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        let shared = Arc::clone(&state);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let state = Arc::clone(&shared);
                // A thread per connection, so a stalled download holds only its own.
                std::thread::spawn(move || serve(stream, &state));
            }
        });
        Self { base, state, root: Arc::new(tempfile::tempdir().unwrap()) }
    }

    /// Publish nothing for this platform in `which`'s folder.
    pub(crate) fn unpublish(&self, which: &Which) {
        let name = crate::deps::folders::source(which).name().to_owned();
        self.state.lock().unwrap().unpublished.push(name);
    }

    /// Serve `which`'s archive as bytes other than the listing describes.
    pub(crate) fn corrupt(&self, which: &Which) {
        let name = crate::deps::folders::source(which).name().to_owned();
        self.state.lock().unwrap().corrupt.push(name);
    }

    /// The dependency root this server's plans install under.
    pub(crate) fn root(&self) -> &Path {
        self.root.path()
    }

    /// The folder `which` would be published in at `version` on this server.
    pub(crate) fn source_at(&self, which: &Which, version: &str) -> Source {
        let name = crate::deps::folders::source(which).name().to_owned();
        Source::parse(&format!("{}/{REPO}/tree/main/deps/{name}/{version}", self.base)).unwrap()
    }

    /// The folder `which` is published in on this server, at the production folder's version.
    pub(crate) fn source(&self, which: &Which) -> Source {
        self.source_at(which, crate::deps::folders::source(which).version())
    }

    /// `which` as the initialization installs it, from this server.
    pub(crate) fn dependency(&self, which: &Which) -> rust_sak::ai::Dependency {
        rust_sak::ai::Dependency { source: self.source(which), ..crate::deps::folders::dependency(which) }
    }

    /// A plan installing `which`, in that order, from this server under [`root`](Self::root).
    pub(crate) fn plan(&self, which: &[Which]) -> crate::setup::Plan {
        crate::setup::Plan {
            root: self.root().to_path_buf(),
            dependencies: which.iter().map(|which| (which.clone(), self.dependency(which))).collect(),
        }
    }

    /// [`plan`](Self::plan), with every source on a port nothing listens on: a machine with no network.
    pub(crate) fn offline_plan(&self, which: &[Which]) -> crate::setup::Plan {
        let closed = closed_base();
        let mut plan = self.plan(which);
        for (_, dependency) in &mut plan.dependencies {
            let url =
                format!("{closed}/{REPO}/tree/main/deps/{}/{}", dependency.source.name(), dependency.source.version());
            dependency.source = Source::parse(&url).unwrap();
        }
        plan
    }

    /// The `models` folder on this server.
    pub(crate) fn models(&self) -> Folder {
        Folder::parse(&format!("{}/{REPO}/tree/main/models", self.base)).unwrap()
    }

    /// The `models` folder on a port nothing listens on: a machine with no network.
    pub(crate) fn offline_models() -> Folder {
        Folder::parse(&format!("{}/{REPO}/tree/main/models", closed_base())).unwrap()
    }

    /// Publish `bytes` as `models/<name>`, replacing what was there.
    pub(crate) fn put_model(&self, name: &str, bytes: &[u8]) {
        self.state.lock().unwrap().models.insert(name.to_owned(), bytes.to_vec());
    }

    /// Serve `models/<name>` as `bytes`, while the listing still describes what [`put_model`](Self::put_model) put.
    pub(crate) fn corrupt_model(&self, name: &str, bytes: &[u8]) {
        self.state.lock().unwrap().models_instead.insert(name.to_owned(), bytes.to_vec());
    }

    /// Send `bytes` of each download of `models/<name>`, then hold the connection open, until
    /// [`unstall_model`](Self::unstall_model).
    pub(crate) fn stall_model(&self, name: &str, bytes: usize) {
        self.state.lock().unwrap().stalled.insert(name.to_owned(), bytes);
    }

    /// Serve `models/<name>` whole again.
    pub(crate) fn unstall_model(&self, name: &str) {
        self.state.lock().unwrap().stalled.remove(name);
    }

    /// Answer every API request (the revision and the listings) with 503, or stop doing so.
    pub(crate) fn set_api_down(&self, down: bool) {
        self.state.lock().unwrap().api_down = down;
    }

    /// How many requests have been made.
    pub(crate) fn requests(&self) -> usize {
        self.state.lock().unwrap().requests.len()
    }

    /// Every download, in order.
    pub(crate) fn downloads(&self) -> Vec<String> {
        let state = self.state.lock().unwrap();
        state.requests.iter().filter(|path| path.contains("/resolve/")).cloned().collect()
    }

    /// How many times the `models` folder was listed.
    pub(crate) fn model_listings(&self) -> usize {
        let state = self.state.lock().unwrap();
        state.requests.iter().filter(|path| path.contains("/tree/") && path.contains("/models")).count()
    }
}

/// A base URL nothing listens on.
fn closed_base() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("http://{}", listener.local_addr().unwrap())
}

/// The archive name this platform picks from a folder of `name`.
fn asset(name: &str) -> String {
    let arch = if std::env::consts::ARCH == "aarch64" { "arm64" } else { "x64" };
    format!("{name}_{}_{arch}.7z", std::env::consts::OS)
}

/// The dependency name of a request's `deps/<name>/<version>/...` path.
fn name_in(path: &str) -> &str {
    path.split_once("/deps/").and_then(|(_, rest)| rest.split('/').next()).unwrap_or("")
}

/// The model file name of a request's `<commit>/models/<name>` path, or `None` for any other.
fn model_in(rest: &str) -> Option<&str> {
    rest.split_once('/').and_then(|(_, folder)| folder.strip_prefix("models/"))
}

fn serve(mut stream: TcpStream, state: &Mutex<State>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
    let mut range = None;
    let mut header = String::new();
    while reader.read_line(&mut header).is_ok_and(|read| read > 2) {
        if let Some(from) = header.to_ascii_lowercase().strip_prefix("range: bytes=") {
            range = from.trim().trim_end_matches('-').parse::<usize>().ok();
        }
        header.clear();
    }
    let (response, stall) = {
        let mut state = state.lock().unwrap();
        state.requests.push(path.clone());
        answer(&path, range, &state)
    };
    let _ = stream.write_all(&response);
    let _ = stream.flush();
    if stall {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

/// The response to `path`, and whether the connection is then held open.
fn answer(path: &str, range: Option<usize>, state: &State) -> (Vec<u8>, bool) {
    if path.starts_with("/api/") && state.api_down {
        return (status(503), false);
    }
    let name = name_in(path);
    let body = if path.contains("/revision/") {
        format!(r#"{{"sha":"{COMMIT}"}}"#).into_bytes()
    } else if let Some((_, rest)) = path.split_once("/tree/") {
        // `<commit>/deps/<name>/<version>` or `<commit>/models`
        let folder = rest.split_once('/').map_or("", |(_, folder)| folder);
        if folder == "models" {
            if state.models.is_empty() {
                return (status(404), false);
            }
            models_listing(state).into_bytes()
        } else {
            let asset = if state.unpublished.iter().any(|unpublished| unpublished == name) {
                format!("{name}_plan9_x64.7z")
            } else {
                asset(name)
            };
            let sha256 = rust_sak::crypto::sha256_bytes(FIXTURE);
            format!(
                r#"[{{"type":"file","oid":"x","size":{size},"lfs":{{"oid":"{sha256}","size":{size}}},"path":"{folder}/{asset}"}}]"#,
                size = FIXTURE.len()
            )
            .into_bytes()
        }
    } else if let Some((_, rest)) = path.split_once("/resolve/") {
        if let Some(model) = model_in(rest) {
            return model_download(state, model, range);
        }
        if state.corrupt.iter().any(|corrupt| corrupt == name) {
            b"not the published archive".to_vec()
        } else {
            FIXTURE.to_vec()
        }
    } else {
        return (status(404), false);
    };
    let mut response =
        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
    response.extend_from_slice(&body);
    (response, false)
}

/// The `models` folder's listing, every file in LFS.
fn models_listing(state: &State) -> String {
    let entries: Vec<String> = state
        .models
        .iter()
        .map(|(name, bytes)| {
            format!(
                r#"{{"type":"file","oid":"x","size":{size},"lfs":{{"oid":"{sha256}","size":{size}}},"path":"models/{name}"}}"#,
                size = bytes.len(),
                sha256 = rust_sak::crypto::sha256_bytes(bytes),
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

/// A download of `models/<name>` from `range`, and whether the connection is then held open.
fn model_download(state: &State, name: &str, range: Option<usize>) -> (Vec<u8>, bool) {
    let Some(bytes) = state.models_instead.get(name).or_else(|| state.models.get(name)) else {
        return (status(404), false);
    };
    let from = range.unwrap_or(0).min(bytes.len());
    let body = &bytes[from..];
    let etag = format!("ETag: \"{}\"", rust_sak::crypto::sha256_bytes(bytes));
    let mut response = if range.is_some() {
        format!(
            "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {from}-{}/{}\r\nContent-Length: {}\r\n{etag}\r\nConnection: close\r\n\r\n",
            bytes.len().saturating_sub(1),
            bytes.len(),
            body.len()
        )
    } else {
        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n{etag}\r\nConnection: close\r\n\r\n", body.len())
    }
    .into_bytes();
    match state.stalled.get(name) {
        Some(after) => {
            response.extend_from_slice(&body[..(*after).min(body.len())]);
            (response, true)
        }
        None => {
            response.extend_from_slice(body);
            (response, false)
        }
    }
}

fn status(code: u16) -> Vec<u8> {
    format!("HTTP/1.1 {code} Status\r\nRetry-After: 0\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes()
}
