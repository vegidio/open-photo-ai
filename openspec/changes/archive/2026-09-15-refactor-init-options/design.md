## Slices

This change is **not** part of a slice series. It is a single self-contained refactor, inserted
because the series' next change would otherwise have to carry it and exceed the project's task limit.

It sits between two changes of the perf/logging arc, so that series is repeated here for whoever
picks this up. The arc is seven changes; this one is none of them, and it blocks row 5.

| # | Change | Delivers | Depends on | State |
|---|--------|----------|-----------|-------|
| 1 | `add-single-instance-1-lock` | At most one OPAI process per user: an advisory lock taken in `Opai::initialize`, the holder recorded, the second process refused | none | **done** (archived `2026-09-14`) |
| 2 | `add-single-instance-2-gui` | The GUI raises its existing window instead of refusing | 1, and a GUI with a window | **done** (archived `2026-09-14`) |
| 3 | `add-logging-1-file` | The sink: `tracing` through `opai`, `<config>/logs/opai.log`, daily rotation, the session divider and header, the per-record `app=` field, `APP_NAME` | 1 | **done** (archived `2026-09-15`) |
| 4 | `add-logging-2-records` | The instrumentation sweep across the run, the session registry, the installers and the cache | 3 | **done** (archived `2026-09-15`) |
| — | **`refactor-init-options`** | **This change: `InitOptions`, the identity as a string, its default and its validation** | **1, 3** | **current** |
| 5 | `add-perf-1-opai` | `release_sessions` made public, the `(requested, actual)` execution provider surfaced out of `process`, model trust as a field of `InitOptions` | 3, and this change | pending |
| 6 | `add-perf-2-harness` | `crates/perf` → the `perftest` binary | 5 | pending |
| 7 | `add-perf-3-tui` | The `ratatui` inline viewport | 6 | pending |

## Context

See proposal.md — Why. Four things read out of the code shape this:

- **`ProcessOptions` is the pattern, and it documents itself.** Its doc comment settles the bundle
  over more parameters, `Default` plus `..Default::default()` over `#[non_exhaustive]`, and `None`
  for "all defaults". None of that is reopened here; this change applies it to the other entry point.
- **The holder record is `<app> <pid>` on one line, split on the first space.** `instance.rs`'s
  `record` writes `format!("{} {}\n", app, pid)` and `read_holder` does `text.trim().split_once(' ')`,
  with the comment that *"a line with a third field is a format this build does not know and is
  refused along with everything else unrecognised"*. The enum's strict `FromStr` is currently what
  stops an ambiguous identity from ever being written. Remove the enum and that guard has to be
  replaced at the other end — D3.
- **`read_holder`'s degradation is deliberate and load-bearing.** Its doc: *"The contents decide
  nothing… every way of failing returns `None`, which is a refusal that names no holder. None of them
  is an error, and none of them is ever a grant."* Nothing in this change may weaken that, which is
  why D3 validates on the way in rather than relaxing anything on the way out.
- **The identity is declared twice per binary today.** `gui` and `cli` each call
  `logging::init(APP_NAME, App::Gui)` and would separately pass an `App` to `initialize`. That
  duplication is pre-existing and this change does not remove it — see Non-Goals.

## Goals / Non-Goals

**Goals:**

- One options bundle on `initialize`, so the argument the next change adds costs no call site.
- An identity an embedder can declare honestly, and a default that names no caller as something it is
  not.
- Delete more than is added. The enum, its parse, its error type, its `ALL` array and its spelling
  table go; a validation function and three constants arrive.

**Non-Goals:**

- **Unifying the two declarations.** A binary still tells `logging::init` who it is and then tells
  `initialize` again. Worth fixing, but it is a different question — the sink is deliberately
  installed *before* `Opai` exists, which is what puts a second launch's refusal inside the log
  rather than ahead of it — and folding it in here would mean redesigning that ordering.
- **Model trust.** It is a field of `InitOptions` in the next change, not this one. This change adds
  the bundle; that change adds a field to it, which is the whole point of the bundle.
- **Changing what a refusal or a record looks like for this project's own binaries.** `gui`, `cli`
  and `perf` render exactly as they do today, which is what keeps every existing test honest.

## Decisions

### D1. A string, not a four-variant enum

The alternative considered first, and at some length, was keeping the enum and adding
`Custom(String)` to it. It works, and it round-trips — but only as `Custom(String)`, not
`Custom(&'static str)`, because `FromStr` reads the holder back out of a file and arbitrary text
cannot become a `&'static str` without leaking. Taking the `String` then costs `App`'s `Copy`, the
`const` on `as_str`, and the `[Self; 3]` in `ALL`.

What that buys is nothing the string does not already give. The enum's stated justification is
closedness, and a variant carrying a free-form name is exactly the "fourth value without anyone
deciding there should be one" the doc warns about — admitted through the front door. Once any text
can be an identity, `Gui | Cli | Perf | Custom(String)` is a `String` plus a spelling table, a parse,
an error type and an array, and four ways to write the same value (`App::Gui` and
`App::Custom("gui".into())` are different values that render identically and would be recorded
identically).

**What is genuinely lost, and what replaces it.** Exhaustive matching: nothing matches on `App`
except `as_str`, which disappears with it. Typo safety: recovered by exporting the three spellings as
constants — a reference to a misspelled constant still fails to compile, which is where the real
protection was. The strict parse that narrowed what a corrupt record could be mistaken for: replaced
by D3, which moves the check to where the identity is declared instead of where it is read back.

### D2. The default identity is the name the caller already gave

