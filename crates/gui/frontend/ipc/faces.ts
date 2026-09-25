import type { CropInfo } from "./crop";
import { mintRun, type Processor } from "./enhance";
import { call } from "./invoke";

// Written a second time in `crates/gui/src/faces.rs`, as a `#[tauri::command]` function's name. A rename on
// one side alone is not a type error but an `invoke` that rejects at runtime: `faces.test.ts` pins this half,
// and `generate_handler![]` the Rust one by refusing to compile against a function that does not exist.
const DETECT_FACES_COMMAND = "detect_faces";

/**
 * A point in the pixel coordinate space of the photograph it was found in - **as the user has framed it**,
 * not as the file on disk is.
 */
export type Point = { x: number; y: number };

/** An axis-aligned rectangle, given by its top-left and bottom-right corners, as Rust's `Rect` names them. */
export type Rect = { min: Point; max: Point };

/**
 * One face found in a photograph: where it is, the five points that orient it, and how sure the detector
 * is that it is a face.
 */
export type Face = {
    // **`bounding_box` is snake_case on a camelCase wire, deliberately.** Every other shape crossing this
    // boundary is renamed by `serde` because this crate defines it; `Face` is `opai`'s own value, flattened
    // into the answer unchanged. The alternative is a mirror of thirteen coordinates whose only job is
    // casing, and a place for the landmark order to be transposed in silence. **Do not "fix" it.** A face
    // is never sent back: a run finds its own faces, and a choice names them by `key`.
    bounding_box: Rect;
    /**
     * Read positionally: left eye, right eye, nose, left mouth corner, right mouth corner. A **fixed-length
     * tuple** rather than an array, matching Rust's `[Point; Face::LANDMARKS]`: a consumer that reordered
     * them would misalign every face silently rather than fail.
     */
    landmarks: [Point, Point, Point, Point, Point];
    /** `0..1`. */
    confidence: number;
    /**
     * The face's identity for a choice among faces: `face_key` in `crates/gui/src/faces.rs`, the four
     * bounding-box coordinates. Rust writes it and reads it back, so this side never composes one.
     */
    key: string;
    /**
     * Whether a face recovery can restore this face: its box covers at most the 512x512 square the recovery
     * models restore at. `opai`'s `Face::restorable`, published by `crates/gui/src/faces.rs` beside the face
     * rather than restated here, so the default among a set of faces and the Autopilot suggestion are one rule.
     *
     * Like `key`, this crate's rather than `opai`'s: the face itself is `opai`'s, flattened in unchanged.
     */
    restorable: boolean;
};

/**
 * Why the `detect_faces` command rejected.
 *
 * `kind` is the `#[serde(tag = "kind")]` on Rust's `DetectError`. **Finding nobody is not among these**: a
 * photograph with no face in it answers an empty array, which is a finding rather than a failure.
 *
 * `message`, where a member carries one, is the core library's own sentence, composed in English and shown
 * untranslated - a diagnostic to be copied into a bug report, as `SetupError`'s is.
 */
export type DetectError =
    | { kind: "notReady" }
    | { kind: "unknownSource"; identity: string }
    | { kind: "unreadableSource"; identity: string; message: string }
    | { kind: "detect"; message: string };

/**
 * Find the faces in an open image, as the user has framed it.
 *
 * Answers **the run's name straight away**, beside the promise of the faces, exactly as {@link enhance}
 * does and for the same reason: the detection reports its progress on the same `enhance:progress` event,
 * and a caller has to be able to name the run it is drawing before the promise settles.
 *
 * **Every coordinate answered is in the framed photograph's pixels** - what the window draws on, and what
 * a face-recovery run is applied to. A framing that changes nothing costs nothing, and a framing returned
 * to is served from the store rather than detected again.
 *
 * **There is no stopping one.** A window that has moved on discards the answer by run name, as it already
 * discards a progress report about a run it abandoned. The detector sees one fixed square whatever the
 * photograph's size, so an abandoned detection is bounded - and its result is kept, so the next request
 * for that framing is served by it.
 *
 * The faces come back **in the order the detector found them**, and that order is load-bearing - see
 * `faces` on `Operation` in `ipc/enhance.ts`.
 *
 * Rejects with the serialized {@link DetectError}, typed as `unknown` because that is what an `invoke`
 * rejection is. A rejection is never an empty answer: an empty array is a photograph with nobody in it,
 * and a rejection is a question that was not answered.
 */
export const detectFaces = (source: string, processor: Processor, crop?: CropInfo) => {
    // Before the invoke, for the reason `enhance` gives beside its own.
    const run = mintRun();

    // `crop` is sent as `undefined` rather than omitted, as `enhance` sends it.
    return { run, done: call<Face[]>(DETECT_FACES_COMMAND, { run, source, processor, crop }) };
};
