import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { appVersion, forgetOutdated, isOutdated } from "./app";

// Mocked at the `invoke` boundary, so the name asserted below is the one that would actually go on
// the wire. This is the frontend half of a contract whose Rust half is `#[tauri::command] fn version`
// in crates/gui/src/lib.rs.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invoked = invoke as unknown as Mock;

describe("appVersion", () => {
    it("calls the command Rust registers, by name", () => {
        appVersion();

        expect(invoked).toHaveBeenCalledWith("version");
    });

    it("returns what Rust answers, untouched", async () => {
        // No mapping and no defaulting: `opai::version()` is the single source of this string, and
        // normalising it here would be a second opinion about what the application's version is.
        invoked.mockResolvedValue("26.9.0");

        await expect(appVersion()).resolves.toBe("26.9.0");
    });
});

describe("isOutdated", () => {
    beforeEach(() => {
        invoked.mockReset();
        forgetOutdated();
    });

    it("calls the command Rust registers, by name", async () => {
        invoked.mockResolvedValue(true);

        await expect(isOutdated()).resolves.toBe(true);
        expect(invoked).toHaveBeenCalledWith("is_outdated");
    });

    it("asks once however often it is called", async () => {
        // Each ask is a request against GitHub's hourly sixty, and a remount must not spend another.
        invoked.mockResolvedValue(false);

        await isOutdated();
        await isOutdated();

        expect(invoked).toHaveBeenCalledTimes(1);
    });
});
