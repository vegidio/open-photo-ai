## 1. The choice

- [x] 1.1 Add the deselection to `frontend/stores/faces.ts` (design.md D2): a second map
      `skipped: Map<string, ReadonlySet<string>>` keyed by identity beside the faces, a writer that
      replaces the whole map and the whole set on every write as this store's one standing rule requires,
      and a `useSkippedFaces(identity)` selector that hands back the same reference until a write happens -
      which is what D7 depends on. Release it in the existing `registerFileOwner` entry and in
      `forgetAllFaces`, and replace the store doc's *"There is no deselection here"* paragraph with what is
      now true. Verify tests cover a skipped set surviving a re-detection at the same framing, a close
      releasing both maps for that photograph and neither for the others, closing all releasing everything,
      and a write replacing rather than mutating both the map and the set.
- [x] 1.2 Add `frontend/lib/faces.ts` with `faceKey(face)` - the four bounding-box coordinates, which is
      the same thing `Faces::write_cache_signature` folds - and `enabledFaces(faces, skipped)` (D3).
      `enabledFaces` must return the `faces` array **itself** when nothing is skipped rather than a copy,
      which is load-bearing for D7. Verify tests cover the key being equal for two faces carrying the same
      bounding box and different for two that do not, the identity return when the skipped set is absent or
      empty, the order of the surviving faces being preserved (it is folded into the run cache tag), and a
      skipped key matching nothing leaving every face enabled.

- [x] 1.3 Decide what a face *starts* as from its area rather than starting every face chosen (D12).
      Add `RESTORED_FACE_AREA = 512 * 512`, `decidedSkips(found, decided, skipped)` and
      `withDecided(found, decided)` to `frontend/lib/faces.ts` - the number read off the recovery models'
      own tile rather than picked, the comparison strictly above it so a face exactly at the tile is
      chosen, and each set handed back **itself** when nothing is added. `decided` is a **third map** in
      `stores/faces.ts` holding the key of every face the photograph has ever been asked about, released
      with the other two: reading "already decided" off the faces map instead would hold one framing's
      worth, so a large face the user turned *on* is skipped again the moment they crop and undo the crop -
      which `gui-faces` forbids and which D2's framing-returned-to property depends on. Call both from
      `setFaces`, reading `decided` **before** this detection is folded into it, so the faces and the
      choice among them are written in **one** `set`: two writes would give `useEnhancementRun` one render
      holding the new faces beside the old choice, which is a chain started over every face and cancelled by
      the next render. Correct the three doc paragraphs that state the old default - the store's `skipped`
      field, the store header and `faceKey`. Verify tests cover a face above the tile being skipped and one
      at or below it chosen, the area being what is compared rather than either edge, a face already decided
      being left exactly as the user left it, a re-detection at the framing in force writing no new set at
      all, the faces at a framing nobody has been at being decided afresh, a face turned on **and** one
      skipped by hand surviving a framing left and returned to, a skipped face never being turned back on,
      the decided record surviving one close and releasing on another, and the faces and the choice landing
      in a single render.

## 2. The dialog

- [x] 2.1 Add `faces.hint` and `faces.apply_one`/`faces.apply_other` to **all thirteen** locale files
      (D10), translated rather than copied from English: the hint is the mockup's *"Click a box to include
      or skip a face."* and the apply reads *"Apply to {{enabled}} of {{count}} faces"*, pluralised on
      `count` as `faces_partial` already is. Verify `i18n/parity.test.ts` passes, and verify first that no
      key this slice needs already exists under another name - `faces.selectFaces`, `faces.toggleFace`,
      `enhancements.faceSelector.title/help/button` and `enhancements.faces_partial_one/other` are all
      already present in all thirteen and must be reused, not duplicated.
