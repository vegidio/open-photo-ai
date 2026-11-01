//! Turning the installed ONNX Runtime into a loaded one.
//!
//! The load is `rust-sak`'s `ai::load_with`: it opens the library in the runtime's version folder, starts the one
//! environment every session is built under, registers the WebGPU plugin, and silences the runtime's logging at
//! process exit. What stays here is this application's policy around it: the environment's name, the logger that
//! counts out-of-memory marks ([`diagnostics`]), the record of what the load cost, and the latch that makes a failed
//! load fail every later initialization in the process.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rust_sak::ai::{self, LoadError, LoadOptions};

use crate::error::InitError;
use crate::telemetry::unit::{self, Outcome, Unit, unit_span};

pub(crate) mod diagnostics;

/// The first load failure in this process, if there has been one.
///
/// `rust-sak` lets a failed load be tried again. This application does not: the `initialization` capability requires
/// every later initialization in a process to fail once a load has, naming the library the first attempt was made
/// against, so a retry — which is exactly what the GUI's error boundary performs — reports
/// [`InitError::RuntimeUnavailable`] rather than a second, different error or a success against a runtime other than
/// the one the first attempt named.
static FAILED_LOAD: Mutex<Option<Failure>> = Mutex::new(None);

/// What the first failed load in this process was, and what it said.
#[derive(Debug)]
struct Failure {
    /// The library, or the folder, that attempt was made against.
    path: PathBuf,
    /// Its error's message.
    reason: String,
}

/// Loads the runtime installed in `runtime_dir` and starts the inference environment every model session is later
/// built under, named `name`. Whether the WebGPU plugin registered and offered a device is then what
/// `SupportedProviders::detect` reports.
///
/// Called from `Opai::initialize` and from nowhere else, which is what makes the latch the whole application's
/// rather than one caller's. A second start after a successful one returns at once and reuses the running
/// environment, whatever it names: a process runs on the first runtime it loads.
///
/// # Errors
///
/// Returns [`InitError::RuntimeLoad`] if the runtime is not installed there or its library will not load,
/// [`InitError::RuntimeStart`] if it loads and refuses to create an environment, and
/// [`InitError::RuntimeUnavailable`] if a load already failed in this process — see [`FAILED_LOAD`].
pub(crate) fn start(name: &str, runtime_dir: &Path) -> Result<(), InitError> {
    let span = unit_span!("runtime_load", library = %runtime_dir.display());

    // Started before the load, so the record below says what the load and the environment together cost: the single
    // slowest step of an initialization that has nothing to install.
    let started = std::time::Instant::now();
    let outcome = span.in_scope(|| load(name, runtime_dir, started));
    let duration = started.elapsed();

    // A failure's record is `initialize`'s, which the error is returned to. The span ends here either way.
    match &outcome {
        Ok(_) => unit::ended(Unit::RuntimeLoad, &span, duration, Outcome::Finished),
        Err(error) => unit::ended(Unit::RuntimeLoad, &span, duration, Outcome::Failed { kind: error.kind(), error }),
    }

    outcome
}

/// [`start`]'s body, inside its span. `started` is when the load began, for the record that closes it.
fn load(name: &str, runtime_dir: &Path, started: std::time::Instant) -> Result<(), InitError> {
    // Held across the load rather than only around the check, so two threads loading at once cannot both make a
    // first attempt. A poisoned lock means a previous caller panicked between the check and the record; the state
    // behind it is still what the next caller has to see.
    let mut failed = crate::task::lock(&FAILED_LOAD);

    if let Some(failure) = failed.as_ref() {
        return Err(InitError::RuntimeUnavailable { path: failure.path.clone(), reason: failure.reason.clone() });
    }

    // The logger is `ort`'s own `tracing` bridge in every respect a reader sees, plus a severity correction and the
    // out-of-memory mark a failed build is read against; see `diagnostics`.
    let options = LoadOptions { name, logger: Some(diagnostics::logger()) };
    #[allow(clippy::disallowed_methods, reason = "runtime::start is the sole loader; see FAILED_LOAD")]
    let loaded = ai::load_with(runtime_dir, &options);
    if let Err(error) = loaded {
        let error = load_failure(error, runtime_dir);
        if let InitError::RuntimeLoad { path, source } | InitError::RuntimeStart { path, source } = &error {
            *failed = Some(Failure { path: path.clone(), reason: source.to_string() });
        }
        return Err(error);
    }

    // The library this process is actually running on, by path. It is the single most useful line in a bug report
    // about inference: it says which install was loaded, and — because a process uses the first runtime it loads —
    // whether a later initialization under a different name is running on the library it installed or on this one.
    let library = ai::runtime_library(runtime_dir).unwrap_or_else(|| runtime_dir.to_path_buf());
    let duration = started.elapsed();
    tracing::info!(name, library = %library.display(), ?duration, "ONNX Runtime started");

    Ok(())
}

