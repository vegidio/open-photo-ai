import { invoke } from "@tauri-apps/api/core";
import { afterEach, describe, expect, it, type Mock, vi } from "vitest";
import { catalogue, type FamilyEntry, forgetCatalogue } from "./catalogue";

// Mocked at the `invoke` boundary, so the name asserted below is the one that would actually go on
// the wire. This is the frontend half of a contract whose Rust half is `#[tauri::command] fn
// catalogue` in crates/gui/src/catalogue.rs.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invoked = invoke as unknown as Mock;

/**
 * Two families, one with a bounded parameter and one with none.
 *
 * The second is what the empty list has to be able to say: colorization takes nothing, and it is
 * published rather than left out so "this enhancement has nothing to configure" is something the
 * window is told.
 */
const CATALOGUE: FamilyEntry[] = [
    {
        family: "sharpen",
        variants: [
            {
                codename: "sh_moscow",
                label: "Moscow",
                precisions: ["fp32", "fp16"],
                parameters: [{ name: "strength", kind: "range", min: 0, max: 100 }],
            },
        ],
    },
    {
        family: "colorization",
        variants: [{ codename: "cl_delhi", label: "Delhi", precisions: ["fp32", "fp16"], parameters: [] }],
    },
];

afterEach(() => {
    forgetCatalogue();
});

describe("catalogue", () => {
    it("calls the command Rust registers, by name, with no arguments", async () => {
        invoked.mockResolvedValueOnce(CATALOGUE);

        await catalogue();

        expect(invoked).toHaveBeenCalledWith("catalogue");
    });

    it("resolves with the whole of what Rust serialized, untouched", async () => {
        // Nothing is filtered, reordered or renamed on the way in - including `detection`, the family
        // no settings row draws, whose empty parameter list is a fact rather than an absence.
        invoked.mockResolvedValueOnce(CATALOGUE);

        await expect(catalogue()).resolves.toEqual(CATALOGUE);
    });

    it("issues no second invoke for a second read", async () => {
        invoked.mockResolvedValueOnce(CATALOGUE);

        const first = await catalogue();
        const second = await catalogue();

        expect(invoked).toHaveBeenCalledTimes(1);
        expect(second).toBe(first);
    });

    it("makes two reads during the first fetch one invoke", async () => {
        invoked.mockResolvedValueOnce(CATALOGUE);

        const [first, second] = await Promise.all([catalogue(), catalogue()]);

        expect(invoked).toHaveBeenCalledTimes(1);
        expect(second).toBe(first);
    });

    it("stays askable after a failed fetch", async () => {
        invoked.mockRejectedValueOnce(new Error("the webview went away"));
        invoked.mockResolvedValueOnce(CATALOGUE);

        await expect(catalogue()).rejects.toThrow("the webview went away");
        await expect(catalogue()).resolves.toEqual(CATALOGUE);

        expect(invoked).toHaveBeenCalledTimes(2);
    });
});
