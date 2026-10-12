## Context

See proposal.md - Why. What matters here is what slice 4 left standing and what it deliberately left unfinished.

`crates/gui/src/setup.rs` holds `Setup<T>`, the wire types and two commands. Three of its decisions are the
ground this slice is built on. **`initialize` is already retryable**: `Setup::ready` stores only a success, so a
call after a failure runs the initialization again - the module's own doc comment names "the failure dialog's
retry" as the third call site it was built for. **The command is already serialized**, so a Try again pressed
twice, or pressed while a retry is in flight, is one initialization. And **`SetupError` was shaped to be
widened**: one tagged struct variant carrying `InitError`'s rendered message, with the comment recording exactly
what this slice would want from it - *"which kind of failure it was, so that 'retrying in a few minutes usually
works' is said about a throttled download and not about a runtime that will not load"*.

On the frontend, `stores/setup.ts` holds the plan rows and a four-state status, and `failed()` today takes no
argument - the error is `console.error`'d in `App.tsx` and dropped. `App.tsx` renders `SetupDialog` while the
status is `running` **or** `failed`, with a comment saying that the failure dialog replacing it is the next
slice. That `|| status === "failed"` turns out to be the line this change *keeps* - see the decision below on
one dialog with two states - and what it holds up is no longer a progress dialog standing over a failure.

Two facts from `opai` shape the rest. `Plan::install` installs strictly sequentially - the runtime, then each
GPU library, each awaited with `?` - so **at most one dependency is in flight when a failure arrives**, which is
what makes "the component that stopped" derivable from the progress stream rather than needing to be reported.
And `InitError` is a `#[non_exhaustive]` enum whose own doc says its named variants exist because *"a front end
reports each differently, and the tests here assert on them"*.

The design authority for what this draws is screen 02b in the Claude Design project - re-read from the project
while planning this change, byte-identical to the copy slice 4 worked from. Its note: *"A destructive Alert
Dialog rather than the loading dialog wearing an error. Per-dependency outcome shows what is missing and how far
it got, the failure reason is stated plainly, the known cause is named with the advice that follows from it, and
Try again is the accent primary with Quit as the way out."* The project's `shadcn-mapping.md` adds one
requirement the screen itself cannot show: the failure reason is *"copyable for a bug report"*.

## Slices

The GUI is ported screen by screen against the Claude Design project the Stack section names.

| # | Slice | Delivers | Depends on | State |
|---|---|---|---|---|
| 1 | `add-gui-1-foundation` | Tailwind 4, shadcn/ui + the primitives the first screens need, the zinc-dark token set with `#009aff` primary, vendored Geist, lucide, all 13 i18n catalogues, Zustand installed | - | done |
| 2 | `add-gui-2-shell-empty` | Screen 01 - navbar, dotted preview canvas with the drag-and-drop invitation, drawer collapsed to its 48px header, sidebar. macOS overlay title bar and the platform IPC the inset needs | 1 | done |
| 3 | `add-gui-3-init-plan` | `InitOptions::on_plan` and the already-installed report, so the install list, its per-dependency sizes and each dependency's outcome are observable before and during a first run. `opai` only | - | done |
| 4 | `add-gui-4-setup-progress` | Screen 02 - `prepare_library_path` at the top of `main`, the `initialize` command, progress and plan events, the managed `Opai` handle, the setup dialog, automatic start, Quit | 2, 3 | done |
| 5 | `add-gui-5-setup-error` | Screen 02b - the failure dialog, per-dependency outcome, the failure kind on the wire, Try again and Quit | 4 | **CURRENT** |

**Slice 5 is this change**, and it closes the series slice 2 opened: with it, a launch either reaches the shell
or explains why it did not. The remaining 19 designed screens are not yet cut into slices; they are cut when
they are reached, and the table carries only what has been committed to.

**Crates.** `gui` only: `src/setup.rs`, and the frontend under `crates/gui/frontend/` - `ipc/setup.ts`,
`stores/setup.ts`, the setup feature split into the dialog and its two states, `App.tsx`, the i18n keys and
the tests. No new shadcn primitive: one was generated and then deleted, which the one-dialog decision below
records. `opai` and `cli` are untouched.

The one thing here that could be argued into `opai` is the classification of `InitError`, and proposal.md
records why it is not: the match reads a public taxonomy the library already documents as existing for front
ends to read, and what `gui` adds on top - which grouping gets a paragraph of advice, which gets a button - are
decisions about a dialog. A `cli` reporting the same failure prints the message and exits.

