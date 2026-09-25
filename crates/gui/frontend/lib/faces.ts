import type { FaceChoice, Operation } from "@/ipc/enhance";
import type { Face } from "@/ipc/faces";

/**
 * A face's identity, as far as anything in this application is concerned: the key Rust publishes on it.
 *
 * **Rust's, not this side's.** `face_key` in `crates/gui/src/faces.rs` writes it - the four bounding-box
 * coordinates, `min.x,min.y,max.x,max.y` - and reads it back when a run asks which faces to restore, so the
 * two ends cannot spell one face two ways. It is stable across a re-detection, because a coordinate is
 * quantized to a hundredth of a pixel as `Face::new` accepts it.
 *
 * The coordinates are in the **framed** photograph's pixels, which is what makes a framing change leave a
 * choice behind: every face at a framing nobody has been at before is at new numbers, so no choice matches
 * and every one of them follows the default. A framing *returned to* answers with the numbers it answered
 * the first time, so the keys match and the choices made there are in force again. See design.md D2.
 */
export const faceKey = (face: Face): string => face.key;

/**
 * Whether a face recovery restores `face` under `choice`: the user's own word where they gave one, and
 * otherwise the default - `restorable`, `opai`'s `Face::restorable`.
 *
 * **The same rule Rust applies** (`FaceChoice::keeps` in `crates/gui/src/faces.rs`) when a run hands a
 * recovery its faces, so what the row counts and the picker shows is what the run restores. A face too
 * large to restore is left alone by default: a recovery reduces it into its 512-pixel square and pastes it
 * back enlarged, which on a face that was already sharp is a softening rather than a restoration.
 */
export const isKept = (face: Face, choice: FaceChoice | undefined): boolean => {
    const key = faceKey(face);

    return !choice?.skipped.includes(key) && (choice?.restored.includes(key) || face.restorable);
};

/**
 * The faces a run restores: the ones found, less the ones the choice leaves alone.
 *
 * **The order of the survivors is preserved**, and that matters: Rust keeps it too, and folds it into the
 * run cache tag of the face recovery they are handed to.
 */
export const enabledFaces = (faces: Face[], choice: FaceChoice | undefined): Face[] =>
    faces.filter((face) => isKept(face, choice));

/**
 * The choice to hold once the faces `shown` have been answered in the picker with `off` - the keys of the
 * ones the user left unchosen - over what `choice` already held.
 *
 * **Only the user's exceptions to the default are kept.** A face left as the default would have it is
 * recorded nowhere; one turned off although it is restorable goes in `skipped`, and one turned on although
 * it is not goes in `restored`. So the choice changes only when a person changes something, and a
 * detection - however often it runs - never writes one.
 *
 * **Faces not shown keep their word**: they belong to a framing the user is not at, and a choice made
 * there is what a framing returned to finds again.
 *
 * Hands back `choice` **itself** where nothing changed, which is what lets the picker commit only a real
 * change: a set replaced with an equal one is what `useEnhancementRun` reads as a choice just applied.
 */
export const choiceAfter = (
    shown: Face[],
    off: ReadonlySet<string>,
    choice: FaceChoice | undefined,
): FaceChoice | undefined => {
    const here = new Set(shown.map(faceKey));
    const skipped = (choice?.skipped ?? []).filter((key) => !here.has(key));
    const restored = (choice?.restored ?? []).filter((key) => !here.has(key));

    for (const face of shown) {
        const key = faceKey(face);
        const left = off.has(key);

        if (left && face.restorable) skipped.push(key);
        if (!left && !face.restorable) restored.push(key);
    }

    const same = (one: readonly string[], other: readonly string[] = []) =>
        one.length === other.length && one.every((key) => other.includes(key));

    if (same(skipped, choice?.skipped) && same(restored, choice?.restored)) return choice;

    return { skipped, restored };
};

/**
 * `operations` with `choice` put into every face recovery among them, for a run to hand the faces it finds.
 *
 * **This is the one place the choice is resolved**, for the canvas's run and for an export alike. The stack
 * holds what the user chose about the enhancement - a model at a precision - and the choice among faces is
 * a property of the pixels, kept beside the faces in `stores/faces.ts`; putting it in the stack would mean
 * every choice rewrote a stack entry, and a stack entry would stop being comparable to the one the add menu
 * created.
 *
 * **No faces cross.** Rust finds them inside the run it is asked for - one request, one progress stream,
 * one stop - and keeps the ones `choice` keeps. A recovery nobody has chosen for carries no choice, and
 * every face follows the default.
 *
 * A stack carrying no face recovery comes back as a copy with every operation unchanged.
 */
export const withChoice = (operations: readonly Operation[], choice: FaceChoice | undefined): Operation[] =>
    operations.map((operation) =>
        operation.family === "face_recovery" && choice ? { ...operation, faces: choice } : operation,
    );
