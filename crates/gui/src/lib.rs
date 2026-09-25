//! The Tauri 2 front end for Open Photo AI.
//!
//! Only what concerns the GUI lives here: Tauri commands and events, and window and app state
//! plumbing. Everything both front ends need is in `opai`, and this crate reaches it through the
//! same public API `cli` uses.

// Raised from the default 128: `enhance`'s async chain plus rust-sak's generic `send_retrying` needs more query
// levels than that to type-check. Raising the limit is cheaper than boxing a future just to satisfy a compile-time
// counter.
#![recursion_limit = "256"]

mod analytics;
mod autopilot;
mod catalogue;
mod command;
mod enhance;
mod export;
mod faces;
mod frontend;
mod images;
mod links;
mod logs;
mod reveal;
mod setup;
mod single_instance;
mod stops;
mod sync;
mod task;
#[cfg(test)]
mod test_support;
mod update;
mod window;

// The command's name is written once more, in `frontend/ipc/app.ts`.
/// The application version.
#[tauri::command]
fn version(traceparent: command::Traceparent) -> &'static str {
    command::traced_sync(command::command_span!("version", traceparent), opai::version)
}

/// Build and run the application. Blocks until it exits.
///
/// `prepared` is `opai::prepare_library_path`'s result from `main`, reported here since this is where the log sink
/// gets installed.
pub fn run(prepared: Result<(), opai::InitError>) {
    // How long the session lasted is what its closing record says.
    let began = std::time::Instant::now();

    // First, and ahead of the single-instance plugin: a refused second launch should still be logged (divider,
    // header, one record) before it exits. A failure here is reported to stderr and the app starts anyway - no
    // log is not fatal for a GUI.
    match opai::logging::init(opai::APP_NAME, opai::GUI) {
        Ok(path) => tracing::debug!(log = %path.display(), "logging to file"),
        Err(error) => eprintln!("opai: could not install the log sink; starting without a log: {error}"),
    }

    // Not fatal: only GPU execution providers are affected, and `Opai::initialize` will report the same cause again.
    if let Err(error) = prepared {
        tracing::error!(%error, "could not establish the library search path; GPU providers will be unavailable");
    }

    let mut builder = tauri::Builder::default();

    // Must be registered FIRST: its setup hook detects a second launch and exits the process via
    // `std::process::exit(0)` before any later plugin/state/call could run. The same ordering binds
    // `Opai::initialize`, which must run after `Builder::build()` (e.g. from `setup` or a command), never in `main`
    // before the builder - otherwise a second GUI could take the single-instance claim first and be refused with
    // `InitError::AlreadyRunning`.
    //
    // `_args`/`_cwd` are ignored: a second launch just means "show the window", nothing more.
    if single_instance_is_supported() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tracing::info!("a second launch was refused; raising the window already open");
            single_instance::raise_main_window(app);
        }));
    }

    builder
        // Backs the init script's `platform` value; registers no command of its own, so no permission is granted.
        .plugin(tauri_plugin_os::init())
        // Needed for the file picker (`DialogExt::dialog()` reads state this registers). Its own commands are left
        // ungranted in `capabilities/default.json`.
        .plugin(tauri_plugin_dialog::init())
        // `tauri-plugin-opener` is deliberately NOT registered: `src/reveal.rs` calls its free function directly, and
        // registering the plugin would expose its `reveal_item_in_dir` command, which hangs/aborts off the main
        // runtime worker - and would add commands the frontend must never invoke.
        //
        // `manage` is keyed by type and a no-op on repeat calls, so each of the following is registered exactly once,
        // empty, here rather than filled in lazily by whichever command runs first.
        .manage(setup::Setup::<opai::Opai>::default())
        // The set of files the user has opened; the whole of what the URI scheme below may read.
        .manage(images::Opened::default())
        // Cache of already-served pixels, so re-scrolling a thumbnail isn't a re-decode. (This app's own cache
        // because only WebView2 sits an HTTP cache in front of a custom scheme handler.)
        .manage(images::Renditions::default())
        // The in-flight enhancement run and its result; read by the scheme handler too (see `src/enhance/slot.rs`),
        // hence registered before it.
        .manage(enhance::Runs::default())
        // The Autopilot analyses in flight, by the window's name for each; separate from the slot above so an analysis
        // never supersedes a run or another analysis (see `src/autopilot.rs`).
        .manage(autopilot::Analyses::default())
        // The exports in flight, by the window's name for each; a table of their own so a stop for an export and one for
        // an analysis can never reach each other's work (see `src/stops.rs`).
        .manage(export::Exports::default())
        // The files this session's exports wrote; the whole of what `reveal_export` may show.
        .manage(export::Written::default())
        // Serves opened images' pixels over a custom scheme instead of base64-over-IPC. Async, so large images decode
        // off the window thread. `images::SCHEME` is a constant, not a literal, because `tauri.conf.json`'s `img-src`
        // must list both its platform forms - see the drift guard test below.
        .register_asynchronous_uri_scheme_protocol(images::SCHEME, images::serve)
        // Arms a fallback timer that shows the window if the frontend is late; the window itself ships hidden and is
        // shown by `window::window_ready` once React has something to paint.
        .setup(|app| {
            // Here rather than beside the log sink: this runs after the single-instance plugin's own setup, which exits a
            // refused second launch, so that launch never opens a connection to the collector.
            //
            // Only where the user has not turned Analytics off, which is read from the file the window mirrors its
            // choice into: the window itself has not loaded yet. A launch that begins with it off sends nothing at all.
            analytics::begin(analytics::read(), opai::telemetry::start);

            window::reveal_when_late(app.handle());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            version,
            analytics::set_analytics,
            frontend::log,
            setup::initialize,
            setup::quit,
            logs::reveal_log,
            links::open_link,
            update::is_outdated,
            catalogue::catalogue,
            enhance::enhance,
            enhance::cancel_enhance,
            enhance::release_enhanced,
            enhance::release_all_enhanced,
            faces::detect_faces,
            autopilot::suggest,
            autopilot::cancel_suggest,
            autopilot::suggested_scale,
            export::export,
            export::cancel_export,
            export::export_formats,
            export::directory::pick_directory,
            export::written::reveal_export,
            images::files::open_images,
            images::files::describe_images,
            images::files::reveal_image,
            images::files::input_extensions,
            window::window_ready
        ])
        .build(tauri::generate_context!())
        .expect("error while building the Open Photo AI application")
        .run(move |_, event| {
            if let tauri::RunEvent::Exit = event {
                // Before telemetry stops, so a write that had to be abandoned is still recorded. The handle in the
                // setup state is never dropped on this path, so its own wait on drop does not run.
                if !opai::settle_cache_writes(CACHE_SETTLE) {
                    tracing::warn!(target: "gui", "the last results were still being cached and were abandoned");
                }

                closing(began.elapsed(), opai::telemetry::stop);
            }
        });
}

