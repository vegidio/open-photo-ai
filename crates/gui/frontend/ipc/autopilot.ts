import type { Family } from "./catalogue";
import type { CropInfo } from "./crop";
import { mintRun, type Processor } from "./enhance";
import { call } from "./invoke";

// Written a second time in `crates/gui/src/autopilot.rs`, as `#[tauri::command]` function names. A rename on
// one side alone is not a type error but an `invoke` that rejects at runtime: `autopilot.test.ts` pins this
// half, and `generate_handler![]` the Rust one by refusing to compile against a function that does not exist.
const SUGGEST_COMMAND = "suggest";
const CANCEL_SUGGEST_COMMAND = "cancel_suggest";

/**
 * One enhancement an analysis concluded a photograph calls for, as Rust's `SuggestionWire` serializes it.
 *
 * **The family, and whatever the pixels determined about it - nothing else.** No suggestion names a model or
 * a precision: which model runs is the user's standing preference, which this side holds. Only an upscale
 * carries a value, the scale the photograph's size calls for.
 *
 * Never `detection`, which is not an enhancement anybody applies.
 */
export type Suggestion = { family: "upscale"; scale: number } | { family: Exclude<Family, "detection" | "upscale"> };

/**
 * Why the `suggest` command rejected.
 *
 * `kind` is the `#[serde(tag = "kind")]` on Rust's `SuggestError`. **An empty answer is not among these**,
 * and neither is an incomplete analysis: a photograph that needs nothing answers an empty array, and one
 * whose face detector could not be fetched answers what the other signals read, the reason going to the log.
 *
 * `stopped` is a question this side withdrew with {@link cancelSuggest}, or the application going down - a
 * caller stays silent for it, and reports the others.
 *
 * `message`, where a member carries one, is the core library's own sentence, composed in English and shown
 * untranslated - a diagnostic to be copied into a bug report, as `DetectError`'s is.
 */
export type SuggestError =
    | { kind: "notReady" }
    | { kind: "unknownSource"; identity: string }
    | { kind: "unreadableSource"; identity: string; message: string }
    | { kind: "stopped" }
    | { kind: "analyse"; message: string };

/**
 * Analyse an open image, as the user has framed it, for the enhancements it calls for among `families`.
 *
 * Answers **the run's name straight away**, beside the promise of the suggestions, exactly as
 * {@link detectFaces} does: a stop and its analysis cross the boundary independently, and a cleanup that runs
 * before the promise settles has to be able to name what it is stopping.
 *
 * `families` is the set the analysis may suggest from. **An empty set checks nothing** and answers nothing;
 * it does not mean "every family".
 *
 * The suggestions come back in the order the application applies enhancements in, whatever order `families`
 * was given in.
 *
 * Rejects with the serialized {@link SuggestError}, typed as `unknown` because that is what an `invoke`
 * rejection is. A rejection is never an empty answer.
 */
export const suggest = (source: string, processor: Processor, families: Family[], crop?: CropInfo) => {
    // Before the invoke, for the reason `enhance` gives beside its own.
    const run = mintRun();

    // `crop` is sent as `undefined` rather than omitted, as `enhance` sends it.
    return { run, done: call<Suggestion[]>(SUGGEST_COMMAND, { run, source, processor, families, crop }) };
};

/**
 * Stop an analysis this side asked for, by the name {@link suggest} answered.
 *
 * The analysis then rejects with `stopped`, even if it had already determined its answer. A stop that
 * reaches the backend before its own analysis still stops it, and one naming an analysis that has already
 * answered does nothing. Stopping one analysis touches no other, and no enhancement run.
 *
 * Resolves whatever happened; there is no outcome to act on.
 */
export const cancelSuggest = (run: string) => call<void>(CANCEL_SUGGEST_COMMAND, { run });
