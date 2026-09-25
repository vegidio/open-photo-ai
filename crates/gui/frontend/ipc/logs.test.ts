import { invoke } from "@tauri-apps/api/core";
import { describe, expect, it, type Mock, vi } from "vitest";
import { type LogsError, revealLog } from "./logs";

// Mocked at the `invoke` boundary, so the name asserted below is the one that would actually go on
// the wire. This is the frontend half of a contract whose Rust half is `#[tauri::command] fn reveal_log`
// in crates/gui/src/logs.rs.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invoked = invoke as unknown as Mock;

describe("revealLog", () => {
    it("calls the command Rust registers, by name, with no arguments", () => {
        revealLog();

        expect(invoked).toHaveBeenCalledWith("reveal_log");
    });

    it("does not call the opener plugin directly", () => {
        revealLog();

        expect(invoked).toHaveBeenCalledWith("reveal_log");
        expect(invoked).toHaveBeenCalledTimes(1);
    });

    it("propagates a failure rather than swallowing it", async () => {
        const rejection: LogsError = {
            kind: "revealLog",
            message: "the log file could not be shown in the file manager: no such file or directory",
        };
        invoked.mockRejectedValueOnce(rejection);

        await expect(revealLog()).rejects.toEqual(rejection);
    });

    it("propagates an unresolvable log location as its own kind", async () => {
        // The shape is the half of the contract Rust's `the_error_crossing_ipc_carries_its_own_sentence`
        // pins from the other side - the two assert the same JSON.
        const rejection: LogsError = {
            kind: "logPath",
            message: "the log file's location could not be determined: no configuration directory",
        };
        invoked.mockRejectedValueOnce(rejection);

        await expect(revealLog()).rejects.toEqual(rejection);
    });
});
