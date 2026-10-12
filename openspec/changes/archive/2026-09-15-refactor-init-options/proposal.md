## Why

`Opai::initialize(name, app, on_progress)` asks every caller for three positional arguments, two of
which most callers have no opinion about. The next change adds a fourth — whether the model files on
disk are trusted — and a fifth is easy to foresee. `Opai::process` already solved this shape: it
takes the two things with no sensible default and bundles the rest behind `Option<ProcessOptions>`,
whose own doc comment records why — *"a field added later is source-compatible for every caller…
which is how `cache` arrived without moving a call site."* Initialization deserves the same
treatment, and it is cheaper to do before the fourth argument lands than after.

The second half is `App`. It exists as a closed three-variant enum, and its doc justifies that
closedness and nothing else: *"Closed rather than a `&str`: there are exactly three binaries, and a
free-form name would let a fourth value in without anyone deciding there should be one."* But an
application embedding this library is not one of those three, and there is no honest value for it to
declare. Once any embedder can name itself, the set is not closed, and a four-variant enum whose
fourth variant carries a free-form name is strictly more machinery than the string it wraps — the
same expressive power, plus a parse, an error type, an `ALL` array and a spelling table.

## What Changes

- **BREAKING: `Opai::initialize` takes `(name, Option<InitOptions>)`.** `app` and `on_progress`
  become fields of `InitOptions`, which has a `Default`, so `None` means every default and
  `..Default::default()` sets one field without naming the others — the idiom `ProcessOptions`
  already documents.

- **BREAKING: the application identity becomes a string.** `App` and `UnknownApp` are removed, along
  with `App::ALL`, `App::as_str` and the `FromStr` parse. `Holder::app` becomes an owned string.

- **The three first-party spellings stay in one place**, as exported string constants rather than
  enum variants, so `gui`, `cli` and `perf` are still written once and referring to one still fails
  to compile if misspelled.

- **An undeclared identity is the application name the caller already gave.** `initialize` already
  takes `name`, the name every directory is resolved from. A caller that declares no identity is
  recorded and logged under it. Nothing is invented and no caller is labelled as a binary it is not.

- **The identity is validated where it is declared.** Non-empty, no whitespace, no control
  characters — refused with an error naming what was rejected and why, in the shape
  `InitError::InvalidName` already uses for the application name. This is not tidiness: the holder
  record is `<app> <pid>` on one line and is read back by splitting on the first space, so an
  identity containing one costs a refused user the name of the application they need to close.

- **BREAKING: `logging::init` takes the identity as a string** for the same reason, and the `app=`
  field carries it unchanged.

- **Two behaviours widen, and no behaviour is removed.** A refusal names whatever the holder recorded
  rather than one of three known applications; a log record carries whatever the process declared
  rather than one of three spellings. Every existing guarantee about what happens when the record is
  absent, empty or unreadable is unchanged.

## Capabilities

### New Capabilities

None. Both behaviours that change belong to capabilities that already have specs.

### Modified Capabilities

- `single-instance`: a refusal names the application the holder recorded, which is no longer one of
  three fixed applications; and what an identity may contain for that record to be legible.
- `logging`: every record carries the identity the process declared, which is no longer one of three
  fixed spellings.

## Impact

**Crates:** `opai`, `gui` and `cli`. The two front ends each declare an identity twice today — once
to `logging::init` and once to `Opai::initialize` — and both call sites change. Nothing moves *into*
a front end: the identity type, its validation and its default all live in `opai`, and the front ends
pass a constant.

**rust-sak:** nothing needed and nothing duplicated. This change deletes more code than it adds; the
validation is a character-class check over a `&str` in `std`, the same shape `config.rs` already uses
to validate the application name. No `rust-sak` module covers application identity, and none should.

**Public API:** `App` and `UnknownApp` are removed; `InitOptions` and the three spelling constants are
added; `Holder` changes shape; `Opai::initialize` and `logging::init` change signature.
`crates/opai/tests/public_api.rs` names all of these and is updated with them. `gui` and `cli` are the
only other consumers and neither is shipping.

**Code:** `app.rs` (most of it deleted), `instance.rs` (the record, the read-back and their tests),
`logging/mod.rs` and `logging/format.rs` (the field), `lib.rs` (the signature and the defaulting),
`error.rs` (the rejection), plus the two front-end `main`s.

**Divergence from the Go implementation:** none that is new. The reference has no equivalent of
either — it takes no application identity at initialization at all, and `shared.AppName` is a single
constant every binary shares — so there is no behaviour here to match or to diverge from. What this
change touches is vocabulary this project invented in the single-instance slice.

**Why now, and why not inside the change that needed it:** `add-perf-1-opai` adds the fourth
argument, and it is that change's task budget that forces this out into its own. Folding both
together came to sixteen tasks against a hard limit of fifteen. Doing this first also means
`initialize`'s signature breaks once rather than twice in consecutive changes.

**Estimated tasks:** 10.
