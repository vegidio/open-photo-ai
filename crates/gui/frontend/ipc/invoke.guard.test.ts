import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

// The window's twin of `every_registered_command_opens_its_span` in crates/gui/src/lib.rs, read from
// the sources as `faro.guard.test.ts` reads them: nothing else stops a module added later from calling
// `invoke` directly, and sending a request that names no trace.

const FRONTEND = join(import.meta.dirname, "..");

/** The call site every traced request goes through. */
const GATEWAY = "ipc/invoke.ts";

/** The two modules whose commands Rust leaves untraced, and which therefore call `invoke` themselves. */
const UNTRACED = ["ipc/analytics.ts", "ipc/log.ts"];

/** Every non-test `.ts` and `.tsx` under `dir`, outside `node_modules`. */
const sources = (dir: string): string[] =>
    readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
        const path = join(dir, entry.name);

        if (entry.isDirectory()) return entry.name === "node_modules" ? [] : sources(path);
        if (!/\.tsx?$/.test(entry.name) || /\.test\.tsx?$/.test(entry.name)) return [];

        return [path];
    });

/** Whether `source` takes `invoke` from Tauri's core, by name or through a namespace import. */
const importsInvoke = (source: string): boolean =>
    [...source.matchAll(/import\s+([^;]*?)\s+from\s+["']@tauri-apps\/api\/core["']/g)].some(([, clause = ""]) =>
        /\binvoke\b|\*\s+as\s+\w+/.test(clause.replace(/\btype\s+\w+/g, "")),
    );

describe("invoke", () => {
    it("has sources to read", () => {
        const paths = sources(FRONTEND).map((path) => relative(FRONTEND, path));

        expect(paths).toContain(GATEWAY);
        expect(paths).toEqual(expect.arrayContaining(UNTRACED));
    });

    it("recognises the imports it guards against", () => {
        expect(importsInvoke('import { invoke } from "@tauri-apps/api/core";')).toBe(true);
        expect(importsInvoke('import {\n    Channel,\n    invoke,\n} from "@tauri-apps/api/core";')).toBe(true);
        expect(importsInvoke("import * as core from '@tauri-apps/api/core';")).toBe(true);
        expect(importsInvoke('import { convertFileSrc } from "@tauri-apps/api/core";')).toBe(false);
        expect(importsInvoke('import { type InvokeArgs } from "@tauri-apps/api/core";')).toBe(false);
    });

    it("is imported only by ipc/invoke.ts and the two untraced modules, so every request is traced", () => {
        const offenders = sources(FRONTEND)
            .map((path) => relative(FRONTEND, path))
            .filter((path) => path !== GATEWAY && !UNTRACED.includes(path))
            .filter((path) => importsInvoke(readFileSync(join(FRONTEND, path), "utf8")));

        expect(offenders, "send through `call` from ipc/invoke.ts instead").toEqual([]);
    });
});
