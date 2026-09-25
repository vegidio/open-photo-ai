import type { Operation } from "@/ipc/enhance";
import type { Face } from "@/ipc/faces";

/**
 * A face's identity, as far as anything in this application is concerned: its four bounding-box
 * coordinates.
 *
 * **The same four numbers `Faces::write_cache_signature` folds**, in the same order and the same
 * spelling - `crates/opai/src/models/face.rs` gives the reasoning, and both halves of it apply here.
 * *Bounding box only*, because the box uniquely identifies a deterministically detected face and the
 * landmarks and the confidence move with it, distinguishing nothing it does not. *Nothing digested*,
 * because a key that is read back out of a set does not need to be short and a legible one is a
 * legible one in a debugger.
 *
 * **Stable across a re-detection**, which is what the whole by-value arrangement rests on: a
 * coordinate is quantized to a hundredth of a pixel as `Face::new` accepts it, so two detections of
 * one photograph at one framing produce the same numbers rather than numbers a sub-pixel apart. That
 * is `opai`'s own property and the reason it has it; this reads the result of it.
 *
 * The coordinates are in the **framed** photograph's pixels, which is what `ipc/faces.ts` answers and
 * what makes a framing change discard the choice: every face at a framing nobody has been at before
 * is at new numbers, so nothing matches and every one of them is decided again - by
 * {@link decidedSkips}, from its size. A framing *returned to* answers with the numbers it answered
 * the first time, so the keys match and both the choice and the decisions behind it are in force
 * again. See design.md D2.
 */
export const faceKey = (face: Face): string => {
    const { min, max } = face.bounding_box;

    return `${min.x},${min.y},${max.x},${max.y}`;
};

/**
 * The faces a run should actually restore: the ones found, less the ones the user skipped.
 *
 * **`faces` itself is handed back when nothing is skipped**, rather than a copy, and that is
 * load-bearing rather than a micro-optimisation: `useEnhancementRun` lists the faces among its
 * effect's dependencies, so a fresh array every render would cancel the run in flight and start
 * another on every render. See design.md D3 and D7.
 *
 * A skipped key that matches no face found simply filters nothing out - which is the whole of what a
 * framing change does to a choice, and is why neither this nor the store needs a rule for one.
 *
 * **The order of the survivors is preserved**, and that matters: it is folded into the run cache tag
 * of the face recovery they are handed to, so reordering them would ask `opai` for a different result
 * for the same selection.
 */
export const enabledFaces = (faces: Face[], skipped?: ReadonlySet<string>): Face[] =>
    skipped?.size ? faces.filter((face) => !skipped.has(faceKey(face))) : faces;

/**
 * The area, in the framed photograph's own pixels, above which a face is left out of a recovery
 * until the user asks for it.
 *
 * 512x512, and not a round number picked for looking like one: it is the tile the recovery models
 * actually run at. `crates/opai/src/models/face_recovery/restore.rs` exports its weights at a static
 * `[1, 3, 512, 512]` and both shipping variants align a face to a 512-pixel template. A face whose
 * box already covers more than that is *reduced* into the tile, restored, and pasted back enlarged -
 * so what the model can give back is bounded by what the reduction threw away, and on a face that
 * was already sharp it is a softening rather than a restoration. At or below it the model is working
 * at or above the detail the photograph holds, which is where a restoration is worth having.
 *
 * Compared as an **area** rather than edge against edge: a 1024x256 box and a 512x512 one cover the
 * same pixels and are the same amount of face to restore.
 *
 * A face exactly at the tile is chosen. It is neither reduced nor enlarged, so there is nothing to
 * defend it from - the argument above begins strictly above this number.
 */
export const RESTORED_FACE_AREA = 512 * 512;

/** The area a face's bounding box covers, in the framed photograph's pixels. */
const faceArea = (face: Face): number => {
    const { min, max } = face.bounding_box;

    return (max.x - min.x) * (max.y - min.y);
};

