//! Turning the installed ONNX Runtime into a loaded one.
//!
//! Slices 1 and 2 put the runtime and the NVIDIA libraries on disk and on the loader's search path; this is where the
//! shared library is actually opened and the environment every model session is later built under is started.
//!
//! The split between the two functions here is what makes any of it testable on a runner with no runtime:
//! [`library_path`] is a pure function of its arguments and is covered for every published platform, while [`start`]
//! cannot be reached without a real ~175 MB library and is covered only where it fails.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use ort::environment::Environment;
use ort::logging::LogLevel;

use crate::error::InitError;
use crate::telemetry::unit::{self, Outcome, Unit, unit_span};

/// The first load failure in this process, if there has been one.
///
/// This exists because `ort`'s own load global does not survive one. It loads through
/// `G_ORT_LIB.get_or_try_init(..)`, whose `OnceLock` is built on [`std::sync::Once::call_once_force`] — and that marks
/// the `Once` **completed** whenever its closure returns without panicking, a returned `Err` included. Nothing is
/// written into the slot, but `get` consults `is_completed()` and hands back a reference to uninitialized memory. So
/// after one failed load, every later `ort::init_from` in the process returns `Ok` having loaded nothing, whatever
/// path it is given; using what it returns is undefined behaviour.
///
/// Recording the first failure here is what makes a retry — which is exactly what the GUI's error boundary performs —
/// report [`InitError::RuntimeUnavailable`] instead of succeeding against a runtime that was never opened.
static FAILED_LOAD: Mutex<Option<Failure>> = Mutex::new(None);

/// The name the WebGPU plugin registers its devices under — what [`ort::device::Device::ep`] reports for them.
pub(crate) const WEBGPU_EP: &str = "WebGpuExecutionProvider";

/// Whether the WebGPU plugin was registered and offered a device, decided once per process.
///
/// Once, because the registration belongs to the process-global environment: a second initialization would be refused
/// by the runtime for registering the same name twice, and has the first one's answer to reuse anyway.
static WEBGPU: OnceLock<bool> = OnceLock::new();

/// The environment the exit hook silences, held weakly so the hook never extends its life — see [`silence_on_exit`].
static EXITING_ENVIRONMENT: OnceLock<Weak<Environment>> = OnceLock::new();

/// What the first failed load in this process was, and what it said.
#[derive(Debug)]
struct Failure {
    /// The library that attempt was made against.
    path: PathBuf,
    /// Its error's message. The error itself cannot be kept: [`ort::LoadDynamicError`] is not `Clone`, and inventing
    /// a variant to rebuild it would report something that did not happen.
    reason: String,
}

/// Where the loader has to be pointed for a library named `lib`, installed into `dir`.
///
/// `lib` comes from the descriptor's own row rather than from a second resolution of the platform here, for the same
/// reason the install directory does: a `match` at this call site could disagree with the lookup the install actually
/// ran from, and then the verified archive would be followed by a path that no file is at.
///
/// It is the name rather than the descriptor because a descriptor need not have one — the GPU libraries are a
/// directory on the search path, not a file the loader is opened against. Go guarded that at this point with
/// `!found || pinned.Lib == ""`, returning an error for a platform that is in fact supported; here a row with no
/// library has nothing to pass, so there is no call to guard against and this cannot fail.
pub(crate) fn library_path(dir: &Path, lib: &str) -> PathBuf {
    dir.join(lib)
}