**rust-sak: nothing is used and nothing is missing.** This change renders a rejected promise. `fetch`, `crypto`
and `fs` are already at work inside `opai` on the path that produced the failure, and there is no infrastructure
here at all - no transfer, no hash, no extraction, no probe - so nothing for `rust-sak` to already provide and
no gap worth extending it with.

**New crates: none.** `serde` with `derive` and `thiserror` are already `gui` dependencies, and this adds one
enum beside types that already derive both. Nothing is added on the frontend either: the failed state is drawn
on the `Dialog` primitive slice 4 already used, and the `AlertDialog` this change briefly generated was shadcn
*source* over the `radix-ui` package slice 1 installed, so even that was never an npm dependency.

## Goals / Non-Goals

**Goals:**

- A launch that fails says what failed, which component it was working on, and why, in words a user can put in
  a bug report without retyping them.
- Advice the application gives is advice that is true of the failure it is about.
- A retry is the same initialization the application starts itself with, so there is one code path that can
  install this machine's components and not two.
- The thing that decides which of three shapes the dialog takes is one value, not three booleans that can
  disagree.

**Non-Goals:**

- **Reporting how far the stopped component got.** The design drew "Stopped at 9%" with "37 MB of 412 MB"
  beneath it; the row says `Error` instead, and no byte figures. See the decision below.
- **A structured reason.** The reason is `InitError`'s own sentence, rendered. Parsing an innermost cause out of
  a `thiserror` chain across an IPC boundary would be a second opinion about a string the library composed.
- **Retry policy.** No counter, no backoff, no automatic second attempt. Try again is a button a person presses,
  which is what makes "retrying in a few minutes usually works" a sentence rather than a schedule.
- **A cancel, or a way to continue without a component.** Unchanged from slice 4: nothing in the application
  runs without the components that did not arrive.
- **Reporting a failed *model* install.** `Dependency::Model` reaches the same reporter, but no model is
  installed during initialization - the first one lands when an operation runs, which is a later slice's screen
  with its own failure surface.

## Decisions

### `SetupError` gains a `Failure` beside its message, and the match is in `gui`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Failure {
    /// Getting the files did not work: a transfer that did not complete, or files that arrived and did not
    /// match what was published.
    Transfer,
    /// A second attempt produces the same answer.
    Unrecoverable,
    /// Everything else, and every variant added after this was written.
    Other,
}
```

carried as `SetupError::Initialize { failure: Failure, message: String }`. The field is named `failure` rather
than `kind` because `kind` is already the `#[serde(tag)]` discriminating `SetupError` itself, so the wire shape
is `{"kind":"initialize","failure":"transfer","message":"..."}`.

**Three kinds, because there are exactly three renderings**, and this project does not serialize an enum with no
reader:

| `Failure` | throttling note | Try again | `InitError` variants |
|---|---|---|---|
| `Transfer` | shown | offered | `Download`, `HashMismatch` |
| `Unrecoverable` | hidden | withheld | `InvalidName`, `InvalidApp`, `UnsupportedPlatform`, `UnknownProvider`, `RuntimeUnavailable` |
| `Other` | hidden | offered | everything else, and the `_` arm |

`HashMismatch` is a transfer failure rather than a verification one: an archive that arrived and hashed wrong is
most often a truncated response from the host that throttles them, which is the same story and the same advice.
`AlreadyRunning` is `Other` deliberately - `InitError`'s own message for it ends *"close it and try again"*, so
withholding the button would contradict the sentence beside it. `RuntimeUnavailable` is `Unrecoverable` because
its doc says so in as many words: *"failed to load earlier in this process and cannot be retried"*.

The `_` arm answers `Other`, which is the safe direction on both readers: an unknown failure gets no advice that
might be false, and gets the button, because offering a retry that fails again costs a second and refusing one
that would have worked costs the launch.

*Alternatives considered.* **A `bool` - retryable or not.** Rejected: it collapses the two readers into one, and
they genuinely differ - `RuntimeLoad` is worth retrying and is not a throttled download. **The full `InitError`
taxonomy on the wire**, one wire variant per library variant. Rejected: fourteen variants with three renderings
between them, and a front end that has to be edited every time `opai` adds one - which is the `#[non_exhaustive]`
trap this codebase already refuses for `Dependency`. **Classifying in `opai`.** Covered in proposal.md: one
reader, and the promotion is available the day there is a second.

### The component that stopped is derived, and it is derivable because installs are sequential

The failing row is the one whose state is `downloading` or `extracting` when the failure arrives - a selector
over the store's rows, not a field written into them.

