import { invoke } from "@tauri-apps/api/core";
import { describe, expect, it, type Mock, vi } from "vitest";
import { type AnalyticsError, setAnalytics } from "./analytics";

// Mocked at the `invoke` boundary, so the name asserted below is the one that would actually go on
// the wire. This is the frontend half of a contract whose Rust half is `#[tauri::command] fn
// set_analytics` in crates/gui/src/analytics.rs.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invoked = invoke as unknown as Mock;

describe("setAnalytics", () => {
    it("calls the command Rust registers, by name, with the choice under the argument Rust reads", () => {
        setAnalytics(false);

        expect(invoked).toHaveBeenCalledWith("set_analytics", { enabled: false });
    });

    it("propagates a failure rather than swallowing it", async () => {
        // The shape Rust's `the_rejection_crosses_under_its_kind` pins from the other side.
        const rejection: AnalyticsError = {
            kind: "setAnalytics",
            message: "the analytics choice could not be saved: disk full",
        };
        invoked.mockRejectedValueOnce(rejection);

        await expect(setAnalytics(true)).rejects.toEqual(rejection);
    });
});
