## Why

Since slice 3, every unit of work in `opai` closes exactly once, as finished, stopped or failed, with its duration and
subject. The collector still receives those units only as log lines. The one span `opai` opens is `session_build`, so
Grafana has no trace to show:
- where an enhancement spent its minute (install, session build, a step, or the cache);
- that nine requests waited on one build;
- which of three `warn`s belongs to which run.

It also has no numbers to chart across the installed base: durations, cache hit rate, how often a GPU falls back to
the CPU, failures by kind, and how many models sit in memory. This slice adds both, now that the records they hang off
are right.

## What Changes

The whole feature needs well over 15 tasks, so it is split into seven slices (see design.md - Slices). This proposal is
the fourth, at **14** tasks.

**In `../rust-sak` (a separate repository), in `o11y::metric`:**

- **A public read-back on each instrument.** It reads a counter's total, a gauge's value, and a histogram's count and
  sum for one tag set. Without it, a consumer cannot test that its code recorded a metric, because `metric::snapshot`
  is `pub(crate)`. Released as the next CalVer tag with the owner's go-ahead. The pins move in `opai`, `gui` and
  `perf` together.

**In `opai`: spans on every unit of work, sent to the collector only**

- **A span per unit.** Each of these is a span: initialization (and the runtime load inside it), a dependency or model
  install, a session request, a session build, an enhancement, each step of one, an analysis, an automatic analysis,
  the six public image operations, and the run cache's reads and writes. They nest the way the work does, so one
  enhancement is one trace.
- **A span says how its unit ended.** It carries an `outcome` of `finished`, `stopped` or `failed`. A failed unit is
  also marked as an error, which Tempo shows as a failed span. A stopped unit is not.
- **One build, many waiters.** A session build is one span, whichever request started it. Every other request that
  waited on that build links to it rather than appearing to have built the model itself.
- **`opai.log` is unchanged.** The file layer's filter drops these spans, so their fields are never appended to the
  records inside them. The existing `session_build` span keeps decorating ONNX Runtime's diagnostics in the file, as
  it does today. This was the owner's choice; see design.md D2.

**In `opai`: metric instruments**

- The duration of every unit, by unit and outcome.
- The duration of each enhancement step, by model and the provider that actually ran it.
- Run cache lookups, by kind (step or analysis) and result (hit or miss).
- Execution-provider fallbacks, by the provider that was attempted.
- Failures, by unit and a stable name for the error's kind.
- Resident sessions, by provider.

Every tag value comes from a small, fixed set. No path, identity, cache tag or error text is ever a tag.

## Capabilities

### New Capabilities

None. Spans and metrics reach the same collector the records do, under the same start, stop and floor rules. Slice 2
put that destination in `logging`, and this slice extends it there.

### Modified Capabilities

- `logging`:
  - Every unit of work reaches the collector as a span, nested under the unit that caused it, saying how it ended.
  - A failed unit's span is marked failed, and a stopped one is not.
  - Requests that waited on one build are linked to it, and the build is one span.
  - The file does not carry the collector's spans.
  - Spans are bounded by what was done, not by how large the picture was.
  - Metrics for durations, cache lookups, fallbacks, failures and resident sessions are sent while telemetry runs,
    with tags drawn only from fixed sets.

  The existing requirements are unchanged.

## Impact

**Crates touched:**
- `opai`: a new crate-private module holding the unit-span helper and the instruments (`telemetry/unit.rs` and
  `telemetry/metrics.rs`). The file filter in `logging/format.rs`. The span and metric calls in each bracket slice 3
  centralised: `lib.rs`, `runtime.rs`, `deps/install.rs`, `sessions/`, `inference/process/`, `inference/execute.rs`,
  `autopilot/`, `image/` and `cache.rs`. A crate-private `kind()` on each error type.
- `gui` and `perf`: the `rust-sak` pin only.
- `cli`: no change. It records the same spans and metrics and never starts telemetry, so nothing leaves the process.

No logic goes into a front end. Spans and metrics describe `opai`'s work, and a second front end would need exactly
the same ones.

**`rust-sak`:** `o11y` already covers the span export, parentage across threads, links, status, the metric
instruments, cumulative export and the series cap. The only gap is reading an instrument's value back, which any
consumer testing its own metrics needs, so it is filled in `rust-sak` itself rather than worked around here.

**Dependencies:** no crate is added. The `rust-sak` pin moves to the new tag.

**Parity divergence from the Go application:** none a user can see. The Go application exported error logs only. This
series adds traces and metrics, as already decided and recorded in design.md - Slices.

**Not in scope:**
- `gui`'s command spans, which will parent these units under the request that caused them. That is slice 5, and it
  must make its span visible to the file deliberately (design.md - Slices).
- The Analytics switch and CI secrets (slice 5).
- The frontend and Faro (slices 6 and 7).
- Spans inside `imaging`, or per pass, tile or region.