This works because `Plan::install` awaits each dependency with `?` in `Plan::all`'s order, so there is never
more than one in flight; every row before the failing one has reported a terminal fraction of `1.0` and every
row after it has reported nothing. A derived answer cannot disagree with the rows it is derived from, which a
`failingRow: string` written beside them could.

It can be **empty**, and that is a real case rather than a defensive one: `AlreadyRunning`, `InvalidApp` and
`InvalidName` are raised before anything is probed, and `RuntimeLoad` and `RuntimeStart` happen after every
component is installed. The dialog then marks no row and shows the reason on its own, below the list, in the
same treatment the row would have given it. That is the `Failure that belongs to no component` scenario in the
spec, and it is the common case on a machine where everything is already on disk and only the runtime will not
start.

*Alternative considered.* **A failure report on the progress channel**, so `opai` names the dependency that
failed. Rejected, and slice 3's design already rejected it for this consumer: the stream says which dependency
was working, the error says what went wrong, and joining them in the front end needs no new library surface. It
would also be `opai` API with one reader, on the same argument as the classification.

### The failing row says `Error`, not how far it got

The design draws `Stopped at 9%` with `connection reset by peer · 37 MB of 412 MB` beneath it. The row says
`Error` instead, and carries no percentage and no byte figures. This is a deliberate departure from the design
file, agreed while planning this change.

The figures are not free. `ProgressRow` carries `name`, `state` and `fraction`, and `fraction` is the
*whole-install* position with `DOWNLOAD_SHARE` applied - a transfer at 37 of 412 MB reports `0.072`, so a row
reading `Stopped at 7%` under a reason line saying 37 of 412 would have the dialog contradicting itself in two
adjacent lines. Drawing the design's `9%` honestly means widening the wire with `Progress`'s `bytes` and
`total`, which are per-*phase* counters - so the same two fields read as expansion bytes when the failure lands
during an expansion, and the row would say "Stopped at 60% · 800 MB of 1.3 GB" about an archive whose transfer
finished.

`Error` costs none of that and loses nothing a user acts on. How far a failed download got is not a fact anyone
does anything with; what it was and why it stopped is, and both are on the row already.

### One dialog with two states, and a sub-component for each

`features/setup/SetupDialog.tsx` is the only dialog. It holds the card, the non-dismissible
behaviour, the focus handling and the role, and renders `SetupProgress` or `SetupFailure` according to
whether the store is holding a failure. The regions both states fill - the header, the overall bar and
list, the bordered aside - are `regions.tsx`, and one `ComponentRow` draws a row in either state.

**This reverses the decision this change first shipped**, which was a separate `SetupErrorDialog` on
shadcn's `AlertDialog`, argued from the design project's note: *"A destructive Alert Dialog rather
than the loading dialog wearing an error."* That reading was wrong about where the seam belongs. The
design file is describing what the failure *looks* like - destructive tint, its own icon, two actions
- and all of that is intact. What it is not describing is a second component, and building one meant
two of everything the two screens have in common. Every defect found afterwards came from exactly
that: the two disagreed about their own width (512pt against 560pt, a `twMerge` miss on one of two
copies of the same class string), about their rows' height (92pt against 52pt, then 54pt against
52pt), about what a component that was never reached is called (`Not reached` against `Queued`) and
drawn with (a dash against a clock), and about whether the overall bar exists at all. None of those
were possible once there was one card, one row and one table.

`role="alertdialog"` is a property of the *state*, not of the primitive: the failed state sets it, the
progress state sets `dialog`. That is the whole of what `AlertDialog` was wanted for - it is the role
that tells a screen reader this is an interruption requiring a response - and it costs one attribute
rather than a second primitive. Written out in both branches rather than left to Radix in one, because
the prop is spread over the primitive's own and `undefined` removes the role instead of falling back
to it. The generated `components/ui/alert-dialog.tsx` had no reader left and is deleted.

**Two invariants are now enforced by construction rather than by matching numbers in two files.**

- *The card.* One `SETUP_SURFACE` on one `DialogContent`, so the width cannot drift.
- *The row's second line.* `ComponentRow` gives it a fixed 14px slot with `justify-end`, and each
  state puts its own thing at the bottom of it - a 4px track, or a 12px line of text. Neither state
  names a gap, so the 40pt/54pt rows cannot disagree. The 14px is 2px more than the design's 12px,
  which is the one deliberate departure: the reason needs its natural line box rather than a squeezed
  one, and squeezing it trimmed the descenders.