- [x] 2.2 Add `frontend/features/faces/FaceBoxes.tsx`: one absolutely positioned button per face, placed in
      **percent** of the framed photograph's dimensions from `framedDimensions(file, crop)` with the
      rendition's natural size as the fallback D5 describes, 3px border, yellow for chosen and grey for
      skipped, each carrying `faces.toggleFace` with its one-based number as its accessible name and each
      operable from the keyboard. No measurement, no resize listener and no `displayWidth`/`displayHeight`
      props - that whole apparatus is what D5 removes. Verify tests cover a box's four percentages against
      known coordinates and known framed dimensions, a framed photograph's percentages being taken against
      the crop rectangle rather than the file, the two appearances being told apart, every box being
      reachable by its accessible name, and operating one calling back with that face. Verify too that no
      box reaches the surfaces the boxes are kept off - the canvas in each of its three modes, the enhanced
      pane and the sidebar's miniature - which `FaceBoxes` having one production consumer makes true today
      and those catch a second being added.
- [x] 2.3 Add `frontend/features/faces/useFaceSelection.ts` (D4): a working `Set<string>` of face keys
      seeded from the store each time the dialog opens, a `toggle`, an `apply` that writes the store only
      when the set actually differs from what is committed, and nothing at all on dismissal. Verify tests
      cover re-seeding on each open (so a discarded edit does not reappear), toggling on and off returning
      to the committed set, apply writing once, apply writing nothing when the set is unchanged, and a
      dismissal after several toggles leaving the store as it was.
- [x] 2.4 Add `FACES_DIALOG_BOUND = 2048` to `frontend/lib/constants.ts` with the arithmetic D6 gives,
      beside the constants that already carry theirs, and add
      `frontend/features/faces/SelectFacesDialog.tsx` drawing screen 13b: the existing `DialogTitleBar`
      reading `faces.selectFaces`, the framed rendition from `renditionFor(file, FACES_DIALOG_BOUND, crop)`
      in a picture box sized by an exported `pictureFit(width, height)` that answers the picture's height
      and the dialog's width as `min()` expressions over `100vh`/`100vw` less an enumerated chrome budget
      (D6 - CSS, no viewport measurement). Both terms must be in **viewport** units: asking the picture to
      fill a dialog that takes its width from the picture is circular, and a browser resolves that circle by
      dropping the aspect. `FaceBoxes` over it, and the footer holding `faces.hint` and an Apply reading
      `faces.apply` with the chosen and total counts. Escape, the close box and a click outside all dismiss
      and all discard. Verify tests cover the dialog naming itself accessibly through the title bar, the
      rendition URL carrying both the bound and the framing, the Apply's label reporting one of two, Apply
      committing and each of the three dismissals writing nothing, the picture box asking for the framed
      aspect, and `pictureFit`'s own arithmetic pinned as strings - a browser and jsdom serialize a `min()`
      differently, so it is asserted where it is a return value rather than through a style attribute.

## 3. The way in

- [x] 3.1 Add screen 13's Faces block to `frontend/features/enhancements/FaceRecoveryOptions.tsx` beneath
      the existing `ModelTray`: a 1px rule, `enhancements.faceSelector.title`, the centred two-line
      `enhancements.faceSelector.help` through `<Trans>` so the `<br/>` stays where a translator can move it
      (D10), and a tertiary button reading `enhancements.faceSelector.button` with the chosen count, calling
      the new `onSelectFaces` prop. The button is unavailable for a photograph in which no faces were found.
      Replace the component doc's *"Choosing among the faces is the next slice"* paragraph with what it now
      draws. Verify tests cover the count on the button following the skipped set, the button being
      unavailable for a photograph with no faces and available for one with some, the help rendering both
      lines, operating the button calling back without writing anything, and the choice surviving the face
      recovery being removed and another added - which is what keeping it in the faces store rather than
      beside the stack buys.
- [x] 3.2 Change `frontend/features/enhancements/EnhancementRow.tsx` twice. Mount `SelectFacesDialog` as a
      **sibling** of the `<Popover>` on local open state, passed down to `FaceRecoveryOptions` as
      `onSelectFaces`, and close the popover when the dialog opens (D8 - inside the popover the dialog
      unmounts itself mid-open). And report the ratio in the info line through
      `t("enhancements.faces", { count, enabled, context })` exactly as the reference's `facesLabel` composes
      it, pluralised on the total (D9), leaving slice 1's rule that an unknown set reports `infoModel`
      untouched. Verify tests cover the row reading `2 Faces` with none skipped, `1/2 Faces` with one
      skipped, `0/2 Faces` with both, the model-and-quality line while the faces are unknown, the dialog
      opening from the panel and the panel closing as it does, and the dialog surviving that close.

