## Why

`opai::catalogue()`'s own doc says what it is for: *"what a front end reads instead of restating the
model vocabulary in its own code."* It publishes every family, every variant's codename and label,
every precision that variant is published at, and the parameters it takes.

The GUI reads all of that to **describe** models - and then restates the vocabulary anyway to
**build** one. `crates/gui/src/enhance/operation.rs` validates `(family, codename, precision)`
against the catalogue in `published()`, and immediately re-decides the same facts against a hardcoded
`match codename.as_str()` naming four upscale models, plus two precision mappers (`float`, `osaka`)
that re-encode which precisions each kind of model admits.

The comment beside that match calls it *"the one place that names a model."* The catalogue is the
other place, and the two can disagree. When they do:

- **A model `opai` publishes cannot be run.** A new upscale variant passes `published()`, is offered
  by both choosers (which are catalogue-driven on both sides), and then falls into
  `_ => UnknownOperation::Model` - the application refusing a model it publishes, with a message
  saying upscale publishes no model by that name.
- **`osaka()` re-encodes "no FP32 build exists"**, which `variant.precisions` already states. Change
  one and the other is silently wrong.

The gap is structural rather than accidental: the catalogue is *descriptive*, so the only thing that
can turn a published name back into a runnable `Operation` is a table someone wrote by hand. This
change makes it constructive, so there is nothing left to keep in step.

## What Changes

- **A catalogue entry can build the operation it describes.** `opai` gains a way to go from
  `(family, codename, precision, parameters)` - the vocabulary it already publishes - to the
  `Operation` it names, or to a statement of what was wrong with the request.
- **The refusals are the library's**, since it is now the library deciding: a codename it does not
  publish, and a precision that variant is not published at.
- **The GUI stops naming models.** `published()`, `float()`, `osaka()` and the codename match all go;
  `Requested::resolve` becomes one call plus the scale clamping it already does.
- **Adding a model stops touching the GUI at all.** Today it is a directory in `opai` plus an arm in
  `operation.rs`. After this it is the directory.

## Crates touched

`opai` and `gui`.

The logic moves **into** `opai`, which is where the layering rule puts it: the CLI needs exactly the
same thing the GUI needs - it takes a codename and a precision from the user and has to turn them
into an `Operation` - and today it would have to write a second copy of this match to do it. That is
the test the project config gives for where something belongs, and this fails it in `gui` today.

This is a public API addition to `opai`, which is why it is its own change rather than folded into a
cleanup.

## rust-sak

Nothing applies, and nothing should. This is the library's own domain vocabulary - model names,
precisions and their constructors. rust-sak covers infrastructure (hashing, fetch, fs, image codecs,
memoization, telemetry, sysinfo); none of it has anything to say about what an upscale model is.

## Parity

No behavioural divergence, and no user-visible change. The Wails application composes and re-parses a
string like `up_kyoto_2x_fp32`; this project refused that in `add-gui-enhance-1-seam` (design.md D1)
in favour of a structured operation resolved against the catalogue. This change completes that
decision rather than altering it - the resolution stops being half-done.

The one behaviour that changes is a bug fix, not a divergence: a model the library publishes becomes
runnable instead of being refused.

## Task estimate

11 tasks. Inside the 15-task budget; not a slice of a series.
