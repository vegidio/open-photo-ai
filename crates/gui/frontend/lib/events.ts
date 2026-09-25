import type { Language } from "@/i18n/languages";
import type { Family } from "@/ipc/catalogue";
import type { FormatChoice } from "@/lib/export";
import { extensionOf } from "@/lib/paths";
import type { PreviewMode } from "@/stores/preview";
import type { Background, Processor, SettingsData } from "@/stores/settings";

// The window's usage events, and all it may say in them. Each event answers a question about how the
// application is used that only the window can answer; a fact the backend already records, or a
// render rather than a decision, is not an event. See the change `add-observability-7-faro`'s design
// (D6, D7) for how each of the Go application's 29 events was judged.
//
// **A closed vocabulary.** Every value is a count, a duration, a yes-or-no, one of the spellings this
// window already uses, or `formatList`'s output. Never a name, a path, a folder, an image's dimensions,
// a user's text or an error message: `events.test.ts` fails on a parameter typed as a bare `string`.

/** A file type summary: sorted, distinct, lowercase extensions joined by commas. Made only by {@link formatList}. */
export type FormatList = string & { readonly __formatList: unique symbol };

/** A preference `settings_saved` may name. Never the Analytics choice: an opt-out that is reported is not one. */
export type SettingKey = Exclude<keyof SettingsData, "analytics">;

/** Each event, and the one set of parameters it carries. */
export type EventParams = {
    /** Once per load, when setup succeeds: what people run with, counting every user rather than those who changed something. */
    app_ready: { processor: Processor; language: Language; background: Background; autopilot: boolean };
    /** Images admitted: how many, how they came in, and their types. */
    files_added: { count: number; source: "browse" | "empty" | "drop"; formats: FormatList };
    /** A drop that held files this application cannot open. */
    files_refused: { count: number; formats: FormatList };
    /** An enhancement added to an image, by the user or by Autopilot. */
    enhancement_added: { family: Family; source: "manual" | "autopilot" };
    /** An enhancement the user removed. */
    enhancement_removed: { family: Family };
    /** An analysis the user started, completed: how many it suggested, zero included. Never an export's own. */
    autopilot_run: { count: number };
    /** The comparison view changed. */
    preview_mode_changed: { mode: PreviewMode };
    /** A framing that changes something was applied. */
    crop_applied: { rotated: boolean; flipped: boolean; has_ratio: boolean };
    /** A batch export started. */
    export_started: { file_count: number; format: FormatChoice; processor: Processor };
    /** A batch export ended, run to the end or not. Milliseconds unbucketed: a bucket would throw away the p95. */
    export_finished: { file_count: number; exported: number; completed: boolean; duration_ms: number };
    /** Settings saved with something changed: which preferences, sorted. Their values are in the next `app_ready`. */
    settings_saved: { changed: readonly SettingKey[] };
    /** The TensorRT question answered. */
    tensorrt_prompt_answered: { accepted: boolean };
    /** The update notice followed to the releases page. */
    update_opened: Record<string, never>;
};

export type EventName = keyof EventParams;

/**
 * Summarises the file types in one import as a sorted, de-duplicated, comma-joined list of lowercase
 * extensions.
 *
 * Sorted and de-duplicated so that "jpg then raw" and "raw then jpg" are the same value on a dashboard,
 * and so the cardinality stays bounded by the set of supported formats rather than by the order files
 * happen to arrive in. Extensions only - never a file name, which would be user content.
 */
export const formatList = (extensions: readonly string[]): FormatList =>
    [...new Set(extensions.map((ext) => ext.replace(/^\./, "").toLowerCase()).filter(Boolean))]
        .sort()
        .join(",") as FormatList;

/** {@link formatList} over the files at `paths`, reading each one's extension and nothing else of it. */
export const formatsOf = (paths: readonly string[]): FormatList => formatList(paths.map(extensionOf));
