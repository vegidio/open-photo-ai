## 1. Rust: the reveal seam

- [x] 1.1 Promote `reveal()` and its failure out of `src/logs.rs` into a module of its own (D2),
      leaving `LogPath` behind and having `reveal_log` call the promoted function with its own
      subject. No behaviour changes. Verify `cargo test -p gui` still passes with `logs.rs`'s four
      existing tests green and the three `cfg` arms moved verbatim.
- [x] 1.2 Give `Opened` the set of admitted paths alongside its identity map (D1), written by the
      same `admit` path both commands funnel through. Verify with unit tests that a file described by
      either route is known by path - including one carrying no identity - and that an arbitrary path
      on the machine is not.
- [x] 1.3 Add the `reveal_image` command taking a path, refusing one `Opened` does not know, and
      calling the promoted reveal; register it in `generate_handler!`. Verify with unit tests that an
      unknown path is refused without reaching the plugin, and that its error serializes with its own
      tag like `LogsError`'s does.
- [x] 1.4 Run `cargo fmt --check`, `cargo clippy` and `cargo test` across the workspace and verify all
      three are clean.

## 2. The IPC seam

- [x] 2.1 Add the `revealImage` wrapper to `frontend/ipc/images.ts`, propagating the rejection rather
      than swallowing it. Verify with a test pinning the command name and the path argument, matching
      how `ipc/logs.test.ts` pins `reveal_log`.

## 3. Closing an image

- [x] 3.1 Add `closeFile(path)` and `closeAll()` to the file store and remove the closed image's
      entry from the transform store (D7). Verify with tests covering each arm of the index fixup -
      closing below the current, closing the current with one following, closing the current when it
      is last, and closing the only image - plus removal from the selection and the transform map.

## 4. The menu

- [x] 4.1 Vendor the shadcn `dropdown-menu` component, then apply the four corrections every
      `shadcn add` run in this repo needs: the rewritten `cn` import, the bogus `cn` package, arrow
      functions, and named React imports. Verify `pnpm typecheck` and `pnpm lint` are clean and no
      new entry appeared in `package.json`.
- [x] 4.2 Build the shared `FileOptionsMenu` (D6): the three actions in order with the separator
      before the reveal, the platform label through i18next `context`, and the menu closed *before*
      any store write (R3). Verify with tests that all three actions fire for the file handed in,
      that the label follows the platform on all three, and that a failed reveal surfaces
      `errors.revealFileFailed`.

## 5. The two call sites

- [x] 5.1 Restructure `DrawerItem` (D4): lift the name bar out of the item's button, give that button
      an explicit `aria-label` of the file's name, and make the glyph a real trigger sized to the
      named glyph constant. Verify with tests that the item button is still reachable by the file's
      name (R5), that the trigger is a separate button, and that neither is nested in the other.
- [x] 5.2 Thread the drawer's ref down to `DrawerItem` and portal the thumbnail's menu into it (D3).
      Verify with a test that a pointerdown inside the open menu leaves the drawer unfolded while one
      on the canvas still folds it.
- [x] 5.3 Fold the drawer when the last open image is closed, by either route. Verify with tests
      covering close-all, closing the last remaining image singly, and closing one of several leaving
      the drawer unfolded.
- [x] 5.4 Add the navbar's overflow control beside the file name with the same menu anchored
      `side="bottom" align="center"` (D5), absent while no image is open. Verify with tests that the
      control appears only with an image open and that its menu acts on the current image.

## 6. Anchoring and verification

- [x] 6.1 Set both anchorings from the named glyph constant and the shared 16px gap (D5), confirming
      `alignOffset`'s sign against the first render. Verify by hand on macOS against screens 09 and
      09b: both corner relationships, the first and last thumbnail of a scrolled strip (R2), and
      closing both one image and all of them with a menu open (R3).
      Measured in the running window rather than eyeballed, by having the page report its own
      `getBoundingClientRect` values. Three things the measurement found and the code now carries:
      `alignOffset` cannot express the gap, because floating-ui clamps a cross-axis offset at the
      point the menu would stop overlapping its trigger - so the gap is a margin (D5); the navbar's
      offset has to be twice the gap, half of it spent between the trigger and the bar's own edge
      (D5); and Radix measured collisions against the viewport rather than the shell's clipping box,
      which cut the last thumbnail's menu in half (R2). Both corner relationships now measure
      `GAPx=16.0 GAPy=16.0` on the first and last thumbnail of a scrolled strip, and closing one
      image and all of them from an open menu both behave - the strip re-flows, or the window empties
      and the drawer folds, with no crash.

- [x] 6.2 Run the frontend's `typecheck`, `test` and Biome - scoped to the files this change touched,
      not repo-wide - and verify all are clean.