## 4. The run

- [x] 4.1 Change `frontend/hooks/useEnhancementRun.ts` to run over the chosen faces (D7): subscribe to the
      skipped set for the current identity, list it among the effect's dependencies, and build the filtered
      array with `enabledFaces` **inside** the effect - computing it in the render body and listing it would
      re-run the chain on every render. `withFaces` itself is unchanged, and so is the detect-then-enhance
      ordering. Verify tests cover the operation carrying only the chosen faces, applying a choice starting
      exactly one new run, applying a choice that changes nothing starting none, a photograph with every
      face skipped still running the chain with an empty face list, and re-rendering without a store write
      starting no run at all.

- [x] 4.2 Make the detection and the chain it was run for **one** movement of the progress bar. Slice 1
      drew each run's own `chainFraction` as it arrived, so a face recovery filled the bar, emptied it and
      filled it again - two sweeps for one piece of work, which this slice's chooser makes constant by
      giving a user a reason to re-run a recovery repeatedly. Compose the single figure in
      `frontend/hooks/useEnhancementRun.ts`, where both runs are already known: the detection owns the head
      of the range and the chain the rest, a chain that needed no detection owns the whole of it, and the
      figure holds where the detection left it while the chain loads its model and says nothing. Hand it to
      `frontend/features/preview/ProgressBar.tsx` as a required `fraction` prop rather than letting it read
      `report.chainFraction` - the report describes whichever run it came from - and thread it through
      `frontend/features/preview/PreviewImage.tsx`. While in that effect, keep a detection that answers for
      a stack that no longer restores faces from disturbing the run in flight: record it for the framing it
      was found at and neither stop nor restart anything. Verify tests cover the figure advancing across the
      pair without returning to the start or completing twice, holding where the detection left it before
      the chain reports, a run over faces already known owning the whole range, and a detection landing for
      a stack that no longer needs faces leaving the running chain alone.

## 5. Checks

- [x] 5.1 Run the frontend lint, typecheck and test scripts scoped to the files this slice touched - biome
      scoped rather than repository-wide, because `pnpm check` reformats files this slice never opened. No
      Rust changes in this slice, so there is nothing for `cargo fmt`, `cargo clippy` or `cargo test` to
      cover; verify that by confirming `git status` names no `.rs` file. Verify `openspec validate --changes
      add-faces-2-select --strict` passes.
- [x] 5.2 Verify by hand on macOS, which is the only coverage the fit and the boxes get because jsdom has
      no layout. Open a photograph with two or more faces, add Face Recovery, and confirm the row reads `N
      Faces` once the detection lands, with any face wider and taller than about 512 pixels already
      skipped and everything smaller chosen (D12) - a portrait shot close enough that one face fills much
      of the frame is the case to check, alongside a group photograph where none of them does; open the options panel and confirm the Faces block draws as screen 13
      draws it with the count on the button; open the dialog and confirm the photograph fills it at its own
      proportions with a box squarely over each face - check a portrait, a landscape and a square
      photograph, which are the three ways a shrink-wrapped aspect box goes wrong; click a box and confirm
      it greys and the footer count drops; dismiss with Escape and confirm the row's count is unchanged and
      no run started; re-open, skip a face and Apply, and confirm exactly one run starts, the row reads
      `1/2 Faces`, and the enhanced pane restores only the face left on; re-open and confirm the skipped
      face is still grey; crop the photograph so both faces survive and confirm the re-detection decides
      every face by its size again rather than carrying the old choice over; turn the photograph to another angle and back, and
      confirm the skipped face is skipped again with no detector run (D2's whole point); on the portrait
      shot, turn the large face the size rule skipped **on**, crop the photograph and undo the crop, and
      confirm it is still on rather than skipped again - reading "already decided" off one framing's faces
      is what gets this wrong, so this is the step that catches it; confirm no box is
      ever drawn on the canvas, on the enhanced pane or on the sidebar's miniature; and close the photograph,
      re-open it, and confirm every face is decided by its size again.
