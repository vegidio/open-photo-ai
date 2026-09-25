import { afterEach, describe, expect, it } from "vitest";
import { isMacOs, isWindows } from "./os";

// The global the plugin's init script populates, stubbed directly rather than mocking
// `@tauri-apps/plugin-os`. That is the point of this file: it pins the wrapper against the real
// `platform()`, which reads this property and throws when it is absent - which is exactly what jsdom
// looks like, and why every component test mocks `@/ipc/os` instead of setting this up.
const stubPlatform = (value: string) => {
    window.__TAURI_OS_PLUGIN_INTERNALS__ = { platform: value } as Window["__TAURI_OS_PLUGIN_INTERNALS__"];
};

afterEach(() => {
    // `unstubGlobals` in vite.config.ts only undoes `vi.stubGlobal`; this is a plain property.
    Reflect.deleteProperty(window, "__TAURI_OS_PLUGIN_INTERNALS__");
});

describe("isMacOs", () => {
    it("is true on macOS", () => {
        stubPlatform("macos");

        expect(isMacOs()).toBe(true);
    });

    it("is false everywhere else", () => {
        stubPlatform("windows");

        expect(isMacOs()).toBe(false);
    });
});

describe("isWindows", () => {
    it("is true on Windows", () => {
        stubPlatform("windows");

        expect(isWindows()).toBe(true);
    });

    it("is false everywhere else", () => {
        stubPlatform("macos");

        expect(isWindows()).toBe(false);
    });
});