/// Loads `library` and starts the inference environment every model session is later built under.
///
/// Called from `Opai::initialize` and from nowhere else, which is what makes the guard below the whole application's
/// guard rather than one caller's.
///
/// The environment is created **eagerly**. `commit` on its own is lazy — it only records the configuration, and the
/// `CreateEnv` would then happen inside the first session construction, in the middle of a user's first enhancement.
/// [`Environment::current`] forces it here instead, so a runtime that cannot start is a failed initialization. It is
/// also the only way to reach `set_log_level`, which is a method on the environment rather than on the builder.
///
/// The handle is dropped at the end rather than returned. `ort` keeps a strong reference in a process-global until
/// the process exits, so the drop releases nothing, and `ort::session::Session` holds an `Arc<Environment>` of its
/// own — which is what makes a session outliving its environment unrepresentable rather than merely avoided.
///
/// # Errors
///
/// Returns [`InitError::RuntimeLoad`] if the library will not load, [`InitError::RuntimeStart`] if it loads and the
/// runtime refuses to create an environment, and [`InitError::RuntimeUnavailable`] if a load already failed in this
/// process — see [`FAILED_LOAD`], which is also why the lock is held across the load rather than only around the
/// check: two threads loading at once reach the same defect, since the one that does not run the closure sees a
/// completed `Once` and gets the same uninitialized handle.
///
/// `webgpu` is the WebGPU plugin's file name beside `library`, from the same descriptor row `library` was named by.
/// On success, says whether that plugin execution provider is usable — see [`register_webgpu`].
pub(crate) fn start(name: &str, library: &Path, webgpu: Option<&str>) -> Result<bool, InitError> {
    let span = unit_span!("runtime_load", library = %library.display());

    // Started before the `dlopen`, so the record below says what the load and the environment together cost: the
    // single slowest step of an initialization that has nothing to install.
    let started = std::time::Instant::now();
    let outcome = span.in_scope(|| load(name, library, webgpu, started));
    let duration = started.elapsed();

    // A failure's record is `initialize`'s, which the error is returned to. The span ends here either way.
    match &outcome {
        Ok(_) => unit::ended(Unit::RuntimeLoad, &span, duration, Outcome::Finished),
        Err(error) => unit::ended(Unit::RuntimeLoad, &span, duration, Outcome::Failed { kind: error.kind(), error }),
    }

    outcome
}

/// [`start`]'s body, inside its span. `started` is when the load began, for the record that closes it.
fn load(name: &str, library: &Path, webgpu: Option<&str>, started: std::time::Instant) -> Result<bool, InitError> {
    // A poisoned lock means a previous caller panicked between the check and the record. The state behind it is still
    // readable and is what the next caller has to see, so the guard is taken rather than the panic propagated.
    let mut failed = crate::task::lock(&FAILED_LOAD);

    if let Some(failure) = failed.as_ref() {
        return Err(InitError::RuntimeUnavailable { path: failure.path.clone(), reason: failure.reason.clone() });
    }

    // Safe to call on a process that has already loaded one: `ort`'s global keeps the first library, so a second
    // initialization under a different application name installs into that name's directories and goes on using the
    // runtime this process opened first.
    //
    // The one sanctioned `ort::init_from` in the crate, which `clippy.toml` now enforces rather than leaving to prose:
    // this function owns the `FAILED_LOAD` guard above, and a call from anywhere else would be the one that loads
    // without recording — which is the whole failure the guard exists for.
    #[allow(
        clippy::disallowed_methods,
        reason = "runtime::start is the sole loader; see FAILED_LOAD and design D9"
    )]
    let builder = match ort::init_from(library) {
        Ok(builder) => builder,
        Err(source) => {
            *failed = Some(Failure { path: library.to_path_buf(), reason: source.to_string() });
            return Err(InitError::RuntimeLoad { path: library.to_path_buf(), source });
        }
    };

    // The return value says whether this configuration is the one that took effect, and it is deliberately ignored:
    // `false` means an environment was already configured in this process, which on a second initialization is the
    // expected case rather than a failure. Nothing else can be learned from it either — the configuration that lost
    // is identical to the one that won, because the only inputs are the application name and two constants.
    builder.with_name(name).with_telemetry(false).commit();

    let environment =
        Environment::current().map_err(|source| InitError::RuntimeStart { path: library.to_path_buf(), source })?;

    // Warning is the useful floor, and the same level Go sets: it is where the node-assignment and
    // execution-provider fallback diagnostics that explain a slow or unexpectedly CPU-bound session appear, without
    // the per-node flood that the informational and verbose levels produce.
    //
    // Where those records go is now settled: `ort`'s `tracing` feature is on, so the environment above was created
    // through `CreateEnvWithCustomLogger` and every record the runtime produces arrives as a `tracing` event under
    // the target `ort` — carrying the runtime's severity, its source location and its logger id as span fields. A
    // process that installed `logging::init` writes them into `logs/opai.log`; one that installed no subscriber
    // discards them. Neither writes them to the terminal: the feature replaces ORT's stdio sink rather than tee-ing
    // it.
    //
    // This clamp is the **first of two floors** under that volume, and it is the one that matters most because it is
    // in force whatever the subscriber does: it stops the records being produced at all. The second is the
    // subscriber's own `ort=warn` directive (see `logging::format`), which is what keeps a `RUST_LOG=debug` asking
    // for this project's cache decisions from also asking the runtime for per-node assignment. A flood therefore
    // needs both dials moved.
    //
    // Note the window this leaves: `ort` creates the environment at `ORT_LOGGING_LEVEL_VERBOSE` and this clamps it
    // immediately afterwards. It is inside this one function, and it is covered by the subscriber's floor.
    environment.set_log_level(LogLevel::Warning);
    silence_on_exit(&environment);

    // The library this process is actually running on, by path. It is the single most useful line in a bug report
    // about inference: it says which install was loaded, and — because a process uses the first runtime it loads —
    // whether a later initialization under a different name is running on the library it installed or on this one.
    let duration = started.elapsed();
    tracing::info!(name, library = %library.display(), ?duration, "ONNX Runtime started");

    let webgpu = *WEBGPU.get_or_init(|| register_webgpu(&environment, library, webgpu));

    Ok(webgpu)
}

