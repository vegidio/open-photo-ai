use std::sync::{Arc, Mutex};

use rust_sak::o11y::Signal;
use serde_json::Value as Json;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tracing::Level as TracingLevel;

use super::*;
use crate::error::SessionError;
use crate::image::Picture;
use crate::inference::process::process;
use crate::logging::{field, records, records_of_blocking};
use crate::models::{ArtifactId, Denoise, FloatPrecision, Strength};
use crate::pipeline::Backend;
use crate::pipeline::test_support::{self, FakeSession, stub_backend_runs};
use crate::providers::ExecutionProvider;
use crate::providers::profile::EpProfile;
use crate::sessions::{Interest, SessionHandle};
use imaging::test_support::gradient;

/// An export failure, as `o11y`'s worker would hand one to the callback.
fn dropped() -> O11yError {
    O11yError::Dropped { signal: Signal::Logs, count: 3 }
}

#[test]
fn a_build_without_a_collector_sends_nothing_and_opens_no_connection() {
    // A listener standing where a collector would be, so "no connection" is observed rather than assumed.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());

    // Either half missing or empty is no collector: this project's collector always authenticates.
    for (endpoint, auth) in
        [(None, None), (Some(url.as_str()), None), (Some(url.as_str()), Some("")), (Some(""), Some("x"))]
    {
        assert_eq!(begin(endpoint, auth).unwrap(), Sending::NoCollector, "{endpoint:?} {auth:?}");
    }

    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
        "a build without a collector connected to one"
    );
}

#[test]
fn a_build_without_a_collector_says_so_in_the_file_at_debug() {
    let (log, _) = records_of_blocking("debug", || begin(None, None));

    let lines = records(&log, "telemetry started");
    assert_eq!(lines.len(), 1, "{log}");
    assert!(lines[0].contains("level=DEBUG "), "{log}");
    assert_eq!(field(lines[0], "sending"), Some("false"), "{log}");

    // And nothing at the default level: it is the ordinary state of a development build.
    let (log, _) = records_of_blocking("info", || begin(None, None));
    assert!(records(&log, "telemetry started").is_empty(), "{log}");
}

#[test]
fn a_second_start_in_one_process_is_refused() {
    // Driven through the latch directly, as `logging`'s second-install test is: the latch is process-wide, and a real
    // first `start` would read whatever collector this test binary happened to be compiled with.
    assert!(STARTED.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok());

    let refused = start();
    assert!(matches!(refused, Err(TelemetryError::AlreadyStarted)), "got {refused:?}");

    STARTED.store(false, Ordering::SeqCst);
}

#[test]
fn only_the_first_export_failure_is_a_warning() {
    // A flag of its own rather than the process's, which the export test below may already have set.
    let reported = AtomicBool::new(false);

    let (log, _) = records_of_blocking("info", || {
        record_export_failure(&reported, &dropped());
        record_export_failure(&reported, &dropped());
        record_export_failure(&reported, &dropped());
    });

    let lines = records(&log, "sending telemetry failed");
    assert_eq!(lines.len(), 1, "{log}");
    assert!(lines[0].contains("level=WARN "), "{log}");
    assert!(lines[0].contains("target=opai::telemetry::export "), "{log}");
    assert!(field(lines[0], "error").is_some_and(|error| error.contains("dropped 3")), "{log}");

    // The rest are still there for a user who asks for them.
    let reported = AtomicBool::new(false);
    let (log, _) = records_of_blocking("debug", || {
        record_export_failure(&reported, &dropped());
        record_export_failure(&reported, &dropped());
    });

    let lines = records(&log, "sending telemetry failed");
    assert_eq!(lines.len(), 2, "{log}");
    assert!(lines[1].contains("level=DEBUG "), "{log}");
}

