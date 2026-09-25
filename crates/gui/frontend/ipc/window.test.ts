import { invoke } from "@tauri-apps/api/core";
import { describe, expect, it, type Mock, vi } from "vitest";
import { windowReady } from "./window";

// Mocked at the `invoke` boundary, so the name asserted below is the one that would actually go on
// the wire. This is the frontend half of a contract whose Rust half is `#[tauri::command] fn
// window_ready` in crates/gui/src/window.rs.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invoked = invoke as unknown as Mock;

describe("windowReady", () => {
    it("calls the command Rust registers, by name", () => {
        windowReady();

        expect(invoked).toHaveBeenCalledWith("window_ready");
    });

    // No arguments, and that is the contract rather than an omission: Tauri injects the window the
    // call arrived from, so which window is shown is decided by where the call came from and there is
    // nothing for this side to name.
    it("sends no arguments", () => {
        windowReady();

        expect(invoked).toHaveBeenCalledWith(expect.anything());
        expect(invoked.mock.calls[0]).toHaveLength(1);
    });
});
