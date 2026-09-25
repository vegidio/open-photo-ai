import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

// The frontend twin of `lib.rs`'s `every_registered_command_opens_its_span`, read from the sources the
// same way: nothing else stops a failure added later from going to the console alone, where no user
// can reach it.

const FRONTEND = join(import.meta.dirname, "..");

/** The one file that may write a failure to the console, because it also sends it on. */
const REPORTER = "lib/report.ts";

/** Every non-test `.ts` and `.tsx` under `dir`, outside `node_modules`. */
const sources = (dir: string): string[] =>
    readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
        const path = join(dir, entry.name);

        if (entry.isDirectory()) return entry.name === "node_modules" ? [] : sources(path);
        if (!/\.tsx?$/.test(entry.name) || /\.test\.tsx?$/.test(entry.name)) return [];

        return [path];
    });

describe("the console", () => {
    it("has sources to read", () => {
        expect(sources(FRONTEND).map((path) => relative(FRONTEND, path))).toContain(REPORTER);
    });

    it("is written to only by report, so every failure also reaches the log", () => {
        const offenders = sources(FRONTEND)
            .map((path) => relative(FRONTEND, path))
            .filter((path) => path !== REPORTER)
            .flatMap((path) =>
                readFileSync(join(FRONTEND, path), "utf8")
                    .split("\n")
                    .flatMap((line, index) =>
                        /console\.(error|warn)\(/.test(line) ? [`${path}:${index + 1}: ${line.trim()}`] : [],
                    ),
            );

        expect(offenders, "report the failure with `report` from lib/report.ts instead").toEqual([]);
    });
});
