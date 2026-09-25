import { invoke } from "@tauri-apps/api/core";
import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { Navbar } from "@/features/navbar/Navbar";
import { Preview } from "@/features/preview/Preview";
import "@/i18n";
import { forgetCatalogue } from "@/ipc/catalogue";
import { useSettingsStore } from "@/stores/settings";
import { useSetupStore } from "@/stores/setup";
import { CATALOGUE, PROVIDERS, render, resetFileStore, resetSettingsStore, resetSetupStore } from "@/test/support";

vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(),
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

// The canvas registers the webview's drag-drop listener on itself; what it does with a drop is
// `hooks/useDroppedImages.test.tsx`'s subject, and here it only has to be registrable.
vi.mock("@tauri-apps/api/webview", () => ({
    getCurrentWebview: () => ({ onDragDropEvent: () => Promise.resolve(() => {}) }),
}));

vi.mock("@/ipc/os", () => ({ isMacOs: () => false, isWindows: () => false }));

const invoked = invoke as unknown as Mock;

/** The window as the application mounts it: the chrome that owns the dialog, and the canvas behind it. */
const mount = async () => {
    await act(async () => {
        render(
            <>
                <Navbar />
                <Preview />
            </>,
        );
    });
};

const openSettings = async () => {
    await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    });
};

const choose = async (surface: string) => {
    await act(async () => {
        fireEvent.click(screen.getByRole("radio", { name: surface }));
    });
};

const leave = async (button: "Save" | "Cancel") => {
    await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: button }));
    });
};

const canvas = () => document.querySelector<HTMLElement>("[data-slot='preview-canvas']");

// Inside the canvas rather than anywhere in the document: the General page draws a particle field of
// its own, as the sample on its tile, whenever the dialog is open.
const field = () => canvas()?.querySelector<HTMLElement>("[data-slot='particles']");

const dotted = () => canvas()?.className.includes("radial-gradient") ?? false;

beforeEach(() => {
    localStorage.clear();
    useSettingsStore.setState(useSettingsStore.getInitialState(), true);
    resetSettingsStore();
    resetFileStore();
    resetSetupStore();
    forgetCatalogue();

    invoked.mockResolvedValue(CATALOGUE);
    useSetupStore.getState().succeeded(PROVIDERS);
});

/**
 * The applies-on-save rule, end to end: the real dialog, the real canvas, the real draft.
 *
 * Asserted here rather than by driving the store, because the thing that could break it is the wiring
 * between the two - the radio writes the dialog's draft and the canvas reads the settings store,
 * which holds only what Save has put in force. A change that pointed the canvas at the draft would
 * pass every unit test and fail exactly this.
 */
describe("choosing a canvas background", () => {
    it("leaves the canvas alone while the choice is still a draft", async () => {
        await mount();
        expect(field()).toBeInTheDocument();

        await openSettings();
        await choose("Dotted");

        // The radio has moved; the canvas has not, whatever the design's own mock does - see
        // `BackgroundCard`.
        expect(screen.getByRole("radio", { name: "Dotted" })).toBeChecked();
        expect(field()).toBeInTheDocument();
        expect(dotted()).toBe(false);
    });

    it("redraws the canvas on Save, without remounting it", async () => {
        await mount();
        const before = canvas();

        await openSettings();
        await choose("Dotted");
        await leave("Save");

        expect(dotted()).toBe(true);
        expect(field()).not.toBeInTheDocument();
        // The same element throughout: the surface is a class and a child of the canvas, not a
        // different canvas. A remount here would drop the drop listener and the zoom the user had.
        expect(canvas()).toBe(before);
    });

    it("draws the surface it had throughout when the choice is cancelled", async () => {
        await mount();

        // A saved choice first, so what Cancel has to preserve is not also the default - otherwise a
        // canvas that quietly reset to particles would pass.
        await openSettings();
        await choose("Dotted");
        await leave("Save");
        expect(dotted()).toBe(true);

        await openSettings();
        await choose("Particles");
        expect(dotted()).toBe(true);

        await leave("Cancel");

        expect(dotted()).toBe(true);
        expect(field()).not.toBeInTheDocument();

        // And the surface shows as the chosen one again next time the dialog is opened.
        await openSettings();
        expect(screen.getByRole("radio", { name: "Dotted" })).toBeChecked();
    });
});
