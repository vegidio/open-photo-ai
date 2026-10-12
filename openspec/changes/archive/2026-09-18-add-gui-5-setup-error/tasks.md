## 1. The failure kind on the wire

- [x] 1.1 Add `Failure` to `crates/gui/src/setup.rs` - `Transfer`, `Unrecoverable`, `Other`, serialized
  camelCase - classifying `opai::InitError` per the table in `design.md`, with a `_` arm answering `Other` -
  verify with a unit test naming one `InitError` input per arm and asserting the `_` arm's direction is the
  one that gives no advice and keeps the button.
- [x] 1.2 Widen `SetupError::Initialize` with the `failure` field beside `message` - named `failure` because
  `kind` is already the variant tag - and set it where the command maps the error - verify with a unit test
  pinning the serialized shape `{"kind":"initialize","failure":"transfer","message":"..."}` and that
  `Display` is still `InitError`'s own sentence alone.

## 2. The frontend boundary

- [x] 2.1 Add the `Failure` and `SetupError` types to `frontend/ipc/setup.ts` and document the widened reject
  value on `initialize`'s doc comment - verify with a vitest asserting the shape a rejection is parsed from,
  in the style of the existing `setup.test.ts` cases.
- [x] 2.2 Add `failure?: SetupFailure` to `frontend/stores/setup.ts`, change `failed` to take the error, add
  the `retry` reducer that clears the failure and sets the status back to `running`, and add the selector for
  the row that stopped - verify with tests that a failure mid-transfer selects exactly that row, that a
  failure with no row working selects none, and that `retry` followed by a plan event replaces the rows
  without falling out of `running`.

## 3. The dialog

- [x] 3.1 Split slice 4's `SetupDialog` into the shell and the parts both of its states share: `surface.ts`
  for the card, `regions.tsx` for the header, the body and the bordered note, `ComponentRow.tsx` for one row
  and the `PRESENTATION` table, `OverallProgress.tsx` and `size.ts` - leaving `SetupProgress.tsx` holding only
  what an install in flight says. A shadcn `alert-dialog` primitive was generated for a separate failure
  dialog first and deleted with it; `design.md` records why the seam moved - verify screen 02 came through
  unchanged, by `SetupDialog.test.tsx` passing against the split without being rewritten.
- [x] 3.2 Add `frontend/features/setup/SetupFailure.tsx` as the dialog's failed state - screen 02b: the
  destructive header and its one sentence, the outcome list built from `ComponentRow` with `Error`,
  `Installed` and never-reached rows and no per-row tracks, the overall bar kept, the reason on one line in
  mono with `select-text` and its base-layer exception in `style.css`, click-to-copy with the `Copied!`
  tooltip, the known-cause note and the Quit / Try again footer - verify with tests over each row outcome,
  the reason drawn against the row it belongs to and on its own where no row stopped, and a click putting the
  whole message on the clipboard.
- [x] 3.3 Drive the two conditionals from `failure.kind`: the known-cause note shown only for `transfer`, Try
  again withheld for `unrecoverable` - verify with tests over all three kinds asserting which of the note and
  the button each one renders.
- [x] 3.4 Keep the dialog undismissable in both states, on the one `DialogContent` rather than once per state:
  `showCloseButton={false}` and handlers refusing Escape, the outside pointer-down and the outside
  interaction. Set the role from the state - `alertdialog` while it holds a failure, `dialog` otherwise, both
  written out because the prop is spread over the primitive's own - verify with a test in each state that the
  dialog survives an Escape key press and an outside click and offers no close button.
- [x] 3.5 Wire `App.tsx`: extract the mount effect's body into a `start` callback shared with Try again, keep
  the one `(running || failed)` condition rendering `SetupDialog`, and pass `start` as `onRetry` - verify with
  tests that a rejection draws the failed state and not the progress one, that Try again puts the progress
  state back and re-invokes the command, and that a retry resolving takes the dialog down.
- [x] 3.6 Add the new i18n keys under `setup.error.*` - title, the sentence about what the application needs,
  the `Error` row state, the known-cause note with its retry advice, Try again and the copy confirmation - to
  all thirteen locale files. A component that was never reached keeps `setup.state.queued`, which is the
  point of reusing the progress state's own words rather than adding a key for it - verify
  `i18n/parity.test.ts` passes.

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy` and `cargo test` for the workspace, and `pnpm lint`,
  `pnpm typecheck` and `pnpm test` in `crates/gui` - all clean.
- [x] 4.2 Launch on macOS with `runtime/` moved aside and `HTTPS_PROXY=http://127.0.0.1:9` and confirm the
  observable sequence against screen 02b: the setup dialog opens, the runtime's row stops, the failure dialog
  replaces it with that row marked `Error` and the reason beneath it, and the throttling advice is shown.
- [x] 4.3 From that dialog, confirm Try again puts the setup dialog back and reaches the same failure again,
  that relaunching without the proxy lets the retry succeed and leaves no dialog, and that Quit ends the
  process from the failure dialog.