/// Arranges for the runtime to stop logging just before `ort` releases `environment` at process exit.
///
/// `ort` releases the environment from an exit handler of its own (`release_env_on_exit`), and by then this thread's
/// thread-locals are gone. Any record the runtime produces during that release reaches `ort`'s `tracing` bridge, whose
/// subscriber touches a destroyed thread-local, panics inside an `extern "C"` callback, and aborts the process — after
/// the work is done, but with a SIGABRT a caller reads as a crash. Runtime 1.30.0 made this reachable: its WebGPU
/// plugin's `ReleaseEpFactory` throws during teardown, and the runtime logs that at `ERROR` while unloading it.
///
/// The hook is registered after the environment exists, so it runs *before* `ort`'s: exit handlers run in reverse
/// order of registration, and on Linux and Windows `ort`'s release runs later still, from `.fini_array` and a TLS
/// callback. Raising the floor to `Fatal` stops the records being produced at all — the runtime checks the severity
/// before it builds a message — so there is nothing left for the bridge to forward. What is lost is a report of a
/// factory the process is about to reclaim anyway.
///
/// Registered once per process, like the environment itself.
fn silence_on_exit(environment: &Arc<Environment>) {
    unsafe extern "C" {
        fn atexit(callback: extern "C" fn()) -> std::ffi::c_int;
    }

    extern "C" fn silence() {
        // Weak, so the hook never becomes the last owner: dropping the last `Arc` here would release the environment
        // from this hook, while it is still logging.
        if let Some(environment) = EXITING_ENVIRONMENT.get().and_then(Weak::upgrade) {
            environment.set_log_level(LogLevel::Fatal);
        }
    }

    if EXITING_ENVIRONMENT.set(Arc::downgrade(environment)).is_ok() {
        // SAFETY: `silence` is a plain `extern "C"` function with no arguments, as `atexit` requires, and it touches
        // no thread-local — only a process-global `OnceLock` and the runtime's C API.
        unsafe { atexit(silence) };
    }
}

