## Why

Slice 1 gave the window a door to `opai`'s analysis, and nothing walks through it: the Autopilot switch is on by
default, persisted and live, and a photograph opened with it on still arrives with an empty sidebar. This slice makes
the switch do what it says: a photograph the user looks at is analysed once, and what it calls for is added to its
list of enhancements.

## What Changes

This is **slice 2 of 2**, and it finishes Autopilot. It is frontend only.

- **The trigger.** A photograph is analysed when all of these hold: it is the current one, Autopilot is on, and it
  has never had a list of enhancements. The families asked for are every enhancement the application offers, minus
  the ones the Enhancements settings page switched off. Turning the switch on over such a photograph analyses it.
- **Once per photograph.** An analysis that answers writes a list, even an empty one, so the photograph is not
  analysed again until it is closed and reopened. This matches the reference. An analysis still in flight for a
  photograph the user moved off keeps going, and it applies to that photograph. Moving back to it shows its spinner
  again and does not start a second analysis.
- **The spinner row.** While the current photograph is being analysed, the sidebar's list is replaced by one row: a
  spinner and "Analysing image...". This is design screen 04, and it uses the `sidebar.analysing` key that already
  exists.
- **Applying the answer.** Every suggestion becomes the operation the add menu would build: the user's default model
  for that family, and the menu's starting bias or strength. The one exception is that an upscale takes the scale the
  analysis chose for the framed pixels. All of them are written in **one batch**, in pipeline order, skipping any
  family already in the list.
- **The 512x512 face rule.** When face recovery is among the suggestions, the window asks for the faces at the same
  framing. That request is served from the run store, because the analysis just made that detection. If every face
  found is larger than `RESTORED_FACE_AREA`, face recovery is dropped from the suggestions. Whatever the outcome, the
  faces are recorded, so the run that follows does not detect again. The rule applies only to Autopilot's suggestion,
  never to a face recovery the user adds by hand.
- **Failures.** An analysis that fails shows the existing `errors.autopilotFailed` toast and writes no list. The
  photograph is therefore analysed again the next time it becomes current. An incomplete analysis is applied, and
  its gap is only logged (by slice 1's backend).
- **Stopping.** Three events stop an analysis in flight, discard its answer and say nothing:
  - closing the photograph
  - switching Autopilot off, which stops every analysis in flight
  - reframing the photograph, which stops the analysis and asks again at the new framing
- **Spec moves.** Three existing requirements now contradict the application:
  - `gui-faces`: the rule that faces are never detected merely because a photograph is open or selected, and the
    rule that detection is always reported on the canvas indicator.
  - `gui-settings`: the sentence "The application does not run an Autopilot analysis yet".
  - `gui-enhance`: the scenario saying that a photograph opened again shows no enhancements.

  Each one is rewritten in this change.

**Deliberate divergences from the reference's behaviour:**

- **Stopped on close, on switch-off and on reframing.** The reference cancels nothing: an answer for a closed file
  is applied to a stack nobody can see, and an answer that lands after the switch was turned off is applied anyway.
  Here the user's last word wins, and the analysis stops holding the detector for an answer nobody will read.
- **Face recovery is dropped when every face is larger than 512x512.** The reference suggests face recovery for any
  face at all. The size rule `gui-faces` already applies to each face is what makes a restoration worthwhile. A
  suggestion whose every face that rule would skip is a row that restores nothing.
- **The upscale's scale is the analysis's own.** The reference sizes the upscale from the file's header. Here the
  analysis measured the framed pixels, which is what will actually be upscaled. On a photograph nobody has cropped,
  the two give the same scale.
- **Suggestions skip families already in the list.** The reference appends them without checking, so an enhancement
  added by hand during an analysis could appear twice. That is impossible here, because the store holds one entry
  per family.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `gui-autopilot`: gains the window's side, covering when an analysis is asked for, how it is shown, how its answer
  becomes a list of enhancements, the face-size rule, and what stops one.
- `gui-faces`: faces may now be found for a photograph because Autopilot is deciding whether to suggest face
  recovery, and a detection made for Autopilot is shown by the spinner row rather than the canvas indicator.
- `gui-settings`: the Autopilot family choice is now read by a running analysis, so the "not yet" sentence goes.
- `gui-enhance`: a photograph opened again starts with no enhancements from before, but Autopilot may suggest some
  afresh. Enhancements Autopilot adds carry what the add menu would give them.

## Impact

**Crates: `gui` only, and within it the frontend only.** No Rust is edited: `suggest`, `cancel_suggest` and
`detect_faces` already answer everything this slice needs. `opai` and `cli` are untouched. All of the logic here is
about a window: which photograph is current, what the sidebar shows, and when a question is withdrawn. The terminal
has no current photograph and no switch.

**`rust-sak`: nothing.** No infrastructure is involved.

**New crates: none. New npm packages: none.**

**Code touched:**

- New: `frontend/stores/autopilot.ts` (the analyses in flight, per photograph, registered as a file owner) and
  `frontend/hooks/useAutopilot.ts` (the trigger, and applying the answer).
- `frontend/lib/enhancements.ts` gains the conversion from suggestion to operation.
- `frontend/lib/faces.ts` gains the every-face-is-too-large check.
- `frontend/stores/enhancements.ts` gains a batch add.
- `features/sidebar/Sidebar.tsx` mounts the hook.
- `features/enhancements/EnhancementList.tsx` draws the spinner row.
- `stores/settings.ts` gets its "nothing reads it yet" comment corrected, and `stores/faces.ts` gets its "only writer"
  comment corrected.

**i18n: no new keys.** `sidebar.analysing` and `errors.autopilotFailed` are already in every locale.

**Task count: twelve.** The split is in design.md, under Slices.