/**
 * The choice to hold for a photograph once `found` has been detected in it: what was already
 * skipped, plus every face among `found` that nothing has decided about yet and that is larger than
 * {@link RESTORED_FACE_AREA}.
 *
 * **The default among a set of faces is their size, not "all of them".** A big face in a group
 * photograph is the one the model can only soften, and the user had to turn it off by hand every
 * time. `decided` is the key of every face this photograph has **ever** been asked about, and it is
 * what makes the rule a *default* rather than a correction applied over and over: a face whose key is
 * in it has already been decided - by this rule or by the user since - so it is left exactly as it
 * is, and a re-detection at the framing in force changes nothing at all.
 *
 * **Ever, rather than at the last detection**, and that is the whole of what gives a framing returned
 * to back what the user chose there. Someone who turns a large face *on*, crops, and undoes the crop
 * is answered by the same face at the same coordinates; read against the last detection alone - which
 * is the *other* framing's faces by then - that face is undecided, and this rule skips it again,
 * silently undoing a choice they made by hand. `gui-faces` requires the opposite, that *"a face the
 * user has decided SHALL keep what the user made of it"*, and {@link withDecided} is what keeps it.
 *
 * A framing nobody has been at before is the other side of that same lookup. Every face moves, so
 * none of their keys is in `decided` and all of them are decided here - which is the answer the
 * repository owner chose: the default that applies when a framing change drops the old choice is this
 * rule, not "everything chosen". See design.md D2 and D12.
 *
 * **Only ever adds.** A face small enough to restore is not *un*-skipped by being found again, or a
 * user who deliberately skipped a small face would have it turned back on by a crop elsewhere in the
 * photograph - and a framing returned to would stop restoring what it restored before.
 *
 * Hands back `skipped` **itself** when nothing is added, `undefined` included. The store writes only
 * what this changes, and a set replaced with an equal one is what `useEnhancementRun` reads as a
 * choice having been applied - it would cancel the run in flight and start another. See D7.
 */
export const decidedSkips = (
    found: Face[],
    decided: ReadonlySet<string> | undefined,
    skipped: ReadonlySet<string> | undefined,
): ReadonlySet<string> | undefined => {
    const added = found
        .filter((face) => faceArea(face) > RESTORED_FACE_AREA)
        .map(faceKey)
        .filter((key) => !decided?.has(key) && !skipped?.has(key));

    return added.length ? new Set([...(skipped ?? []), ...added]) : skipped;
};

/**
 * Whether none of `faces` is small enough to be worth restoring: every one is larger than
 * {@link RESTORED_FACE_AREA}.
 *
 * What Autopilot drops a face-recovery suggestion on, and the same definition of "too large" that
 * {@link decidedSkips} skips a face on - so a suggestion survives exactly when the recovery it adds would
 * restore at least one face by default.
 *
 * **An empty array answers true.** The analysis said there were faces, so this only happens if the pixels
 * changed underneath it, and a face recovery with nothing to restore is exactly the row this keeps out.
 */
export const noFaceToRestore = (faces: Face[]): boolean => faces.every((face) => faceArea(face) > RESTORED_FACE_AREA);

/**
 * Every face a photograph has been asked about once `found` has been detected in it: the keys already
 * there, plus one for each face found.
 *
 * This is what {@link decidedSkips} reads, and it is a record of its own rather than something read
 * off the faces held for the photograph because that map holds **one** detection - the one at the
 * framing in force. A face read against it stops being decided the moment the user frames the
 * photograph another way, so a crop and an undo of that crop would put a large face the user turned
 * on back through the size rule.
 *
 * **It accumulates across framings and is never pruned while the photograph is open**, for that same
 * reason: a key dropped on the way out of a framing is a key missing on the way back into it. It
 * grows by one short string per face per framing visited, is bounded by the photograph, and is
 * released with the faces and the choice when it is closed.
 *
 * Hands back `decided` **itself** when every face found is already in it - which is what a
 * re-detection at the framing in force is - so the store writes nothing at all for one.
 */
export const withDecided = (
    found: Face[],
    decided: ReadonlySet<string> | undefined,
): ReadonlySet<string> | undefined => {
    const added = found.map(faceKey).filter((key) => !decided?.has(key));

    return added.length ? new Set([...(decided ?? []), ...added]) : decided;
};

/**
 * `operations` with `faces` put into every face recovery among them.
 *
 * **This is the one place the faces are resolved**, for the canvas's run and for an export alike: what differs
 * between them is which array this is handed, not who hands it. The stack holds what the user chose - a model at a
 * precision, and no faces - because the faces are a property of the pixels and change when the framing does; putting
 * them in the stack would mean every framing change rewrote every photograph's stack, and a stack entry would stop
 * being comparable to the one the add menu created.
 *
 * A stack carrying no face recovery comes back as a copy with every operation unchanged.
 */
export const withFaces = (operations: readonly Operation[], faces: Face[]): Operation[] =>
    operations.map((operation) => (operation.family === "face_recovery" ? { ...operation, faces } : operation));