/// How long closing waits for the run cache to finish writing the last results: a large result's encode and write,
/// on a window that is already gone.
const CACHE_SETTLE: std::time::Duration = std::time::Duration::from_secs(5);

/// Records the session's end, then stops telemetry through `stop`.
///
/// In that order, so the record reaches the collector in the flush the stop makes: a session that ended is then told
/// apart from one that stopped sending because it crashed. `stop` sends what telemetry still holds, waiting at most its
/// export timeout, on a window that is already gone.
fn closing(uptime: std::time::Duration, stop: impl FnOnce()) {
    tracing::info!(target: "gui", ?uptime, "Open Photo AI is closing");

    stop();
}

/// Whether to register the single-instance plugin at all. See [`single_instance::session_bus_is_usable`] for the
/// rule and why it matters on Linux.
#[cfg(target_os = "linux")]
fn single_instance_is_supported() -> bool {
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").ok();
    let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from);

    let supported = single_instance::session_bus_is_usable(address.as_deref(), runtime_dir.as_deref());
    if !supported {
        tracing::warn!(
            "no usable D-Bus session bus; starting without single-instance activation, so a second launch will \
             open its own window"
        );
    }

    supported
}

#[cfg(not(target_os = "linux"))]
fn single_instance_is_supported() -> bool {
    // Other platforms degrade on their own (macOS falls back when the socket can't bind; Windows when the mutex is
    // taken but the holder's window isn't up yet), so there's nothing to gate.
    true
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;
    use std::time::Duration;

    #[test]
    fn the_sessions_end_is_recorded_before_telemetry_stops() {
        let mut seen_at_stop = None;

        let recorder = crate::test_support::Recorder::default();
        {
            use tracing_subscriber::layer::SubscriberExt;

            let subscriber = tracing_subscriber::registry().with(recorder.clone());
            tracing::subscriber::with_default(subscriber, || {
                super::closing(Duration::from_secs(90), || seen_at_stop = Some(recorder.recorded().events));
            });
        }

        let events = seen_at_stop.expect("telemetry was stopped");
        let [record] = events.as_slice() else {
            panic!("expected the closing record before the stop: {events:#?}")
        };
        assert_eq!(record.level, tracing::Level::INFO);
        assert_eq!(record.target, "gui");
        assert_eq!(record.field("uptime"), Some("90s"));

        // Nothing written after the stop could have been sent.
        assert_eq!(recorder.recorded().events.len(), 1);
    }

    /// Every command the window can invoke is wrapped: the names `generate_handler!` registers are exactly the names
    /// given to `command_span!` across the crate, plus two that are never traced. `set_analytics`, because a span would
    /// be one more thing about an opt-out that could leave the process. `log`, because a span per record would root a
    /// trace holding nothing but the record.
    ///
    /// Nothing else stops a new `#[tauri::command]` from skipping the wrapper, so a command whose requests would
    /// root no trace and record none of their failures would compile and ship. Read from the sources, as the
    /// guards below read `tauri.conf.json`.
    #[test]
    fn every_registered_command_opens_its_span() {
        let lib = include_str!("lib.rs");
        let (_, handler) = lib.split_once("generate_handler![").expect("lib.rs registers its commands");
        let (handler, _) = handler.split_once(']').expect("the handler list is closed");
        let registered: BTreeSet<&str> = handler
            .split(',')
            .map(|path| path.trim().rsplit("::").next().expect("a path has a last segment"))
            .filter(|name| !name.is_empty())
            .collect();

        let mut traced = BTreeSet::new();
        spans_in(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut traced);
        traced.extend(["set_analytics", "log"].map(String::from));

        let traced: BTreeSet<&str> = traced.iter().map(String::as_str).collect();
        assert_eq!(registered, traced, "a registered command opens no `command_span!`, or a span names no command");
    }

    /// The names given to `command_span!` in `dir`'s sources, outside tests and comments.
    fn spans_in(dir: &Path, names: &mut BTreeSet<String>) {
        for entry in std::fs::read_dir(dir).expect("the sources are listable") {
            let path = entry.expect("readable").path();

            if path.is_dir() {
                spans_in(&path, names);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") || path.ends_with("test_support.rs") {
                continue;
            }

            let source = std::fs::read_to_string(&path).expect("readable");
            let source = source.split("#[cfg(test)]\nmod tests").next().unwrap_or_default();

            for line in source.lines().filter(|line| !line.trim_start().starts_with("//")) {
                for (at, _) in line.match_indices("command_span!(\"") {
                    let rest = &line[at + "command_span!(\"".len()..];
                    let name = rest.split('"').next().expect("a quoted name");
                    names.insert(name.to_string());
                }
            }
        }
    }

    /// `tauri.conf.json`'s `version`/`identifier` must match the crate: Tauri reads the JSON literal directly rather
    /// than resolving `version.workspace`, so a mismatch ships a bundle whose version disagrees with its binary.
    /// `identifier` also keys every single-instance backend (D-Bus name, Windows mutex/window class, macOS socket
    /// path), so changing it would let old and new versions run side by side against the same config directory.
    #[test]
    fn tauri_conf_json_agrees_with_the_crate() {
        let raw = include_str!("../tauri.conf.json");
        let conf: serde_json::Value = serde_json::from_str(raw).expect("tauri.conf.json is not valid JSON");

        assert_eq!(
            conf["version"],
            env!("CARGO_PKG_VERSION"),
            "tauri.conf.json `version` has drifted from the workspace version in Cargo.toml"
        );
        assert_eq!(
            conf["identifier"], "io.vinicius.opai",
            "tauri.conf.json `identifier` has drifted from the Go app's bundle identity"
        );
    }

    /// `images::SCHEME` must be allowed in both `img-src` policies (`csp` and `devCsp`, which don't merge) and in
    /// both its platform forms (`opai:` on macOS/Linux, `http://opai.localhost` on Windows) - otherwise every image
    /// silently fails to load with nothing but a console message inside the webview.
    #[test]
    fn tauri_conf_json_lets_the_webview_load_the_scheme_rust_registers() {
        let raw = include_str!("../tauri.conf.json");
        let conf: serde_json::Value = serde_json::from_str(raw).expect("tauri.conf.json is not valid JSON");

        let scheme = crate::images::SCHEME;
        let forms = [format!("{scheme}:"), format!("http://{scheme}.localhost")];

        for policy in ["csp", "devCsp"] {
            let sources = conf["app"]["security"][policy]["img-src"]
                .as_str()
                .unwrap_or_else(|| panic!("{policy} has no img-src to serve images through"));

            for form in &forms {
                assert!(
                    sources.split_whitespace().any(|source| source == form),
                    "{policy} img-src does not allow `{form}`, so the webview will refuse every image \
                     the `{scheme}` protocol serves: {sources}"
                );
            }
        }
    }

    /// The window sends to Faro's collector itself, so both policies' `connect-src` must allow Grafana Cloud. The exact
    /// host is a build secret and a policy cannot be built from one, hence the wildcard: it must stay confined to
    /// `*.grafana.net`, and neither policy may widen `connect-src` to any host at all.
    #[test]
    fn tauri_conf_json_lets_the_window_reach_faro_and_nothing_wider() {
        let raw = include_str!("../tauri.conf.json");
        let conf: serde_json::Value = serde_json::from_str(raw).expect("tauri.conf.json is not valid JSON");

        for policy in ["csp", "devCsp"] {
            let sources = conf["app"]["security"][policy]["connect-src"]
                .as_str()
                .unwrap_or_else(|| panic!("{policy} has no connect-src"));
            let sources: Vec<&str> = sources.split_whitespace().collect();

            assert!(
                sources.contains(&"https://*.grafana.net"),
                "{policy} connect-src does not allow Faro's collector, so the window can send nothing: {sources:?}"
            );
            for wide in ["*", "https:", "http:"] {
                assert!(!sources.contains(&wide), "{policy} connect-src allows every host through `{wide}`");
            }
        }
    }

    /// The window ships hidden (avoiding a blank flash while the bundle parses) and its pre-paint background must
    /// match the `--background` token in `frontend/style.css` (avoiding a white flash), so both are asserted against
    /// their sources rather than duplicated here.
    #[test]
    fn the_window_ships_hidden_in_the_colour_the_stylesheet_paints() {
        let raw = include_str!("../tauri.conf.json");
        let conf: serde_json::Value = serde_json::from_str(raw).expect("tauri.conf.json is not valid JSON");
        let window = &conf["app"]["windows"][0];

        assert_eq!(
            window["visible"],
            serde_json::json!(false),
            "the main window no longer ships hidden, so the webview is on screen and empty until React paints"
        );

        let css = include_str!("../frontend/style.css");
        let (_, after) = css.split_once("--background:").expect("style.css declares no `--background` token");
        let background = after.split(';').next().expect("the `--background` declaration is unterminated").trim();

        assert_eq!(
            window["backgroundColor"].as_str(),
            Some(background),
            "the window's pre-paint colour has drifted from `--background` in frontend/style.css"
        );
    }

    /// The grant stays at `core:default` + drag-region only; a command this crate registers itself needs no
    /// permission entry, and this guards against a plugin permission set being added without a deliberate decision.
    #[test]
    fn the_window_is_granted_nothing_beyond_the_two_permissions_it_was_given() {
        let raw = include_str!("../capabilities/default.json");
        let capability: serde_json::Value = serde_json::from_str(raw).expect("default.json is not valid JSON");

        assert_eq!(
            capability["permissions"],
            serde_json::json!(["core:default", "core:window:allow-start-dragging"]),
            "the main window's grant has changed; a command this crate registers itself needs no permission"
        );
    }
}
