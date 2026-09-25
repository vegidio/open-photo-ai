import { invoke } from "@tauri-apps/api/core";
import { describe, expect, it, type Mock, vi } from "vitest";
import { log } from "./log";

// Mocked at the `invoke` boundary, so the name asserted below is the one that would actually go on
// the wire. This is the frontend half of a contract whose Rust half is `#[tauri::command] fn log` in
// crates/gui/src/frontend.rs.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invoked = invoke as unknown as Mock;

describe("log", () => {
    it("calls the command Rust registers, by name, with the record under the argument Rust reads", () => {
        log({ level: "error", message: "m", error: "e", stack: "s", componentStack: "c" });

        // camelCase, as Rust's `WindowRecord` deserializes it.
        expect(invoked).toHaveBeenCalledWith("log", {
            record: { level: "error", message: "m", error: "e", stack: "s", componentStack: "c" },
        });
    });

    it("propagates a failure rather than swallowing it", async () => {
        invoked.mockRejectedValueOnce("invalid args `record` for command `log`");

        await expect(log({ level: "warn", message: "m" })).rejects.toBe("invalid args `record` for command `log`");
    });
});