The failed state keeps the overall bar rather than hiding or reserving it. It is the same question -
how much of what this machine needs is on disk - and the answer did not stop being true when the
attempt did; with the failing row's own track gone it is the only thing still saying how far the
attempt got. Its count is also why the failed header carries one sentence and not two: repeating
"2 of 4" a line above the bar that says it was the first thing the merge made visible. The header's
description reserves two lines' height (`min-h-[2lh]`) so that the one-sentence state does not stand
20pt shorter than the two-sentence one.

### The reason is one line, and the whole sentence on a click

The reason renders `SetupError.message` in the monospace face at 11px, in the destructive tint, **truncated to
one line** - and clicking it writes the whole of `message` to the clipboard, with a `Copied!` tooltip held open
for 1.6s.

**One line is arithmetic, not taste.** The failing row has to be the height the progress dialog's working row
is, or a failure resizes the list it is reported in. But the constraint runs both ways: the reason gets its
natural 12px line box with 2px of air above it, and the *working row's gap* is what was widened to match -
`gap-2.5` where the design draws 8px, a deliberate 2px. Both rows are 14px below the name, and measured on macOS
both are 54pt against 40pt for a row with nothing under its name.

Squeezing the reason into a 10px box to keep the working row at the design's 52pt was tried first and reverted:
it trimmed the tips of the descenders. The row height is the cheaper thing to move, and moving both rows keeps
the invariant that matters.

**Click-to-copy is what makes the truncation affordable**, and it is a better answer to the same problem than
selection was. A summary is the thing the Wails app does that this slice exists to stop doing - its
`dialogs.download.errorDetail` is one fixed string for every failure - so what reaches a bug report has to be
the library's own chain, URL and all. Selecting a truncated line yields the part that fitted; one click yields
all of it, out of a `message` the component holds in full whatever is drawn.

`select-text` stays on the line for what *is* drawn, and is why `style.css` puts its `user-select: none` in
`@layer base`: the comment there records that Tailwind's `select-text` lands in `utilities` and wins wherever it
is applied.

`navigator.clipboard` rather than `tauri-plugin-clipboard-manager`: the webview is a secure context in both
development and a release build, so the plugin would be a dependency, a permission entry and a second way to do
one line. It is read optionally rather than assumed, because a click handler that threw would take the failure
dialog down with it.

The confirmation is a **controlled** Radix tooltip - `open={copied}`, never opened by hover - because there is
nothing to say about the line until it has been copied, and a `Copied!` that appeared on hover would be a claim
made before anything happened. The timer is cleared on unmount, which is a real case: Try again takes the dialog
off screen while the tooltip is still up.

*Alternative considered.* **Wrapping to three lines with the rest reachable by selection**, which is what this
change first shipped. Rejected on measurement: it made the failing row 92pt against the working row's 52pt, so
the list grew by 40pt at the moment it reported a failure.

### The status stays the single source of whether the dialog is up

`App.tsx` becomes

```tsx
{(status === "running" || status === "failed") && <SetupDialog onRetry={start} />}
```

One condition covering both, which is what the one-dialog decision above costs here: the `|| status ===
"failed"` slice 4 wrote with a comment pointing at this slice is **kept**, and what changes is that it is no
longer a progress dialog wearing a failure - `SetupDialog` reads the failure itself and draws the state it
belongs to. No "is the error dialog open" flag is introduced beside the status, for the same reason slice 4
gave for not introducing one beside `running`: the status is the whole of the answer, and a second piece of
state holding the same fact is a second piece of state that can disagree.

The store gains `failure?: SetupFailure` - `{ kind: Failure; message: string }`, optional-property syntax and
`undefined` rather than `null`, per this project's frontend conventions - written by `failed(error)` and cleared
by `retry()`. `failed` taking the error is the change to an existing reducer; `retry()` is new and does two
things atomically: clears the failure and puts the status back to `running`, so the setup dialog comes back the
moment the button is pressed rather than after the retry's first report.

`apply()` is unchanged, including its `state.status === "idle"` guard. That guard now does a second job for
free: during a retry the status is already `running`, so the guard is not reached and a retry whose every
component reports `installed` does not fall back out of the dialog. Nothing about the retry needed a new status
value.

**The rows are not reset on retry.** The retry's own plan event replaces them wholesale - that is already what
`apply` does with a `plan` - and it arrives within milliseconds of the command starting. Until it does, the
setup dialog shows the previous attempt's rows, which is what the previous attempt actually found. Resetting
them to queued first would be a frame of the dialog claiming the two installed components were not.

### Try again calls `initialize`, and the call site is shared with the mount effect

