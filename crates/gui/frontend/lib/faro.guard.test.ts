import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

// The twin of `report.guard.test.ts`, read from the sources the same way: nothing else stops a module
// added later from reaching Faro on its own, past the Analytics choice and the build's collector that
// `startFaro` checks. OpenTelemetry's SDK is under the same rule: Faro's tracing is built on it, and a
// tracer reached directly would send past the same two checks.

const FRONTEND = join(import.meta.dirname, "..");

/** The one file that may import Faro's SDK, or OpenTelemetry's. */
const GATEWAY = "lib/faro.ts";

/** Every non-test `.ts` and `.tsx` under `dir`, outside `node_modules`. */
const sources = (dir: string): string[] =>
    readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
        const path = join(dir, entry.name);

        if (entry.isDirectory()) return entry.name === "node_modules" ? [] : sources(path);
        if (!/\.tsx?$/.test(entry.name) || /\.test\.tsx?$/.test(entry.name)) return [];

        return [path];
    });

describe("Faro", () => {
    it("has sources to read", () => {
        expect(sources(FRONTEND).map((path) => relative(FRONTEND, path))).toContain(GATEWAY);
    });

    it("is imported only by lib/faro.ts, with OpenTelemetry, so nothing sends past the Analytics choice", () => {
        const offenders = sources(FRONTEND)
            .map((path) => relative(FRONTEND, path))
            .filter((path) => path !== GATEWAY)
            .flatMap((path) =>
                readFileSync(join(FRONTEND, path), "utf8")
                    .split("\n")
                    .flatMap((line, index) =>
                        /["'](@grafana\/faro-|@opentelemetry\/)/.test(line)
                            ? [`${path}:${index + 1}: ${line.trim()}`]
                            : [],
                    ),
            );

        expect(offenders, "send through `track`, `sendError` or `traced` from lib/faro.ts instead").toEqual([]);
    });
});