/// The initialization error a failed load is reported as, naming the path the load was attempted against, or
/// `runtime_dir` for an error that names none.
fn load_failure(error: LoadError, runtime_dir: &Path) -> InitError {
    match &error {
        LoadError::NotInstalled { path } | LoadError::Open { path, .. } => {
            InitError::RuntimeLoad { path: path.clone(), source: error.clone() }
        }
        LoadError::Start { path, .. } => InitError::RuntimeStart { path: path.clone(), source: error.clone() },
        // `LoadError` is `rust-sak`'s and `#[non_exhaustive]`; an unknown one is a runtime that could not be loaded.
        _ => InitError::RuntimeLoad { path: runtime_dir.to_path_buf(), source: error.clone() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn the_process_is_opted_out_of_runtime_telemetry_before_main() {
        // `rust-sak`'s `ai` sets the opt-out from a constructor before `main`; the test binary links it the way every
        // application does, so this is that constructor having run and survived the link.
        assert_eq!(std::env::var("ORT_DISABLE_TELEMETRY").as_deref(), Ok("1"));
    }

    #[test]
    fn each_load_error_is_reported_against_its_path_as_the_kind_it_is() {
        let path = PathBuf::from("/config/io.vinicius.ai/onnx/1.30.0/libonnxruntime.1.30.0.dylib");
        let open = LoadError::Open { path: path.clone(), reason: "not a library".to_string() };
        let start = LoadError::Start { path: path.clone(), reason: "refused".to_string() };
        let missing = LoadError::NotInstalled { path: path.clone() };

        assert!(matches!(load_failure(open, &path), InitError::RuntimeLoad { path: p, .. } if p == path));
        assert!(matches!(load_failure(missing, &path), InitError::RuntimeLoad { path: p, .. } if p == path));
        assert!(matches!(load_failure(start, &path), InitError::RuntimeStart { path: p, .. } if p == path));
    }

    /// The variable the parent sets for the child half of a [`child_test`].
    const CHILD: &str = "OPAI_RUNTIME_TEST_CHILD";

    /// What the child prints once it is running as the child. The parent looks for this rather than reading libtest's
    /// summary line, which is an unspecified format that a harness change could reword at any time.
    const RAN: &str = "OPAI_RUNTIME_TEST_RAN";

    /// Declares a test whose body may only run in a process of its own.
    ///
    /// The filter the parent re-invokes itself with is built from the function's own name, so the two cannot drift.
    /// `--exact` with a name that matches nothing runs no tests and still exits 0, which is what the sentinel is for.
    ///
    /// The module prefix is still a literal, once: `module_path!()` expands to `opai::runtime::tests`, and cargo's
    /// filter wants `runtime::tests::…` with no crate name.
    macro_rules! child_test {
        ($(#[$meta:meta])* fn $name:ident() $body:block) => {
            $(#[$meta])*
            #[test]
            fn $name() {
                if !is_child() {
                    return in_its_own_process(concat!("runtime::tests::", stringify!($name)));
                }
                println!("{RAN}");
                $body
            }
        };
    }

    /// Runs one test of this binary, by name, in a process of its own.
    ///
    /// Everything below needs a process that has not loaded a runtime or latched a failure, and a process has one of
    /// each: [`FAILED_LOAD`] is process-wide by design. Cargo runs a crate's unit tests in one binary on concurrent
    /// threads, so which test went first would otherwise be whichever thread won.
    fn in_its_own_process(test: &str) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(output.status.success(), "the child failed\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}");
        assert!(
            stdout.lines().any(|line| line == RAN),
            "the child ran no test; `{test}` is not the name of one\n--- stdout ---\n{stdout}"
        );
    }

    /// Whether this run is the child half, which is the half that may load a runtime.
    fn is_child() -> bool {
        std::env::var_os(CHILD).is_some()
    }

    /// The runtime library's name on this platform, as `rust-sak` finds it.
    fn library_name() -> &'static str {
        match std::env::consts::OS {
            "macos" => "libonnxruntime.1.30.0.dylib",
            "windows" => "onnxruntime.dll",
            _ => "libonnxruntime.so.1.30.0",
        }
    }

    /// A folder that reads as an installed runtime whose library is `bytes`, which no loader can open.
    fn installed_with(bytes: &[u8]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".installed"), b"{}").unwrap();
        std::fs::write(dir.path().join(library_name()), bytes).unwrap();
        dir
    }

    child_test! {
        fn a_runtime_that_is_not_installed_is_reported_against_the_folder_it_was_looked_for_in() {
            let missing = Path::new("/no/such/directory/onnx/1.30.0");

            match start("opai-test", missing).unwrap_err() {
                InitError::RuntimeLoad { path, .. } => assert_eq!(path, missing),
                other => panic!("expected RuntimeLoad, got {other:?}"),
            }
        }
    }

    child_test! {
        fn a_file_that_is_not_a_shared_library_is_reported_against_its_own_path() {
            // A file that exists and that the loader cannot make anything of, which is the failure a truncated or
            // corrupted install produces.
            let dir = installed_with(b"this is not a shared library");

            match start("opai-test", dir.path()).unwrap_err() {
                InitError::RuntimeLoad { path, .. } => assert_eq!(path, dir.path().join(library_name())),
                other => panic!("expected RuntimeLoad, got {other:?}"),
            }
        }
    }

    child_test! {
        fn every_start_after_a_failed_load_is_refused_naming_the_first_path() {
            let first = installed_with(b"this is not a shared library");
            assert!(matches!(start("opai-test", first.path()).unwrap_err(), InitError::RuntimeLoad { .. }));

            // A different folder, which `rust-sak` alone would try: the latch is what makes this, and every later
            // one, an error naming the *first* attempt.
            let second = installed_with(b"this is not a shared library either");
            for _ in 0..2 {
                match start("opai-test", second.path()).unwrap_err() {
                    InitError::RuntimeUnavailable { path, reason } => {
                        assert_eq!(path, first.path().join(library_name()), "the retry must name the first library");
                        assert!(!reason.is_empty(), "the retry must carry what the first attempt reported");
                    }
                    other => panic!("expected RuntimeUnavailable, got {other:?}"),
                }
            }
        }
    }

    /// The real thing: install the runtime from its Hugging Face folder, start it, and start it again.
    ///
    /// `#[ignore]`d because it downloads the published archive on every run, which no CI runner should be asked to do
    /// on every push. Run it by hand:
    ///
    /// ```text
    /// cargo test -p opai -- --ignored --nocapture runtime::tests::
    /// ```
    #[tokio::test]
    #[ignore = "downloads the published runtime; run by hand with --ignored"]
    async fn a_repeated_initialization_reuses_the_running_environment() {
        use rust_sak::ai::ort::AsPointer as _;

        let (_root, _opai) = crate::live_support::live_application("opai-live-runtime").await;
        let first = ai::environment().expect("an environment is running after the start");

        // A second start in the same process, as a front end's error boundary performs, naming another folder. It
        // succeeds and leaves the running environment in place: a process runs on the first runtime it loads.
        start("opai-live-runtime-again", Path::new("/no/such/folder")).expect("a second start must succeed");
        if cfg!(target_os = "macos") {
            assert!(
                ai::Supported::detect(false, false).webgpu,
                "the runtime's WebGPU plugin offered this Mac's GPU, so the report says WebGPU"
            );
        }
        let second = ai::environment().unwrap();
        assert_eq!(first.ptr(), second.ptr(), "the second start replaced the running environment");
    }
}
