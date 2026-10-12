## Why

Slice 4 drew screen 02 and stopped one step short of the launch that does not finish. Today a failed
initialization leaves the progress dialog exactly where it was - a row frozen part way through its bar, a footer
still promising that "the app will be ready to use when the downloads finish", and Quit as the only control. The
application has failed and says nothing about it; the one record of why is a log file nobody has been told to
look in.

That is a first launch, on a machine that has never run this application, ending in a dialog that lies. Screen
02b is the honest version of it: the failure is named, the component that stopped is pointed at, the reason is
on screen and copyable for a bug report, the known cause is stated where it applies, and Try again is offered
where retrying could actually work.

## What Changes

- **A failure state of the setup dialog - screen 02b.** One dialog with two states rather than two dialogs: the
  card, the overall bar, the component list and the fact that none of it can be dismissed are the same in both,
  and when initialization rejects what changes is the header, what each row says, whether the known cause is
  named and what the footer offers. The failure reads as the design's destructive alert - a warning icon,
  "Couldn't finish setting up", a destructive tint and two real actions - and it is drawn as a state because
  everything under the header is shared. See `design.md`; the seam moved during implementation.
- **Per-component outcome.** The component that was working when the failure arrived is marked as the one that
  stopped - a destructive icon, a tinted row, the state `Error` - and carries the failure's own sentence beneath
  it, in the monospace face, on one line, selectable, and copied whole by a click so it can go into a bug
  report. Every other row keeps what it last reported, in the words the progress state gave it: installed, or
  queued and never reached.
- **A failure kind on the wire.** `SetupError` gains the classification slice 4 deliberately shaped it to gain:
  `gui` matches `opai::InitError`'s public variants into three kinds the dialog draws differently - a transfer
  failure, a failure retrying cannot fix, and everything else. It is what decides whether the throttling advice
  is said and whether Try again is offered at all.
- **The known-cause note, conditionally.** "These files are hosted on GitHub, which throttles downloads... -
  retrying in a few minutes usually works" is shown for a transfer failure and withheld from a runtime that will
  not load, where it would be advice to wait for something that is not going to change.
- **Try again.** Re-invokes the existing `initialize` command, which slice 4 already built to be retryable -
  only success is remembered - and puts the dialog back in its progress state while the retry runs. It is
  **not** offered for a failure that cannot succeed on a second attempt: no runtime published for this
  platform, a rejected application name, a runtime that already failed to load in this process. Those leave
  Quit alone.
- **No new shadcn primitive**, and the ~10 new i18n keys screen 02b needs, added to all thirteen catalogues.
  An AlertDialog was generated for a failure dialog of its own and deleted when that became a state instead;
  the role it was wanted for is one attribute on the dialog already there.

**Not in this slice.** No new Tauri command - `initialize` and `quit` are both already there and both already do
what this needs. No change to `opai`. No cancel, no "report a bug" action, no retry counter or backoff: Try
again is a button a person presses.

## Capabilities

### New Capabilities

None. `gui-setup` already covers what the application does between launch and being ready to use, and slice 4
said in as many words that slice 5 extends it rather than adding a capability of its own.

### Modified Capabilities

- `gui-setup`: the one requirement slice 4 wrote about failure - *"A setup that fails leaves the dialog in place
  with what it had"* - is replaced. In its place: what the failure dialog states, how the component that stopped
  is identified and what it shows, when the known cause is named, when a retry is offered and what pressing it
  does, and that Quit remains the way out of all of it.

## Impact

**Crates touched: `gui` only.** `src/setup.rs` for the failure kind, and the frontend under
`crates/gui/frontend/` - the IPC types, the setup store, the setup feature split into the dialog and its two
states, the i18n keys and the tests. `opai` and `cli` are untouched.

Nothing here belongs in `opai`. The classification is a match over `InitError`'s own public variants, which
slice 1 documented as existing precisely because *"a front end reports each differently"* - reading that
taxonomy is not duplicating it. What `gui` adds on top is the decision about which of the three groupings gets
the GitHub sentence and which gets a Try again button, and both of those are decisions about a dialog: a `cli`
reporting the same failure would print the message and exit, with no button to withhold and no room for a
paragraph of advice. If a second front end ever wants the same judgement, the promotion is a method on
`InitError` in `opai` at that point; putting it there now would be public API with one reader and no second
opinion to reconcile.

**rust-sak: nothing applies.** This change transfers nothing, hashes nothing, extracts nothing and probes
nothing - it renders a rejected promise. `fetch`, `crypto` and `fs` are already in use inside `opai` on the path
that produced the failure being drawn, and there is no infrastructure here for `rust-sak` to provide and no gap
worth extending it with.

**New dependencies: none.** Rust: `serde` with `derive` and `thiserror` are already `gui` dependencies and the
change adds one enum beside types that already use them. Frontend: the failed state is drawn on the `Dialog`
primitive slice 4 already used, and the AlertDialog this change briefly generated was shadcn *source* over the
`radix-ui` package slice 1 installed, so no npm package is added either.

**Deliberate divergence from the Wails app's behaviour**, all of it following the design project, which the
project's Stack section makes the authority for these screens:

- **A state that is genuinely a failure, rather than the download dialog changing its copy.** The Wails dialog
  swaps two lines of text and enables its one button; the spinner keeps spinning behind an error, and the
  per-download bars sit at whatever fraction they reached with nothing saying which one is the problem. This is
  the same card, but the rows stop claiming work is under way, the one that stopped is marked, the header and
  the icon are destructive, and the dialog announces itself to a screen reader as an interruption requiring a
  response.
- **The failing component is named, and the reason is on screen.** The Wails app writes the reason to the
  browser console and shows the user `dialogs.download.errorDetail`, a fixed sentence that is the same whether
  the network went away, the archive hashed wrong or the platform has no runtime published.
- **Try again is withheld where it cannot work.** The Wails app offers it for every failure, including
  "no runtime is published for darwin/arm64", where pressing it produces the same error as fast as the machine
  can.
- **Quit.** The Wails dialog has no way out but killing the process, which slice 4 already diverged on and this
  slice keeps.

Behaviour that is **not** a divergence and is kept: Try again re-runs the same initialization rather than a
narrower repair, a retry that succeeds closes the dialog and leaves the application usable, and the failure is
written to the log where it happened.

**Parity gaps this slice knowingly leaves open:** the TensorRT first-run dialog the Wails app shows after a
*successful* initialization, which is a later screen; and the providers the initialization decided, which
nothing reads until there is a Settings screen.

**Task estimate: 13**, inside the 15-task limit. This change is slice 5 of a series; `design.md` carries the
full slice list.