#[test]
fn a_refused_payload_is_recorded_with_the_collectors_own_answer() {
    let refused = O11yError::ExportRejected {
        signal: Signal::Metrics,
        status: 429,
        body: "the ingestion rate limit (1500) was exceeded\n".to_string(),
    };

    let (log, _) = records_of_blocking("info", || record_export_failure(&AtomicBool::new(false), &refused));

    let lines = records(&log, "sending telemetry failed");
    assert_eq!(lines.len(), 1, "{log}");
    assert!(field(lines[0], "error").is_some_and(|error| error.contains("status 429")), "{log}");
    assert!(
        field(lines[0], "response").is_some_and(|response| response.contains("ingestion rate limit (1500)")),
        "the collector's reason was not recorded: {log}"
    );

    // A refusal with no body, and a failure that is not a refusal, have no answer to record.
    let silent = O11yError::ExportRejected { signal: Signal::Metrics, status: 429, body: " ".to_string() };
    for error in [silent, dropped()] {
        let (log, _) = records_of_blocking("info", || record_export_failure(&AtomicBool::new(false), &error));

        let lines = records(&log, "sending telemetry failed");
        assert_eq!(lines.len(), 1, "{log}");
        assert_eq!(field(lines[0], "response"), None, "{log}");
    }
}

/// A backend that opens a fake filter session for whatever it is asked for, so an enhancement runs with no runtime.
struct Filtering;

impl Backend for Filtering {
    type Session = FakeSession;
    type Error = std::io::Error;

    async fn acquire(
        &self,
        _artifact: &ArtifactId,
        _profile: &EpProfile,
        _requested: ExecutionProvider,
        _interest: &Interest,
    ) -> Result<SessionHandle<FakeSession>, SessionError> {
        Ok(test_support::session(None))
    }

    fn run_tile(handle: &SessionHandle<FakeSession>, input: &[f32], output: &mut [f32]) -> Result<(), Self::Error> {
        <test_support::Fake as Backend>::run_tile(handle, input, output)
    }

    stub_backend_runs!("a denoise takes the tile seam"; run_graph, run_named_outputs, run_weighted);
}

/// One finished enhancement over a small picture, through [`Filtering`].
fn enhance(runtime: &tokio::runtime::Runtime) {
    let strength = Strength::new(0.5).expect("a strength in range");
    let picture = Picture::new("/pictures/export.jpg", Arc::new(gradient(64, 48)), "e0e0e0e0e0e0e0e0");
    let chain = [Denoise::stockholm(FloatPrecision::Fp32, strength)];

    runtime
        .block_on(process(&Filtering, None, &picture, &chain, None))
        .expect("the enhancement ran");
}

/// The trace and span a window names when it asks for a request, as `traceparent` carries them.
const WINDOW_TRACE: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
const WINDOW_SPAN: &str = "00f067aa0ba902b7";

/// What the stand-in collector received: one entry per request.
#[derive(Debug)]
struct Post {
    path: String,
    authorization: Option<String>,
    body: Json,
}

type Posts = Arc<Mutex<Vec<Post>>>;

/// Accepts connections until the runtime is dropped, and answers every request `200` while `reachable` is set.
async fn collector(listener: TcpListener, posts: Posts, reachable: Arc<AtomicBool>) {
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(serve(stream, Arc::clone(&posts), Arc::clone(&reachable)));
    }
}

/// One keep-alive connection: HTTP/1.1 requests with a `content-length` body, which is all `reqwest` sends for JSON.
///
/// While `reachable` is clear, a request is read and the connection dropped with no answer, which is what the exporter
/// sees of a collector that went away mid-session. Nothing is recorded for it.
async fn serve(stream: TcpStream, posts: Posts, reachable: Arc<AtomicBool>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream);

    loop {
        let mut request_line = String::new();
        if reader.read_line(&mut request_line).await? == 0 {
            return Ok(());
        }
        let path = request_line.split_whitespace().nth(1).unwrap_or_default().to_string();

        let mut length = 0;
        let mut authorization = None;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header).await?;
            let header = header.trim_end();
            if header.is_empty() {
                break;
            }
            if let Some((name, value)) = header.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    length = value.trim().parse().unwrap_or(0);
                } else if name.eq_ignore_ascii_case("authorization") {
                    authorization = Some(value.trim().to_string());
                }
            }
        }

        let mut body = vec![0; length];
        reader.read_exact(&mut body).await?;
        if !reachable.load(Ordering::SeqCst) {
            return Ok(());
        }
        let body = serde_json::from_slice(&body).unwrap_or(Json::Null);
        posts.lock().unwrap().push(Post { path, authorization, body });

        reader.get_mut().write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n").await?;
    }
}

