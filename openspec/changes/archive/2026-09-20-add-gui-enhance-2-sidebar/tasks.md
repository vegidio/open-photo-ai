## 1. What an enhancement is on this side

- [x] 1.1 Give `lib/enhancements.ts` the three things a row, a menu entry and a chip need that it does
      not publish - an icon and a short name per family, and the list of families the wire can carry
      with the exhaustiveness check that makes a forgotten one a typecheck failure (D2) - and verify
      tests cover the runnable list matching the wire's own family union and every family naming an
      icon and a short name.
- [x] 1.2 Add the builder that turns a stored `<codename>_<precision>` default and an image's
      dimensions into an upscale `Operation`, splitting the model value through `ModelOption.codename`
      rather than on a separator, and choosing the scale by the image's megapixels (D3), and verify
      tests cover a codename containing an underscore, a stored selection naming a model the catalogue
      no longer publishes, and the three scale buckets including a record with no dimensions.

## 2. The stack

- [x] 2.1 Add the per-file `Map<path, Operation[]>` to `stores/enhancements.ts` with its add, replace
      and remove writers, the pipeline ordering on add, and the per-file drop on close and clear
      (D1) - rewriting the parked `immer` comment to record that the Maps landed without it - and
      verify tests cover the three writers, the ordering, two files keeping separate stacks, closing
      one file leaving another's alone, and `partialize` still persisting `autopilot` alone.

## 3. Adding and removing

- [x] 3.1 Make the sidebar's Add enhancement control live while an image is open and open the menu of
      runnable enhancements, with one already in the stack shown as unavailable (D2), and verify tests
      cover the control being disabled with no image, the menu offering Upscale, choosing it writing
      the stack at the user's default model, and it being unavailable once added.
- [x] 3.2 Draw the stack as a row per enhancement - icon, name, the info line beneath composed through
      the existing `enhancements.info*` keys, and the remove control revealed on hover and on focus -
      and verify tests cover the line reporting the model, the scale and the quality tier for a whole
      and a fractional scale, a model published at one precision only, the remove control being
      reachable by keyboard, and removing one of several leaving the rest in order.

## 4. The options panel

- [x] 4.1 Add the `shadcn` popover primitive with the fixes every generated primitive in this project
      needs - the `cn` import, the bogus `cn` package, arrow functions, named React imports, the
      hard-wired portal target - and build the options frame that a row opens, anchored to that row
      (D5), and verify the frame opens from a row, closes on dismissal, and leaves the stack unchanged
      when nothing inside it is changed.
- [x] 4.2 Add the model tray inside the frame - every model at every published precision, the info
      marker carrying that model's description on its first precision, the selection painted off
      `aria-checked` so the tooltip cannot clobber it (D5) - and verify tests cover the options coming
      from the catalogue, the selected model looking selected, choosing another writing the stack, and
      the marker appearing once per model.
- [x] 4.3 Add the scale control - the typed multiplier bounded by the selected model's own published
      range, the Max shortcut, and the 1x / 2x / 4x / Custom group (D4) - and verify tests cover a
      value inside the range, one above it being brought down, a trailing decimal separator being held
      while typing, Max reaching the published maximum, and Custom being shown as chosen for a value
      no shortcut names.

## 5. Running it

- [x] 5.1 Carry the operation's family on the progress report, on both sides of the boundary - the
      field on `RunProgress` in `enhance.rs` from `opai::Subject::family()`, and its type in
      `ipc/enhance.ts` (D7) - and verify a Rust test asserts the family is reported for a run and a
      cached operation alike, and `enhance.test.ts` pins the field's spelling.
- [x] 5.2 Add the run hook: start a run when the stack, the image or the processor changes, stop the
      one it replaces, stop one that nothing is waiting for, answer the source's own identity for an
      empty stack, keep only the reports naming its own run, and raise the existing notice for a
      failure but not for a stop (D6) - and verify tests cover each of those, including a stop arriving
      before its own run and a report from an abandoned run being discarded.
- [x] 5.3 Draw the result in the enhanced pane, fitted to the result's own dimensions and sharing the
      source's transform key (D8), and verify tests cover the enhanced pane pointing at the result
      while the original pane points at the source, both panes moving together at every zoom, and the
      pane falling back to the source before a run lands and after one is stopped.
- [x] 5.4 Draw the progress bar over the preview - the chain's figure, the enhancement's short name in
      the interface's language, the fetch's own percentage while a model is being installed - from the
      moment a run is asked for rather than from its first report, and verify tests cover a running
      report, an installing report, an asked-for run that has reported nothing, and no bar while
      nothing runs.

## 6. Language and checks

- [x] 6.1 Add whatever keys sections 3 to 5 needed that the ported catalogues do not already carry -
      the remove control's label above all - to every one of the thirteen locale files, and verify the
      i18n parity test passes.
- [x] 6.2 Run `cargo fmt --check`, `cargo clippy` and `cargo test`, and the frontend lint, typecheck
      and test scripts scoped to the files this slice touched, and verify all of them pass.