/// Registers the WebGPU plugin execution provider installed beside `library` as `plugin`, and says whether it offers a
/// device.
///
/// WebGPU ships as a *plugin* rather than being built into the runtime: the library exports `CreateEpFactories`, so it
/// is attached through `RegisterExecutionProviderLibrary` and its devices, never through the named
/// `AppendExecutionProvider("WebGPU")` the `ort::ep::WebGPU` type calls — which this runtime does not know.
///
/// A machine that cannot run WebGPU at all — on Linux, one without a Vulkan driver for a GPU — never has the plugin
/// registered, so its adapter probe never starts.
///
/// Nothing here is an error. An unsupported machine, a missing file, a refused registration or no adapter each leaves
/// WebGPU unsupported and the rest of the runtime untouched, which is the same place a machine without the plugin is
/// in.
fn register_webgpu(environment: &Arc<Environment>, library: &Path, plugin: Option<&str>) -> bool {
    if !rust_sak::sysinfo::is_webgpu_supported() {
        // Only Linux is probed: macOS and Windows always report support, and any other platform has no WebGPU at all.
        let reason = if cfg!(target_os = "linux") {
            "no Vulkan GPU driver"
        } else {
            "not available on this platform"
        };
        tracing::info!("WebGPU not supported on this machine ({reason})");
        return false;
    }

    let Some(plugin) = plugin.map(|plugin| library.with_file_name(plugin)) else {
        tracing::info!("WebGPU plugin not published for this platform");
        return false;
    };
    if !plugin.is_file() {
        tracing::info!(plugin = %plugin.display(), "WebGPU plugin not installed");
        return false;
    }

    // The handle only unregisters on request, so dropping it keeps the library registered for the process.
    if let Err(error) = environment.register_ep_library("WebGPU", &plugin) {
        tracing::warn!(plugin = %plugin.display(), %error, "WebGPU plugin could not be registered");
        return false;
    }

    let devices = webgpu_devices(environment).count();
    tracing::info!(plugin = %plugin.display(), devices, "WebGPU plugin registered");

    devices > 0
}

