import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { pauseFaro } from "@/lib/faro";
import { useSettingsStore } from "@/stores/settings";
import { mirrorAnalytics } from "./analytics";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@/lib/faro", () => ({ pauseFaro: vi.fn() }));

const paused = pauseFaro as unknown as Mock;

const invoked = invoke as unknown as Mock;

/** The choices sent to `set_analytics`, in order. */
const sent = () =>
    invoked.mock.calls
        .filter(([command]) => command === "set_analytics")
        .map(([, args]) => (args as { enabled: boolean }).enabled);

let unsubscribe: () => void = () => {};

beforeEach(() => {
    localStorage.clear();
    useSettingsStore.setState(useSettingsStore.getInitialState(), true);
    invoked.mockReset();
    invoked.mockResolvedValue(undefined);
    paused.mockReset();
});

afterEach(() => unsubscribe());

describe("mirrorAnalytics", () => {
    it("sends the stored choice once at boot", async () => {
        localStorage.setItem("settings-storage", JSON.stringify({ state: { analytics: false }, version: 0 }));
        await useSettingsStore.persist.rehydrate();

        unsubscribe = mirrorAnalytics();

        expect(sent()).toEqual([false]);
    });

    it("sends the choice again when a Save turns it off", () => {
        unsubscribe = mirrorAnalytics();

        useSettingsStore.getState().apply({ analytics: false });

        expect(sent()).toEqual([true, false]);
    });

    it("sends nothing for a Save that changes something else", () => {
        unsubscribe = mirrorAnalytics();

        useSettingsStore.getState().apply({ background: "dotted" });
        useSettingsStore.getState().apply({ analytics: true });

        expect(sent()).toEqual([true]);
    });

    it("pauses the window's sending before Rust is told of an opt-out", () => {
        const order: string[] = [];
        paused.mockImplementation(() => order.push("pause"));
        invoked.mockImplementation(async (_command: string, args: { enabled: boolean }) => {
            order.push(`set_analytics(${args.enabled})`);
        });
        unsubscribe = mirrorAnalytics();

        useSettingsStore.getState().apply({ analytics: false });

        expect(order).toEqual(["set_analytics(true)", "pause", "set_analytics(false)"]);
    });

    it("does not resume the window's sending on an opt-in", async () => {
        localStorage.setItem("settings-storage", JSON.stringify({ state: { analytics: false }, version: 0 }));
        await useSettingsStore.persist.rehydrate();
        unsubscribe = mirrorAnalytics();
        paused.mockClear();

        useSettingsStore.getState().apply({ analytics: true });

        expect(sent()).toEqual([false, true]);
        expect(paused).not.toHaveBeenCalled();
    });

    it("drops a rejection with a console message", async () => {
        const error = vi.spyOn(console, "error").mockImplementation(() => {});
        invoked.mockRejectedValueOnce({ kind: "setAnalytics", message: "disk full" });

        unsubscribe = mirrorAnalytics();
        await vi.waitFor(() => expect(error).toHaveBeenCalledOnce());

        error.mockRestore();
    });
});
