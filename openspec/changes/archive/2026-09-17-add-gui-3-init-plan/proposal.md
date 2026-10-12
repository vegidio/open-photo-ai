## Why

Screens 02 and 02b in the Claude Design project are the application's first run: a dialog listing
every component this machine needs, each with its size and its state - installed, downloading with
its own track, or queued - above an overall bar answering "how much longer?". They are the two
deliberate redesigns in that project rather than ports of the Wails dialog, which shows only a row
per dependency that has already started and no list at all.

`opai` cannot currently tell a front end what that list is. `InitOptions::on_progress` reports a
[`Progress`] per phase of each dependency it **installs**, and an install that is already current
and complete is skipped in silence - "nothing is transferred and nothing is reported, because there
is no work to report". So the four facts the dialog is built on are all unavailable:

- **Which dependencies this machine will install.** A Mac gets the runtime alone; a non-RTX NVIDIA
  card gets CUDA and cuDNN too; an RTX card gets TensorRT as well. The answer comes from
  `gpu::adapters()` and the platform, and `Plan::select` already computes it - privately.
- **Each dependency's published size**, which is the pinned value in `deps/artifact.rs`.
- **Which of them are already installed**, which is the fingerprint-and-`intact` rule in
  `deps/install.rs`.
- **The denominator** for "1 of 3 components installed".

A front end cannot derive any of them without re-probing the hardware and reimplementing the
install-record rule, and `cli` would need the same thing on its own first run - which is the test
this project's architecture states. So the reporting surface belongs here, and it is worth having on
its own: it is `opai`'s to state, it is testable without a window, and slices 4 and 5 then build a
dialog rather than a dialog plus an API.

## What Changes

- **`InitOptions::on_plan`**, a callback fired **once** with what this initialization is about to
  do: the dependencies in install order, each carrying its [`Dependency`] identity and the sum of
  its published source sizes. It fires after the claim is taken and the hardware probed, and before
  the first byte moves, so a front end can draw every row - including the ones not started yet -
  before anything happens. A caller that registers none is charged for none, exactly as
  `on_progress` already behaves.
- **A new public type** for one row of that plan, exposing the dependency and its total published
  size.
- **A terminal report for a dependency that is skipped.** A dependency found already current and
  complete stops being silent: it reports once, through the existing `on_progress` callback, saying
  it is in place and that nothing was transferred for it. This is a new [`Phase`] variant, which
  that `#[non_exhaustive]` enum admits additively.
- **No change to `InitError`.** With the plan known and installs running strictly in order, a front
  end attributes a failure to the first dependency not yet finished, and reads how far it got from
  the last [`Progress`] it held. Adding a dependency to every failing variant would be a second
  answer to a question the stream already answers.
- **No change to the install pipeline.** The "is it already installed?" check stays exactly where it
  is, inside `install_inner` - see `design.md` for why hoisting it to pre-compute the plan was
  rejected.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `initialization`: what an initialization reports. Gains requirements for the plan report - that it
  names every dependency this machine will install, in install order, with each one's published
  size, once, before anything is transferred - and for a skipped dependency reporting its outcome
  rather than nothing. The existing requirement that a skipped install transfers nothing and reports
  no *progress* is narrowed rather than reversed: there is still no work, and the one report says so.

## Impact

**Crates touched: `opai` only.** `crates/opai/src/lib.rs` (the `InitOptions` field and the one call
site after `Plan::select`), `crates/opai/src/progress.rs` (the new row type and the `Phase` variant)
and `crates/opai/src/deps/install.rs` (the early return reports before it returns). `gui` and `cli`
are untouched by this slice; `gui` consumes it in slice 4.

Nothing here is front-end logic. It is the hardware probe's conclusion, the pinned sizes and the
install record - all three already `opai`'s, none of them derivable above it.

**rust-sak: nothing applies, and nothing here is worth extending it with.** Its `fetch` module
already owns the transfer, the retries, the resume and the live progress this reports *over*; what
this change adds is `opai`'s own statement of which of its own dependencies it is about to install,
which is not general-purpose infrastructure. `sysinfo` supplies the GPU probe behind `Plan::select`
and is already a dependency. No new crate, Rust or otherwise.

**Public API: additive, no breaking change.** `InitOptions` is deliberately not `#[non_exhaustive]`
and every caller constructs it with `..Default::default()`, so a new field costs no call site.
[`Phase`] is `#[non_exhaustive]`, so a new variant is additive for external matchers. The existing
behaviour of `on_progress` for a dependency that *is* installed is unchanged, byte for byte.

**Deliberate divergence from the Wails app's behaviour: yes, and it is the point.** The Go dialog
renders `Object.entries(downloads)` - a row appears only once that dependency has reported, so the
list grows as the run proceeds and never mentions a dependency that was already installed. Screens
02 and 02b instead show the whole list from the start. That is the redesign this slice exists to make
possible, and the design project is the authority for it.

**Parity gaps this slice knowingly leaves open:** the dialog itself. Nothing in `gui` consumes any
of this until slice 4, so a first run still shows no dialog at all - the window comes up and the
application is not yet initialized from it. Slice 4 is where that is wired.

**Task estimate: 10**, inside the 15-task limit. This change is slice 3 of a series; `design.md`
carries the full slice list.
