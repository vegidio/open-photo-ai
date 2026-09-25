//! The span every unit of work is sent to the collector as, and the one call that closes it.
//!
//! A unit's span is opened with [`unit_span!`], under [`TARGET`], and ended by [`ended`] in the arm that already
//! writes the unit's closing record, with the duration that record measured. So the record, the span and the metric
//! agree on how the unit ended and how long it took.
//!
//! These spans go to the collector only. The file's filter excludes [`TARGET`], so none of their fields is ever written
//! onto a record in `opai.log`: see [`logging::format`](crate::logging).

use std::fmt::Display;
use std::time::Duration;

use tracing::Span;

use super::metrics;

/// The target every unit span is created under, and the one the file's filter excludes.
pub(crate) const TARGET: &str = "opai::unit";

/// Opens a unit's span: `name`, at `info`, under [`TARGET`], with `outcome` and `error` left for [`ended`] to record.
///
/// Further fields follow the name as they would in `tracing::info_span!`.
macro_rules! unit_span {
    ($name:literal $(, $($fields:tt)*)?) => {
        tracing::info_span!(
            target: $crate::telemetry::unit::TARGET,
            $name,
            outcome = tracing::field::Empty,
            error = tracing::field::Empty
            $(, $($fields)*)?
        )
    };
}

pub(crate) use unit_span;

/// A kind of unit with a closing record, as its span is named and its metrics are tagged.
///
/// A step, a session request and a cache access have no closing record of their own and are not here: their spans are
/// marked with [`mark`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unit {
    Initialize,
    RuntimeLoad,
    Install,
    SessionBuild,
    Enhancement,
    Analysis,
    AutoAnalysis,
    ImageLoad,
    ImageSave,
    ImageEncode,
    ImageProbe,
    ImageProbeRaw,
    ImageIdentity,
}

impl Unit {
    /// Every unit, for the tests that pin the set of names a metric can be tagged with.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 13] = [
        Self::Initialize,
        Self::RuntimeLoad,
        Self::Install,
        Self::SessionBuild,
        Self::Enhancement,
        Self::Analysis,
        Self::AutoAnalysis,
        Self::ImageLoad,
        Self::ImageSave,
        Self::ImageEncode,
        Self::ImageProbe,
        Self::ImageProbeRaw,
        Self::ImageIdentity,
    ];

    /// The unit's name: its span's name, and its `unit` tag.
    ///
    /// The same string in both places, so a metric and a trace are queried by one name.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::RuntimeLoad => "runtime_load",
            Self::Install => "install",
            Self::SessionBuild => "build",
            Self::Enhancement => "enhancement",
            Self::Analysis => "analysis",
            Self::AutoAnalysis => "automatic_analysis",
            Self::ImageLoad => "image_load",
            Self::ImageSave => "image_save",
            Self::ImageEncode => "image_encode",
            Self::ImageProbe => "image_probe",
            Self::ImageProbeRaw => "image_probe_raw",
            Self::ImageIdentity => "image_identity",
        }
    }
}

/// How a unit ended, as its closing record already says.
pub(crate) enum Outcome<'a> {
    Finished,
    /// Cancelled or shut down. Never marked as failed, for the reason its record is not a warning.
    Stopped,
    Failed {
        /// The error's stable name, which is what the failure is counted by. Never its text.
        kind: &'static str,
        /// The error itself, which the span carries as the reason it failed.
        error: &'a dyn Display,
    },
}

impl Outcome<'_> {
    /// `finished`, `stopped` or `failed`: the span's `outcome` and the metric's tag.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Finished => "finished",
            Self::Stopped => "stopped",
            Self::Failed { .. } => "failed",
        }
    }
}

/// Closes `unit`'s span with `outcome`, beside the record that closes the unit, and measures it.
///
/// `duration` is the one that record measured, so the record, the span and the metric agree. A failure is also
/// counted by its `kind`.
pub(crate) fn ended(unit: Unit, span: &Span, duration: Duration, outcome: Outcome<'_>) {
    mark(span, &outcome);

    metrics::UNIT_DURATION
        .record_with_tags(duration.as_secs_f64(), &[("unit", unit.as_str()), ("outcome", outcome.as_str())]);

    if let Outcome::Failed { kind, .. } = outcome {
        metrics::FAILURES.add_with_tags(1, &[("unit", unit.as_str()), ("kind", kind)]);
    }
}

/// Records `outcome` on `span`, and `error` where it failed, which the bridge turns into a failed status.
///
/// [`ended`] without the metrics, for the spans with no closing record of their own: a step, a session request and a
/// cache access.
pub(crate) fn mark(span: &Span, outcome: &Outcome<'_>) {
    span.record("outcome", outcome.as_str());

    if let Outcome::Failed { error, .. } = outcome {
        span.record("error", tracing::field::display(error));
    }
}

#[cfg(test)]
mod tests;