`InitOptions::default()` cannot name one, because the name is not known until `initialize` is called
with it. So the field is an `Option` inside the bundle and the defaulting happens in `initialize`,
where `name` is in hand: an identity that was not declared is the application name.

Each alternative invents something. A fixed `"cli"` labels a desktop application as a terminal one on
every line of its log and tells a refused user to close something that is not running. A fixed
placeholder like `"app"` makes two different embedders on one machine indistinguishable in a file
whose entire purpose is telling interleaved sessions apart. The name the caller supplied is neither
invented nor ambiguous, and it is already validated as a directory segment by `config::app_dir`.

That validation is **not** a substitute for D3's, and the defaulting does not get to skip it. The two
rules overlap and neither contains the other: `config::app_dir` refuses a path separator and a NUL,
which D3 does not name, and permits a space, which D3 refuses — and a space is the one thing the
holder record cannot carry. So the identity `initialize` resolves goes through D3 whether it was
declared or defaulted, and `InitError::InvalidApp` can name the application name itself. A caller
whose name carries a space is refused at initialization rather than recorded as a line that reads
back as a different identity beside an unparseable process id.

The cost is verbosity for this project's own reverse-DNS name, and it is not paid: `gui`, `cli` and
`perf` all declare, so the default is reached only by an embedder, for whom its own name is the
useful answer.

### D3. The identity is validated where it is declared, not coped with where it is read

Non-empty, no whitespace, no control characters, refused with an error carrying the rejected text and
a reason — the shape `InitError::InvalidName` already has for the application name, including its
`reject(reason)` closure in `config.rs`.

The rule is derived from the record format rather than chosen for tidiness. `<app> <pid>` split on
the first space means an identity containing a space writes a line that reads back as a different
identity and an unparseable process id, so `read_holder` returns `None` — a refusal that names no
holder. That is safe, and it is silent: the symptom is a user being told something is running without
being told what, which is the one thing the holder record exists to prevent. An empty identity is
worse in a different way — it reads back as a holder whose name is nothing, rendering `(, pid 4821)`.
Control characters would break the line itself and, in the log, the one-record-per-line property the
formatter rests on.

**Validated on the way in rather than relaxed on the way out** because `read_holder`'s contract must
not move. It answers `None` to everything it cannot make sense of and that is never a grant; adding
an escape or a quoting scheme to carry a space would mean the reader deciding more, not less.

**Alternative rejected: sanitising instead of refusing.** Replacing spaces or truncating would let
two distinct applications record the same identity, which is the collision the validation exists to
avoid, and would do it quietly.

### D4. `Holder` keeps both fields and loses `Copy`

`Holder { app: String, pid: u32 }` is `Clone` rather than `Copy`. Nothing depends on its `Copy`:
`InitError` derives `Debug, Error` and not `Clone`, so `AlreadyRunning { holder: Option<Holder> }` is
unaffected, and the type is constructed in one place and rendered in one place.

The pair stays a struct for the reason already written on it — *"a process id with no application
does not say what to close, and an application with no process id does not say which copy of it."*

### D5. The three spellings stay `&'static str` constants, exported

`gui`, `cli` and `perf` are written once, as constants on the crate, and the front ends pass those
rather than literals. The property this preserves is the one the enum's tests pin today: the
spellings are what a refusal prints and what a log record carries, so renaming one is a visible change
that has to be made in a single place.

The tests that currently walk `App::ALL` become tests over an array of those three constants — the
round-trip through the holder record, and the assertion that the spellings are the published ones and
are distinct. `UnknownApp` and its test go; there is no longer a parse that can fail.

## Risks / Trade-offs

**A typo in a string identity is no longer a compile error** → Only for a caller writing a literal.
The three first-party binaries use the constants in D5, so the compile-time protection is unchanged
where it existed; an embedder naming itself was always going to supply text.

**Two embedders could declare the same identity** → They could, and the same was true of two copies
of `gui`. The identity names an application, not an instance; the process id beside it is what names
the instance, which is why they are recorded together.

**D3 makes initialization fail where it previously could not** → A new failure mode on a path that is
run once per launch, with a rejected value that is a compile-time constant for every caller in this
workspace. The `Result` is already there and already carries `InvalidName` for the sibling case.

**The bundle lets a caller set a field and forget another** → The point of `Default` plus
`..Default::default()`, and the reason `#[non_exhaustive]` is deliberately not used, both as
`ProcessOptions` already documents.

**A change this size touching single-instance and logging could regress either** → Both capabilities
have full spec coverage and both suites are self-contained: `instance.rs` drives real re-executed
child processes, and `logging` renders records to a buffer. Every existing scenario for the three
first-party spellings stays literally true, which is what makes a regression visible rather than
arguable.

## Crates, dependencies and validation

**Crate split.** `opai` holds the identity, its default, its validation and the bundle. `gui` and
`cli` change only at their two call sites each, passing a constant `opai` exports — no logic moves
into a front end, and nothing here is duplicated between them.

**`rust-sak`.** Nothing used and nothing needed. Application identity is not something any of its
modules covers, and this change adds no infrastructure: the validation is a character-class check in
`std`, of the same shape `config.rs` already applies to the application name. No gap, so nothing to
extend there.

**Other crates.** None introduced; `thiserror` and `tracing` are already dependencies.

**Platforms and execution providers.** No execution provider is involved and no model is opened. The
single-instance suite re-executes real child processes and runs on macOS, Linux and Windows in CI as
it has since slice 1; the identity's round-trip through the holder record is exercised there. The
logging field is rendered into a buffer and is platform-independent. D3's rejection of control
characters is checked on all three, since the line-oriented record format is the same on each.