/// The devices the WebGPU plugin registered with `environment`, in the order the runtime reports them.
///
/// One filter for the two questions asked of them — whether the plugin offers any at initialization, and which one a
/// session attaches — so the two cannot disagree about what counts as a WebGPU device.
pub(crate) fn webgpu_devices(environment: &Environment) -> impl Iterator<Item = ort::device::Device<'_>> {
    environment.devices().filter(|device| device.ep().is_ok_and(|ep| ep == WEBGPU_EP))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deps::artifact::{CUDA, CUDNN, ONNX_RUNTIME, TENSORRT};
    use crate::deps::release::{Dependency, RELEASE_BASE_URL};
    use std::process::Command;

    /// The descriptor `release` yields for one platform, as the install would have built it.
    fn descriptor(release: &crate::deps::artifact::Release, os: &'static str, arch: &'static str) -> Dependency {
        Dependency::from_release_at(RELEASE_BASE_URL, release, os, arch).unwrap()
    }

    /// The library name `release` publishes for one platform, as `Opai::initialize` takes it off the runtime row.
    fn lib_of(release: &crate::deps::artifact::Release, os: &'static str, arch: &'static str) -> &'static str {
        descriptor(release, os, arch)
            .lib
            .expect("the runtime names a library on every platform it is published for")
    }

    #[test]
    fn every_published_runtime_platform_resolves_to_its_own_library() {
        let cases = [
            ("macos", "aarch64", "libonnxruntime.1.30.0.dylib"),
            ("linux", "x86_64", "libonnxruntime.so.1.30.0"),
            ("linux", "aarch64", "libonnxruntime.so.1.30.0"),
            ("windows", "x86_64", "onnxruntime.dll"),
            ("windows", "aarch64", "onnxruntime.dll"),
        ];

        let dir = Path::new("/somewhere/opai/runtime");

        for (os, arch, lib) in cases {
            let path = library_path(dir, lib_of(&ONNX_RUNTIME, os, arch));
            assert_eq!(path, dir.join(lib), "{os}/{arch}");
        }

        assert_eq!(cases.len(), ONNX_RUNTIME.archives.len(), "a pinned platform is untested");
    }

    #[test]
    fn a_dependency_that_names_no_library_has_nothing_to_pass_here() {
        // The three GPU libraries are a directory on the search path rather than a file the loader is opened against,
        // and their rows say so: there is no name to hand `library_path`, so the call that would have to be rejected
        // cannot be written. The absence of an argument stands in for a runtime guard — and for a
        // `UnsupportedPlatform` reported for a platform that is in fact supported.
        for release in [&CUDA, &CUDNN, &TENSORRT] {
            assert_eq!(descriptor(release, "linux", "x86_64").lib, None, "{}", release.name);
        }
    }

    /// Set by the parent in the child's environment; its presence is what turns a run of a test into the half that
    /// actually touches `ort`.
    const CHILD: &str = "OPAI_RUNTIME_TEST_CHILD";

    /// What the child prints once it is running as the child. The parent looks for this rather than reading libtest's
    /// summary line, which is an unspecified format that a harness change could reword at any time.
    const RAN: &str = "OPAI_RUNTIME_TEST_RAN";

    /// Declares a test whose body may only run in a process of its own.
    ///
    /// The filter the parent re-invokes itself with is built from the function's own name, so the two cannot drift.
    /// Writing it out by hand is what made the parent's "did a test actually run?" check necessary in the first place:
    /// `--exact` with a name that matches nothing runs no tests and still exits 0.
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
    /// Everything below needs a pristine `ort` load global, and a process has exactly one: the first `init_from` is
    /// the only one whose outcome is its own, and after a failed one every later call returns `Ok` having loaded
    /// nothing (see [`FAILED_LOAD`]). Cargo runs a crate's unit tests in one binary on concurrent threads, so which
    /// test went first would otherwise be whichever thread won. `tests/reexec_linux.rs` isolates its `execvp` the
    /// same way; the difference here is that these tests need crate-internal access, which an integration test has
    /// none of.
    fn in_its_own_process(test: &str) {
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(output.status.success(), "the child failed\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}");

        // A filter that matches nothing runs no tests and still exits 0, so without this a renamed test would pass
        // here having asserted nothing at all — which is exactly what a self-invoking test gets wrong silently.
        // The child's own sentinel rather than libtest's summary line: the latter is an unspecified format, and
        // parsing it would turn a harness reword into three failing tests for no reason.
        assert!(
            stdout.lines().any(|line| line == RAN),
            "the child ran no test; `{test}` is not the name of one\n--- stdout ---\n{stdout}"
        );
    }

    /// Whether this run is the child half, which is the half that may touch `ort`.
    fn is_child() -> bool {
        std::env::var_os(CHILD).is_some()
    }

    child_test! {
        fn a_library_that_is_not_there_is_reported_against_the_path_it_was_attempted_against() {
            let missing = Path::new("/no/such/directory/libonnxruntime.1.30.0.dylib");
            let error = start("opai-test", missing, None).unwrap_err();

            match error {
                InitError::RuntimeLoad { path, .. } => assert_eq!(path, missing),
                other => panic!("expected RuntimeLoad, got {other:?}"),
            }
        }
    }

    child_test! {
        fn a_file_that_is_not_a_shared_library_is_reported_against_its_own_path() {
            // A file that exists and that `dlopen` cannot make anything of, which is the failure a truncated
            // or corrupted install produces — distinct from the missing-file case only in what the loader says
            // about it.
            let dir = tempfile::tempdir().unwrap();
            let not_a_library = dir.path().join("libonnxruntime.1.30.0.dylib");
            std::fs::write(&not_a_library, b"this is not a shared library").unwrap();

            let error = start("opai-test", &not_a_library, None).unwrap_err();

            match error {
                InitError::RuntimeLoad { path, .. } => assert_eq!(path, not_a_library),
                other => panic!("expected RuntimeLoad, got {other:?}"),
            }
        }
    }

    child_test! {
        fn a_second_start_after_a_failed_load_is_refused_rather_than_wrongly_succeeding() {
            let first = Path::new("/no/such/directory/first-onnxruntime.dylib");
            assert!(matches!(start("opai-test", first, None).unwrap_err(), InitError::RuntimeLoad { .. }));

            // A different path, and one that is not a library either. `ort` alone answers this with `Ok` — its
            // `OnceLock` counts the failed attempt as completed and hands back a reference to uninitialized
            // memory — so the whole point of the guard is that this is an error, and that it names the *first*
            // attempt rather than this one.
            let dir = tempfile::tempdir().unwrap();
            let second = dir.path().join("second-onnxruntime.dylib");
            std::fs::write(&second, b"this is not a shared library either").unwrap();

            match start("opai-test", &second, None).unwrap_err() {
                InitError::RuntimeUnavailable { path, reason } => {
                    assert_eq!(path, first, "the retry must name the library the first attempt used");
                    assert!(!reason.is_empty(), "the retry must carry what the first attempt reported");
                }
                other => panic!("expected RuntimeUnavailable, got {other:?}"),
            }
        }
    }

    /// The real thing: install the pinned runtime and start an environment against it.
    ///
    /// `#[ignore]`d because it downloads the published archive on every run — ~175 MB on the platforms where the
    /// runtime ships with its execution providers — which no CI runner should be asked to do on every push. It is the
    /// same arrangement, for the same reason, as Go's `internal/deps/live_manual_test.go`. Run it by hand:
    ///
    /// ```text
    /// cargo test -p opai -- --ignored --nocapture
    /// ```
    ///
    /// It needs a process nothing else has loaded a runtime in, which `--ignored` very nearly gives it for free:
    /// every other test that loads one is in `sessions::live`, and each of those starts the runtime through the same
    /// guarded path — a second start in one process is supported and leaves the running environment in place, which
    /// is what this test itself asserts below.
    #[tokio::test]
    #[ignore = "downloads the pinned ~175 MB runtime; run by hand with --ignored"]
    async fn the_pinned_runtime_loads_and_its_environment_starts() {
        let root = tempfile::tempdir().unwrap();
        let app_dir = root.path().join("opai-live-test");

        let descriptor =
            Dependency::from_release_at(RELEASE_BASE_URL, &ONNX_RUNTIME, std::env::consts::OS, std::env::consts::ARCH)
                .unwrap();

        let (_opai, installed) = crate::Opai::install(
            "opai-live-test",
            app_dir.clone(),
            crate::instance::tests::claimed(&app_dir),
            crate::setup::Plan::runtime_only(descriptor.clone()),
            crate::ModelTrust::Published,
            None,
        )
        .await
        .unwrap();

        // From the directory the install reported, exactly as `Opai::initialize` does it — so this drives the real
        // path rather than a hand-built one that could resolve somewhere the install never wrote.
        let library = library_path(&installed.runtime, descriptor.lib.expect("the runtime names a library"));
        assert!(library.is_file(), "the install left no library at {}", library.display());

        // The first start is the one that loads the library, so it is the one whose record says what the load cost.
        // Asserted only here: the record is written after a successful load, which nothing hermetic can produce.
        let (log, started) =
            crate::logging::records_of_blocking("info", || start("opai-live-test", &library, descriptor.webgpu));
        started.expect("the pinned runtime must load and start");

        let [loaded] = crate::logging::records(&log, "ONNX Runtime started")[..] else {
            panic!("the load was not recorded exactly once:\n{log}");
        };
        assert!(loaded.contains("level=INFO"), "{loaded}");
        assert_eq!(
            crate::logging::field(loaded, "library"),
            Some(crate::logging::format::as_field_value(&library.display().to_string()).as_str()),
            "the record does not name the library: {loaded}"
        );
        assert!(
            crate::logging::field(loaded, "duration").is_some(),
            "the record does not say how long the load took: {loaded}"
        );

        let first = Environment::current().expect("an environment must be running after start");

        // A second start in the same process, as a front end's error boundary performs. It must succeed, and it must
        // leave the environment that is already running in place rather than build a second one — which the ONNX
        // Runtime C API could not do anyway, since it supports one `CreateEnv` per process even after a `ReleaseEnv`.
        start("opai-live-test-again", &library, descriptor.webgpu)
            .expect("a second start in the same process must succeed");

        let second = Environment::current().unwrap();
        assert!(std::sync::Arc::ptr_eq(&first, &second), "the second start replaced the running environment");

        // The one place an `ort::Error` can be built at all: every constructor goes through `CreateStatus`, which
        // needs the API of a loaded runtime. So `RuntimeStart`'s own `Display` is asserted here rather than in the
        // unit suite, which has no runtime to construct one against.
        let error = InitError::RuntimeStart {
            path: library.clone(),
            source: ort::Error::new("the runtime refused to create an environment"),
        };
        let message = error.to_string();
        assert!(
            message.contains(&library.display().to_string()),
            "the message did not name the library: {message}"
        );
        assert!(
            message.contains("refused to create an environment"),
            "the message lost ort's own text: {message}"
        );

        // What the bridge is for, against a real runtime: the C++ diagnostics reach a subscriber rather than the
        // terminal. A hermetic test can emit an `ort`-target event and check the line it produces — `logging::format`
        // does — but only a loaded runtime can show that `CreateEnvWithCustomLogger` actually took, so this reports
        // what it captured and leaves the reading to whoever ran it by hand.
        //
        // Reported rather than asserted, deliberately. Whether a given machine's runtime emits anything at warning
        // during a bare environment start is the runtime's business and varies by platform and provider set; failing
        // the live check because a healthy machine had nothing to complain about would make it a worse check.
        let captured = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = std::sync::Arc::clone(&captured);

        tracing::subscriber::with_default(collecting_subscriber(sink), || {
            // A second start under the running environment, which is the call that re-reads the configuration and is
            // the cheapest thing here that asks the runtime to say something.
            start("opai-live-log-test", &library, descriptor.webgpu).expect("a start under a subscriber must succeed");
            let _ = Environment::current();
        });

        let records = captured.lock().unwrap();
        println!("captured {} record(s) from the runtime through `ort`'s tracing bridge", records.len());
        for record in records.iter() {
            println!("  ort: {record}");
        }
        println!(
            "if the runtime printed any of the above to this terminal as well, the bridge is tee-ing rather than \
             replacing its sink and the capture is not complete"
        );

        println!("live runtime loaded from {}", library.display());
        println!("ort build info: {}", ort::info());
    }

    /// A subscriber that keeps every `ort` record's message in `into`, for the live check above.
    ///
    /// Only the message, and only the `ort` target: this exists to show that the records arrive, and the line they
    /// would be rendered as is `logging::format`'s to decide and its tests' to check.
    #[cfg(test)]
    fn collecting_subscriber(into: std::sync::Arc<Mutex<Vec<String>>>) -> impl tracing::Subscriber {
        use tracing::field::{Field, Visit};

        struct Collect(std::sync::Arc<Mutex<Vec<String>>>);

        impl<S> tracing_subscriber::Layer<S> for Collect
        where
            S: tracing::Subscriber,
        {
            fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
                if !event.metadata().target().starts_with("ort") {
                    return;
                }

                struct Message(String);
                impl Visit for Message {
                    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                        if field.name() == "message" {
                            self.0 = format!("{value:?}");
                        }
                    }
                }

                let mut message = Message(String::new());
                event.record(&mut message);
                self.0.lock().unwrap().push(format!("[{}] {}", event.metadata().level(), message.0));
            }
        }

        use tracing_subscriber::layer::SubscriberExt as _;
        tracing_subscriber::registry().with(Collect(into))
    }

    #[test]
    fn the_path_is_under_the_directory_it_was_installed_into() {
        // What makes the loaded runtime provably the one this application installed: the path is the join of the
        // directory the install wrote to and the name the verified archive pinned, with nothing else consulted.
        let dir = Path::new("/somewhere/opai/runtime");
        let path = library_path(dir, lib_of(&ONNX_RUNTIME, "macos", "aarch64"));

        assert_eq!(path.parent(), Some(dir));
    }
}
