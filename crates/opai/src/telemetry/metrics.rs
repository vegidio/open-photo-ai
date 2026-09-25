//! The instruments this crate measures its work with.
//!
//! Recorded in every binary, at one atomic operation each, and exported only once [`start`](super::start) has opened
//! `o11y`: an instrument in a process that never starts telemetry accumulates a number nobody reads.
//!
//! Every tag value comes from a fixed set the code enumerates: a [`Unit`](super::unit::Unit) or
//! [`Outcome`](super::unit::Outcome) name, an error's `kind`, a model's `model_tag`, an execution provider, or one of
//! the literals below. No path, identity, cache tag or error text is ever a tag.

use std::sync::LazyLock;

use rust_sak::o11y::metric::{self, Counter, Gauge, Histogram};

/// Bucket bounds in seconds, from a step served from the store to a TensorRT build or a model download.
///
/// `rust-sak`'s defaults run from 0 to 10 000 and suit milliseconds, not seconds.
const SECONDS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0,
];

/// How long a unit took, by `unit` and `outcome`.
pub(crate) static UNIT_DURATION: LazyLock<Histogram> =
    LazyLock::new(|| metric::histogram("opai_unit_duration_seconds").with_buckets(SECONDS));

/// How long a computed enhancement step took, by `model` and the `provider` its sessions ran on, or `mixed`.
pub(crate) static STEP_DURATION: LazyLock<Histogram> =
    LazyLock::new(|| metric::histogram("opai_step_duration_seconds").with_buckets(SECONDS));

/// Run cache lookups, by `kind` ([`STEP`] or [`ANALYSIS`]) and `result` ([`HIT`] or [`MISS`]).
pub(crate) static CACHE_LOOKUPS: LazyLock<Counter> = LazyLock::new(|| metric::counter("opai_cache_lookups_total"));

/// Sessions built on the CPU because the `requested` provider could not open the model.
pub(crate) static PROVIDER_FALLBACKS: LazyLock<Counter> =
    LazyLock::new(|| metric::counter("opai_provider_fallbacks_total"));

/// Failed units, by `unit` and the error's `kind`.
pub(crate) static FAILURES: LazyLock<Counter> = LazyLock::new(|| metric::counter("opai_failures_total"));

/// Sessions held in memory, by `provider`.
pub(crate) static RESIDENT_SESSIONS: LazyLock<Gauge> = LazyLock::new(|| metric::gauge("opai_resident_sessions"));

/// [`CACHE_LOOKUPS`]'s `kind` for an enhancement step.
pub(crate) const STEP: &str = "step";
/// [`CACHE_LOOKUPS`]'s `kind` for an analysis.
pub(crate) const ANALYSIS: &str = "analysis";
/// [`CACHE_LOOKUPS`]'s `result` for a lookup the store served.
pub(crate) const HIT: &str = "hit";
/// [`CACHE_LOOKUPS`]'s `result` for a lookup it did not.
pub(crate) const MISS: &str = "miss";

/// Counts one run cache lookup of `kind`.
pub(crate) fn cache_lookup(kind: &'static str, hit: bool) {
    CACHE_LOOKUPS.add_with_tags(1, &[("kind", kind), ("result", if hit { HIT } else { MISS })]);
}
