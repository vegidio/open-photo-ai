//! Sending records to this project's collector.
//!
//! The subscriber [`logging::init`](crate::logging::init) installs already carries a layer for the collector, and it
//! sends nothing until [`start`] is called. `start` is the application's decision, `stop` is the matching one at exit,
//! and neither is ever made by this library on its own: a consumer that never calls `start` sends nothing, and so
//! does any build compiled without a collector.
//!
//! What is sent is what the file receives at its default level — this project at `info`, ONNX Runtime at `warn` — plus
//! the spans those records were emitted in, as traces. `RUST_LOG` does not change it. See
//! [`logging::format`](crate::logging) for the filter.

// The export itself — batching, the export thread, resource enrichment, the session id and the gate every hook checks
// — is `rust-sak`'s `o11y`. What lives here is this project's configuration of it, and the two records it adds to the
// file: that sending started, and that it failed.

use std::sync::atomic::{AtomicBool, Ordering};

use rust_sak::o11y::trace::SpanContext;
use rust_sak::o11y::{self, Config, Environment, Level, O11yError};
use thiserror::Error;

pub(crate) mod metrics;
pub(crate) mod unit;

/// The target telemetry's own export-failure records are emitted under, which the collector's filter excludes.
pub(crate) const EXPORT_TARGET: &str = "opai::telemetry::export";

// Baked in at compile time rather than read at run time, as the reference implementation's CI string-replaced them in.
// `rustc` records an `option_env!` read in the dep-info, so cargo rebuilds this crate when either variable changes.
// Prefixed `OPAI_` so neither is mistaken for the `OTEL_EXPORTER_OTLP_*` variables OpenTelemetry SDKs read at run time.
/// The collector's base URL; `/v1/logs` and `/v1/traces` are appended to it.
const ENDPOINT: Option<&str> = option_env!("OPAI_OTEL_ENDPOINT");
/// The whole `Authorization` header value the collector expects.
const AUTH: Option<&str> = option_env!("OPAI_OTEL_AUTH");

/// The service every record is reported under, which the Go application's dashboards already query.
const SERVICE_NAME: &str = "opai";

/// Whether [`start`] has been called in this process.
static STARTED: AtomicBool = AtomicBool::new(false);

/// Whether an export failure has been recorded in this process yet.
static EXPORT_FAILED: AtomicBool = AtomicBool::new(false);

/// What [`start`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sending {
    /// Records are being sent to the collector.
    Yes,
    /// This build names no collector, so nothing will be sent. The ordinary state of a development build.
    NoCollector,
}

/// Why [`start`] did not start sending.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TelemetryError {
    /// Telemetry was already started in this process, whether or not that start found a collector.
    ///
    /// The first start stays in effect. A caller seeing this should carry on rather than retry.
    #[error("telemetry has already been started in this process")]
    AlreadyStarted,

    /// The collector's address or credentials this build was compiled with were refused.
    ///
    /// Only a malformed build secret produces it. Nothing is sent, and the application should run on.
    #[error("the collector this build names was refused: {0}")]
    Invalid(#[source] O11yError),
}

/// Starts sending records to this project's collector.
///
/// Call it once, after [`logging::init`](crate::logging::init): records emitted before it are written to the file
/// only. The first record sent is a session record carrying the version, the operating system and the architecture.
///
/// A build compiled without `OPAI_OTEL_ENDPOINT` and `OPAI_OTEL_AUTH` returns [`Sending::NoCollector`], opens no
/// connection, and is not an error.
///
/// # Errors
///
/// [`TelemetryError::AlreadyStarted`] if this process already called it, and [`TelemetryError::Invalid`] if the
/// collector this build names was refused. Neither is a reason to stop the application.
pub fn start() -> Result<Sending, TelemetryError> {
    // Latched first, like `logging::init`, and never cleared: a second call is refused whatever the first one found,
    // so `NoCollector` cannot be read as "try again later". `o11y` itself can start once per process.
    if STARTED.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return Err(TelemetryError::AlreadyStarted);
    }

    begin(ENDPOINT, AUTH)
}