/// Every element of the `outer[].inner[].leaf[]` arrays of every post to `path`.
fn items<'a>(posts: &'a [Post], path: &str, outer: &str, inner: &str, leaf: &str) -> Vec<&'a Json> {
    posts
        .iter()
        .filter(|post| post.path == path)
        .flat_map(|post| post.body[outer].as_array().into_iter().flatten())
        .flat_map(|resource| resource[inner].as_array().into_iter().flatten())
        .flat_map(|scope| scope[leaf].as_array().into_iter().flatten())
        .collect()
}

/// The string value of attribute `key` on an OTLP log record or span.
fn attribute<'a>(item: &'a Json, key: &str) -> Option<&'a str> {
    item["attributes"].as_array()?.iter().find(|attribute| attribute["key"] == key)?["value"]["stringValue"].as_str()
}

// ⚠️ The only test in this suite that initialises `o11y`, which can happen once per process: no other test may call
// `begin` with a collector, or `start`. While it runs, `records_of_blocking` holds the suite's recording lock, so no
// other test's records reach the collector either.
#[test]
fn records_and_spans_reach_the_collector_as_the_file_has_them() {
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_io().build().unwrap();
    let listener = runtime.block_on(TcpListener::bind("127.0.0.1:0")).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let posts = Posts::default();
    let reachable = Arc::new(AtomicBool::new(true));
    runtime.spawn(collector(listener, Arc::clone(&posts), Arc::clone(&reachable)));

    let (log, (started, failed_while_dark)) = records_of_blocking("info", || {
        let started = begin(Some(&url), Some("Bearer export-test"));

        {
            let span = tracing::info_span!("export_test_unit", unit = "export");
            let _entered = span.enter();
            tracing::info!("export test info record");

            // Work handed to a blocking thread, bound there to this recording's subscriber as every thread is to the
            // global one `logging::init` installs.
            let recording = tracing::dispatcher::get_default(tracing::Dispatch::clone);
            let handed_off: Result<(), crate::InitError> = runtime.block_on(crate::task::spawn_blocking(move || {
                tracing::dispatcher::with_default(&recording, || tracing::info!("export test handed-off record"));
            }));
            handed_off.unwrap();
        }

        // A unit of this crate's own: its span, a step beneath it, its records and its metrics.
        enhance(&runtime);

        // A unit that fails, whose failure record belongs to its span's trace.
        crate::image::load_blocking("/export-test/never-written.png").expect_err("a file that was never written");

        // A request whose window named its trace, with a unit of this crate's own beneath it, and requests whose
        // window named none, or named one that cannot be read.
        let traceparent = format!("00-{WINDOW_TRACE}-{WINDOW_SPAN}-01");
        with_parent(Some(&traceparent), || tracing::info_span!("export_test_request", run = "window")).in_scope(|| {
            tracing::info!("export test record under a window's trace");
            crate::image::load_blocking("/export-test/under-a-window.png").expect_err("a file that was never written");
        });
        for (name, header) in [
            ("export_test_request_malformed", Some("00-not-a-traceparent-01")),
            ("export_test_request_zeroes", Some("00-00000000000000000000000000000000-0000000000000000-01")),
            ("export_test_request_unnamed", None),
        ] {
            with_parent(header, || tracing::info_span!("export_test_request_unparented", case = name)).in_scope(|| {});
        }

        tracing::debug!("export test debug record");

        // `ort`'s own shape: see `logging::format::is_ort_record_span`.
        let ort = tracing::span!(
            target: "ort::logging",
            TracingLevel::TRACE,
            "ort",
            id = "export-test",
            location = "export_test.cc:1"
        );
        tracing::event!(target: "ort::logging", parent: &ort, TracingLevel::WARN, "export test runtime warning");
        drop(ort);

        // The collector goes away for one export. Everything so far is sent first, so only what follows is lost.
        o11y::flush();
        assert!(!EXPORT_FAILED.load(Ordering::SeqCst), "an export failed while the collector was answering");
        reachable.store(false, Ordering::SeqCst);
        tracing::info!("export test record while unreachable");
        o11y::flush();
        // The real failure's record is emitted on `o11y`'s export thread, which this recording's subscriber is not
        // bound to, so the flag is what observes it here. What the callback writes, and at which level, is
        // `only_the_first_export_failure_is_a_warning`'s.
        let failed_while_dark = EXPORT_FAILED.load(Ordering::SeqCst);
        reachable.store(true, Ordering::SeqCst);

        // And on this thread, so that the record goes through the collector's filter rather than around it.
        record_export_failure(&EXPORT_FAILED, &dropped());

        stop();
        tracing::info!("export test record after stop");

        (started, failed_while_dark)
    });

    assert_eq!(started.unwrap(), Sending::Yes);
    assert!(failed_while_dark, "an export to an unreachable collector was not reported");

    let posts = posts.lock().unwrap();
    assert!(!posts.is_empty(), "nothing reached the collector");
    assert!(
        posts.iter().all(|post| post.authorization.as_deref() == Some("Bearer export-test")),
        "a request went without the build's authorization: {posts:?}"
    );

    let logs = items(&posts, "/v1/logs", "resourceLogs", "scopeLogs", "logRecords");
    let spans = items(&posts, "/v1/traces", "resourceSpans", "scopeSpans", "spans");
    let log_record = |body: &str| logs.iter().copied().find(|record| record["body"]["stringValue"] == body);

    // The session record, under the service name the Go application's dashboards query.
    assert!(log_record("telemetry started").is_some(), "{logs:?}");
    let resource = &posts.iter().find(|post| post.path == "/v1/logs").unwrap().body["resourceLogs"][0]["resource"];
    assert_eq!(attribute(resource, "service.name"), Some(SERVICE_NAME), "{resource}");
    // Against the profile rather than a literal: `development` is also `o11y`'s default, so a debug-only assertion
    // would pass with the setting never made, and `cargo test --release` is what catches that.
    let environment = if cfg!(debug_assertions) { "development" } else { "production" };
    assert_eq!(attribute(resource, "deployment.environment.name"), Some(environment), "{resource}");
    // The machine's hardware, on the resource rather than on each record, under the names the dashboards query.
    let hardware = crate::hardware::snapshot();
    assert_eq!(attribute(resource, "cpu.model"), Some(hardware.cpu_model.as_str()), "{resource}");
    let has_int = |key: &str| {
        resource["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["key"] == key && entry["value"]["intValue"].is_string())
    };
    assert!(has_int("cpu.cores") && has_int("memory"), "{resource}");

    // The record inside a span arrives in that span's trace, and the span arrives as a trace.
    let info = log_record("export test info record").expect("the info record did not arrive");
    let unit = spans.iter().find(|span| span["name"] == "export_test_unit").expect("the span did not arrive");
    assert_eq!(info["traceId"], unit["traceId"], "the record is not in its span's trace");
    assert_eq!(info["spanId"], unit["spanId"]);

    // As does a record from work handed to a blocking thread inside that span.
    let handed_off = log_record("export test handed-off record").expect("the handed-off record did not arrive");
    assert_eq!(handed_off["traceId"], unit["traceId"], "the handed-off record left its span's trace");
    assert_eq!(handed_off["spanId"], unit["spanId"]);

    // The runtime's diagnostic carries its span's fields, and its span is not a trace of its own.
    let warning = log_record("export test runtime warning").expect("the runtime's warning did not arrive");
    assert_eq!(attribute(warning, "location"), Some("export_test.cc:1"), "{warning}");
    assert_eq!(attribute(warning, "id"), Some("export-test"), "{warning}");
    assert!(!spans.iter().any(|span| span["name"] == "ort"), "the runtime's span was exported: {spans:?}");

    // An enhancement is one trace: the run, the step beneath it, and the records it wrote.
    let enhancement = spans.iter().find(|span| span["name"] == "enhancement").expect("the enhancement was not sent");
    let step = spans.iter().find(|span| span["name"] == "step").expect("the step was not sent");
    assert_eq!(step["traceId"], enhancement["traceId"], "the step left the enhancement's trace");
    assert_eq!(step["parentSpanId"], enhancement["spanId"], "the step is not the enhancement's child");
    assert_eq!(attribute(enhancement, "outcome"), Some("finished"), "{enhancement}");
    let finished = log_record("enhancement finished").expect("the closing record did not arrive");
    assert_eq!(
        finished["traceId"], enhancement["traceId"],
        "the closing record is not in the enhancement's trace"
    );

    // A failed unit's record is in the failed span's trace, and the span says it failed.
    let load = spans
        .iter()
        .find(|span| span["name"] == "image_load" && span["traceId"] != WINDOW_TRACE)
        .expect("the failed load was not sent");
    assert_eq!(attribute(load, "outcome"), Some("failed"), "{load}");
    let unreadable = log_record("an image could not be read").expect("the failure record did not arrive");
    assert_eq!(unreadable["traceId"], load["traceId"], "the failure record is not in its unit's trace");
    assert_eq!(unreadable["spanId"], load["spanId"]);

    // A request whose window named its trace continues it, under the window's span, and the work beneath it follows.
    let request = spans
        .iter()
        .find(|span| span["name"] == "export_test_request")
        .expect("the request was not sent");
    assert_eq!(request["traceId"], WINDOW_TRACE, "the request left the window's trace: {request}");
    assert_eq!(request["parentSpanId"], WINDOW_SPAN, "the request is not the window's span's child: {request}");
    let beneath = spans
        .iter()
        .find(|span| span["name"] == "image_load" && span["traceId"] == WINDOW_TRACE)
        .expect("the unit beneath the request left its trace");
    assert_eq!(beneath["parentSpanId"], request["spanId"], "the unit is not the request's child: {beneath}");
    let under_window =
        log_record("export test record under a window's trace").expect("the request's record did not arrive");
    assert_eq!(under_window["traceId"], WINDOW_TRACE);
    assert_eq!(under_window["spanId"], request["spanId"]);

    // One that named none, or one that cannot be read, roots a trace of its own.
    let unparented: Vec<_> = spans.iter().filter(|span| span["name"] == "export_test_request_unparented").collect();
    assert_eq!(unparented.len(), 3, "{spans:?}");
    for span in unparented {
        assert_ne!(span["traceId"], WINDOW_TRACE, "{span}");
        assert!(span["parentSpanId"].as_str().unwrap_or_default().is_empty(), "not a root: {span}");
    }

    // And it was measured, by unit and outcome.
    let metrics = items(&posts, "/v1/metrics", "resourceMetrics", "scopeMetrics", "metrics");
    let durations = metrics
        .iter()
        .find(|metric| metric["name"] == "opai_unit_duration_seconds")
        .expect("the unit durations were not sent");
    let points = durations["histogram"]["dataPoints"].as_array().expect("a histogram's points");
    assert!(
        points
            .iter()
            .any(|point| attribute(point, "unit") == Some("enhancement")
                && attribute(point, "outcome") == Some("finished")),
        "no finished enhancement was measured: {durations}"
    );

    // Below the floor, and telemetry's own failure, are never sent.
    assert!(log_record("export test debug record").is_none(), "{logs:?}");
    assert!(log_record("sending telemetry failed").is_none(), "{logs:?}");

    // What the collector could not take was discarded rather than held for later, and nothing followed the stop.
    assert!(log_record("export test record while unreachable").is_none(), "{logs:?}");
    assert!(log_record("export test record after stop").is_none(), "{logs:?}");

    // And the file has what it always had, the session record included.
    assert_eq!(records(&log, "telemetry started").len(), 1, "{log}");
    assert_eq!(records(&log, "export test info record").len(), 1, "{log}");
    assert_eq!(records(&log, "export test record while unreachable").len(), 1, "{log}");
    assert_eq!(records(&log, "export test record after stop").len(), 1, "{log}");
    assert!(records(&log, "export test runtime warning")[0].contains("location=export_test.cc:1"), "{log}");

    // With the request's identifier and nothing of the trace its window named. A header that could not be read left
    // no record either.
    let under_window = records(&log, "export test record under a window's trace");
    assert_eq!(under_window.len(), 1, "{log}");
    assert_eq!(field(under_window[0], "run"), Some("window"), "{log}");
    assert!(
        !log.contains(WINDOW_TRACE) && !log.contains(WINDOW_SPAN),
        "the window's trace reached the file: {log}"
    );
    assert!(!log.contains("traceparent") && !log.contains("not-a-traceparent"), "{log}");
}
