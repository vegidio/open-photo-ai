import { useEffect, useRef, useState } from "react";
import { type ExportName, type ExportNaming, exportNames, type FormatChoice } from "@/ipc/export";
import { report } from "@/lib/report";

/**
 * What the queue's files are exported as, by path, and the format choice that answer was given for.
 *
 * `format` is there because the names lag the settings by a round trip: what each file is written in depends on the
 * format choice alone (the queue is fixed while the dialog is open), so a reader of `byPath`'s formats checks it
 * against the current choice before trusting them.
 */
export type ExportNames = { byPath: ReadonlyMap<string, ExportName>; format: FormatChoice };

/**
 * What each queued file is exported as under `naming`, by path - `undefined` until Rust's first answer arrives.
 *
 * **Asked again on every change** to the queue or `naming` - the prefix, the suffix, the folder or the format: one
 * `export_names` call covers the whole queue, so typing is one round trip per keystroke and no debounce makes the
 * names lag. A change is a new `naming` object, so a caller keeps one that has not changed (`useShallow` does).
 *
 * **Only the latest question's answer is kept.** An earlier one that arrives after it - fast typing - is dropped, so a
 * row never goes back to an older name. Until the latest arrives the last names stay, so typing does not flicker.
 */
export const useExportNames = (queue: readonly string[], naming: ExportNaming): ExportNames | undefined => {
    const [names, setNames] = useState<ExportNames>();
    const asked = useRef(0);

    useEffect(() => {
        asked.current += 1;
        const question = asked.current;

        exportNames(queue, naming).then(
            (answer) => {
                if (question !== asked.current) return;

                setNames({ byPath: new Map(answer.map((name) => [name.path, name])), format: naming.format });
            },
            // The rows keep the names they had; the export itself asks Rust again, and fails on its own if it must.
            (error: unknown) => report("naming the queued exports failed", error),
        );
    }, [queue, naming]);

    return names;
};