/// Stops sending, after sending what is held.
///
/// Blocks for at most one export timeout (10 s). Records emitted afterwards still reach the file. Safe to repeat, and
/// safe to call when telemetry never started.
pub fn stop() {
    o11y::shutdown();
}

/// Runs `open` so that the span it opens continues the trace `traceparent` names, as a child of the span it names.
///
/// For a request whose caller started a trace of its own and sent it as a W3C `traceparent`: the span `open` creates
/// with no span of this process above it joins the caller's trace, and everything beneath it follows. A span that
/// already has a parent here keeps it.
///
/// Where `traceparent` is `None`, or not a W3C `traceparent`, `open` runs as it would without it and the span roots a
/// new trace. A malformed header is not recorded: it comes from the caller's own code, where a test catches it, and is
/// not a condition worth a record in every request. The header never becomes a field, so it never reaches the file.
pub fn with_parent<T>(traceparent: Option<&str>, open: impl FnOnce() -> T) -> T {
    match traceparent.and_then(SpanContext::from_traceparent) {
        Some(context) => o11y::tracing::with_parent(context, open),
        None => open(),
    }
}

/// [`start`]'s body once the latch is held, with the collector passed in so a test can supply one.
pub(crate) fn begin(endpoint: Option<&str>, auth: Option<&str>) -> Result<Sending, TelemetryError> {
    // Both, or no collector: this project's collector always authenticates, so an endpoint alone would only be refused.
    let (Some(endpoint), Some(auth)) = (endpoint.filter(|v| !v.is_empty()), auth.filter(|v| !v.is_empty())) else {
        // So that a file whose session never reached the collector says why.
        tracing::debug!(
            version = crate::version(),
            os = std::env::consts::OS,
            arch = std::env::consts::ARCH,
            sending = false,
            "telemetry started"
        );
        return Ok(Sending::NoCollector);
    };

    o11y::init(config(endpoint, auth)).map_err(|error| match error {
        O11yError::AlreadyInitialized => TelemetryError::AlreadyStarted,
        other => TelemetryError::Invalid(other),
    })?;

    // The file's session header was emitted before the gate opened, so without this the collector's first record from
    // a session would be whatever happened to come next.
    tracing::info!(
        version = crate::version(),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        "telemetry started"
    );

    Ok(Sending::Yes)
}

/// This project's configuration of `o11y`. Everything not named keeps `o11y`'s default: a 5 s flush, batches of 512,
/// 8192 records buffered and a 10 s export timeout. Geolocation stays off, as it was in the Go application.
fn config(endpoint: &str, auth: &str) -> Config {
    // From the build profile rather than from a third variable: a release build is what users run.
    let environment = if cfg!(debug_assertions) { Environment::Development } else { Environment::Production };

    Config::builder(endpoint, [("Authorization", auth)])
        .service_name(SERVICE_NAME)
        .service_version(crate::version())
        .environment(environment)
        .min_level(Level::Info)
        .on_export_error(|error| record_export_failure(&EXPORT_FAILED, error))
        .build()
}

/// Records a failed export in the file: at `warn` the first time `reported` sees one, at `debug` after that.
///
/// So an unreachable collector costs one line per session at the default level, not one every flush. Under
/// [`EXPORT_TARGET`], which the collector's filter excludes: sending it would queue a record about the queue failing
/// onto the queue that is failing.
///
/// A payload the collector refused also carries the collector's own answer as `response`. The error's `Display` names
/// only the status, and a `429` or a `400` says what happened without saying why: which limit was hit, or which sample
/// was refused, is in the body. `o11y` already truncates it to something loggable.
///
/// Runs on `o11y`'s export thread, and must not panic, which would end that thread: the error is formatted through
/// `Display`, which does not.
fn record_export_failure(reported: &AtomicBool, error: &O11yError) {
    let response = match error {
        O11yError::ExportRejected { body, .. } if !body.trim().is_empty() => Some(body.trim()),
        _ => None,
    };

    if reported.swap(true, Ordering::Relaxed) {
        tracing::debug!(target: EXPORT_TARGET, error = %error, response, "sending telemetry failed");
    } else {
        tracing::warn!(target: EXPORT_TARGET, error = %error, response, "sending telemetry failed");
    }
}

#[cfg(test)]
mod tests;
