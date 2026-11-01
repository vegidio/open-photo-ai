//! A local stand-in for the Hugging Face Hub, for the initialization suites: the `revision`, `tree` and `resolve`
//! requests `rust-sak`'s installer makes, over plain HTTP/1.1 on an ephemeral port, each one recorded.

use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rust_sak::ai::Source;

use crate::progress::Dependency as Which;

/// The fixture every folder publishes: a runtime library named for each platform, beside `nested/libcudart.so.13`.
pub(crate) const FIXTURE: &[u8] = include_bytes!("testdata/runtime.7z");

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
    /// Serve [`FIXTURE`] as the archive for this platform in every `deps/<name>/<version>` folder.
    pub(crate) fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        let shared = Arc::clone(&state);
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    continue;
                }
                let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
                let mut header = String::new();
                while reader.read_line(&mut header).is_ok_and(|read| read > 2) {
                    header.clear();
                }
                let response = {
                    let mut state = shared.lock().unwrap();
                    state.requests.push(path.clone());
                    answer(&path, &state)
                };
                let _ = stream.write_all(&response);
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
        let closed = {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            format!("http://{}", listener.local_addr().unwrap())
        };
        let mut plan = self.plan(which);
        for (_, dependency) in &mut plan.dependencies {
            let url =
                format!("{closed}/{REPO}/tree/main/deps/{}/{}", dependency.source.name(), dependency.source.version());
            dependency.source = Source::parse(&url).unwrap();
        }
        plan
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

fn answer(path: &str, state: &State) -> Vec<u8> {
    let name = name_in(path);
    let body = if path.contains("/revision/") {
        format!(r#"{{"sha":"{COMMIT}"}}"#).into_bytes()
    } else if let Some((_, rest)) = path.split_once("/tree/") {
        // `<commit>/deps/<name>/<version>`
        let folder = rest.split_once('/').map_or("", |(_, folder)| folder);
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
    } else if path.contains("/resolve/") {
        if state.corrupt.iter().any(|corrupt| corrupt == name) {
            b"not the published archive".to_vec()
        } else {
            FIXTURE.to_vec()
        }
    } else {
        return b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
    };
    let mut response =
        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
    response.extend_from_slice(&body);
    response
}