The effect body in `App.tsx` becomes a `start` callback that both the mount effect and `onRetry` call:
`retry()`, then `initialize(apply).then(succeeded, onFailure)`.

One call site rather than two, because the two differ in nothing: the command is the same, the channel handler
is the same, and the resolution paths are the same. It is also what makes the serialization in Rust meaningful -
a Try again pressed while the previous attempt is somehow still running is caught by `Setup::ready`'s lock,
exactly as StrictMode's double mount is, rather than by a flag on this side that would have to be right about
both.

**A retry creates a new `Channel`**, because `initialize` creates one per call. The previous attempt's channel
is dropped with nothing referencing it; a late report on it lands in a handler updating a store the new plan is
about to replace, which is the same "no ordering is relied on" property slice 4 recorded.

### Which button is offered is a property of the failure, not of the dialog

`SetupFailure` reads `failure.kind` once and derives both conditionals from it. Withholding Try again is
what the `Unrecoverable` kind is *for*, so the dialog has no second rule about when to show it, and no state of
its own about whether it has been pressed - a second press while a retry is running cannot happen, because
pressing it once takes the dialog off screen.

## Risks / Trade-offs

- **The classification is a match on `#[non_exhaustive]` variants, and a variant added to `InitError` lands
  silently in `Other`.** → Accepted and deliberate: `Other` is the safe rendering for an unknown failure, and
  the alternative - a front end that fails to compile when the library gains an error case - is the coupling
  this codebase already refused for `Dependency`. The Rust test asserts one input per arm rather than
  exhaustively, which is all `#[non_exhaustive]` permits from outside the crate.
- **A component that is `Unrecoverable` on this launch might not be on the next one.** `UnsupportedPlatform` is
  permanent, but `UnknownProvider` would change if the options changed - and `gui` passes none, so it cannot
  arise from this front end today. → Accepted: the dialog's Quit is a restart away from a different answer, and
  a button that reproduces the same error instantly is worse than its absence.
- **The reason is untranslated.** It is `InitError`'s sentence, composed in English in `opai`, shown in a
  dialog whose every other string comes from thirteen catalogues. → Accepted, and it is the same judgement the
  component names already carry: it is a diagnostic to be copied into a bug report, and a translated one would
  be a diagnostic nobody reading the report can search for. The sentence *around* it - "Couldn't finish setting
  up", the advice, the buttons - is translated.
- **A failure with no working row shows the reason detached from the list.** On a machine with everything
  installed, screen 02b renders as a header, three `Installed` rows and a reason block, which is a layout the
  design file does not draw. → Accepted: it is the honest picture of that failure, and the alternative is
  attributing a runtime failure to whichever component happened to be last.
- **The failure dialog can appear without the setup dialog ever having been shown.** A failure raised before any
  transfer - `AlreadyRunning`, an unwritable configuration directory - moves the status straight from `idle` to
  `failed`, so the first thing a user sees is 02b over an empty shell, with an empty component list. → Accepted,
  and it is better than the alternative on both sides: showing 02 first would be a progress dialog for work that
  never started, and suppressing 02b would be a launch that fails in silence. The list being empty is correct;
  the reason is what that dialog is carrying.
- **Try again's return to the progress state is one synchronous render, and nothing enforces that.** → Not a
  race in practice: `onRetry` calls `retry()`, which sets the status in the same React event, so the failed
  state going away and the progress state arriving land in one render with no frame in between. It is worth
  naming because it is the kind of thing that becomes a flash if the retry ever grows an async step before the
  status change - and it is also why neither footer button closes the dialog itself: they are plain `Button`s,
  not Radix's `DialogClose`, so the status is the only thing that can take the dialog off screen.

**Platforms and execution providers.** Developed and verified on macOS. A real failure is forced there without
touching the machine or the network - `HTTPS_PROXY=http://127.0.0.1:9`, which `rust-sak`'s `fetch` honours, so
the runtime's transfer fails to connect in that one process and produces a genuine `InitError::Download`. That
exercises the `Transfer` rendering, the derived failing row, the reason block and a Try again that fails again
end to end. The `Unrecoverable` and `Other` renderings cannot be produced on this machine and are covered by
frontend tests driving the store with each kind, which is the same substitution slices 3 and 4 made for the
four-row plan. Windows and Linux are **unverified in a real launch**: the multi-row failure list, where the
failing row is a GPU library rather than the runtime, is untested end to end. **No execution provider is
involved**: this change runs no model, builds no session and loads no runtime - it draws the reason one could
not be loaded.
