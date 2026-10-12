## Why

The window can open a photograph, draw it at any zoom and compare it against itself, but it cannot
yet ask for it to be enhanced: nothing in `gui` calls `Opai::process`. Everything that call needs is
already built - the operation carriers, the progress reports, the cancellation token, the run cache -
and the only thing missing is the seam between them and the webview.

That seam is worth cutting on its own, ahead of any screen, because three of its four parts are
decisions the interface cannot take back later: how an operation is spelled on the wire, how a run in
flight is stopped, and where the enhanced pixels live. A slice that drew the sidebar first would be
settling all three by accident.

## What Changes

- **A run can be asked for.** A command takes a source image's identity, a chain of operations and
  the provider to run on, and answers with the enhanced result's identity and dimensions.
- **An operation crosses the boundary structured, not as a string.** The reference composes
  `up_kyoto_2x_fp32` and parses it back; `catalogue` already publishes the codename, the precisions
  and the parameters that string encodes, so the interface names them directly instead. Upscale is
  the one shape carried today - it is the one family this application can offer - and it is carried
  as a tagged variant so a second family is an arm rather than a reshaping.
- **A run in flight can be stopped from the window.** Cancellation is `ProcessOptions::cancel` and
  already cooperative between tiles in `opai`; what is new is a way to reach it from a webview, which
  Tauri gives nothing equivalent to Wails' cancellable call for. The window supplies the run's
  identifier, so a cancellation that arrives before the run has started still lands on the right run.
- **A second run supersedes the first.** The application holds one run and one enhanced result at a
  time, and asking for a new one cancels what it displaces. This is not an optimisation: it is what
  makes "only one run at a time" a property of the backend rather than a promise the interface makes.
- **A run reports what it is doing.** An event carries which operation, whether its model is being
  installed or being run, how far the whole chain has got, and - while a model is being fetched - how
  far that fetch has got on its own terms. Reports are throttled where they are emitted, since a
  tiled run produces thousands of them and the interface can draw a hundred positions.
- **The enhanced pixels are served over the existing URI scheme**, addressed by the result's own
  composed identity, at whatever size the interface is drawing them. Nothing is base64-encoded onto
  the IPC channel and no second protocol is introduced.
- **The set the window may read from widens** from the files the user opened to those plus the
  results this application produced from them. Nothing else on the machine becomes reachable, and a
  result that has been superseded stops being.

No screen changes. The Add enhancement button stays unavailable; it goes live in slice 2, which is
what the sidebar's own note says should happen.

## Capabilities

### New Capabilities

- `gui-enhance`: asking for a chain of operations to be run over an open image, stopping one in
  flight, what a run reports while it works, and how its result reaches the window.

### Modified Capabilities

- `gui-images`: the requirement *Only files the user has opened can be served* widens from admitted
  files to admitted files plus the enhanced results produced from them, and gains the refusal for a
  result that is no longer held.

## Impact

**Crates: `gui` only.** `opai` is not edited - `process`, `ProcessOptions`, `InferenceProgress` and
`Scale` are all consumed as they stand, which is the test this slice passes rather than the exception
it takes. `cli` is untouched.

The frontend work here is the `ipc/` module and its tests alone. It sits in `gui` for the reason
every other `ipc/` module does: it is the spelling of a Tauri command, which is not something the
terminal front end could want. The conversion from the wire shape to `opai::Operation` is in Rust for
the opposite reason - it is the only place the two vocabularies meet, and a front end composing
carriers would be a second place for them to disagree.

**rust-sak: nothing used directly, and nothing missing.** This slice hashes, downloads, extracts and
decodes nothing of its own. The run reaches `fetch` and `crypto` when a model has to be installed and
`memo` when the cache is read, all through `opai`, exactly as it already does for the CLI. The
enhanced result is encoded by the rendition path built in `add-gui-images-1-ipc`, which reaches
rust-sak's `image` through `opai`.

**New crates: none. New npm packages: none** - see design.md D6 for why the store this unblocks does
not need `immer`.

**Affected code:** a new `gui/src/enhance.rs`; `gui/src/images.rs` for the widened resolution behind
the scheme; `gui/src/lib.rs` for the handler and the managed state; a new
`gui/frontend/ipc/enhance.ts`.

**Tasks: 15.** At the limit rather than under it - the seam is four separable pieces (the wire shape,
the slot, the two commands, the reporting) and each earns its own verification. Slice 2 is estimated
at fifteen too; design.md - Slices records the cut to make if it overflows.

**Deliberate divergences from the reference's behaviour:**

- **A superseded run is cancelled by the backend, not only by the window.** The Wails application
  relies entirely on its frontend calling `p.cancel()` in an effect cleanup. Here the single-run slot
  cancels what it displaces as well, so an unmount or a frontend fault cannot leave a run holding
  sessions and burning a GPU. The user-visible result is the same; what changes is that it stays the
  same when the interface misbehaves.
- **A cached operation is distinguishable from a run one.** `opai` reports `Stage::Cached`, which the
  reference has no equivalent for. It advances the bar and never labels it, so a fully cached run
  shows nothing at all rather than flashing - see design.md D5.
- **The default upscale model is Tokyo, where the reference hardcodes Kyoto.** Not introduced here;
  it is the gap `lib/enhancements.ts` already declares, and this is the slice that makes it the model
  a run actually executes. It stands: the catalogue's order is documented as the order a chooser
  should offer models in, and closing the gap needs either a codename table in a front end or a
  `default` field on the catalogue - the two things that rule exists to avoid.
