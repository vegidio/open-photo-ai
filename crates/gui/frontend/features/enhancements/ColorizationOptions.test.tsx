import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import { forgetCatalogue } from "@/ipc/catalogue";
import type { Operation } from "@/ipc/enhance";
import { useEnhancementStore } from "@/stores/enhancements";
import { CATALOGUE, HOLIDAY, openFiles, render, resetFileStore } from "@/test/support";
import { EnhancementList } from "./EnhancementList";

vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(() => Promise.resolve(CATALOGUE)),
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

const colorization = (codename: string, precision: Operation["precision"]): Operation => ({
    family: "colorization",
    codename,
    precision,
});

/** What the current image is set to have done to it. */
const stack = () => useEnhancementStore.getState().enhancements.get(HOLIDAY.path);

const panel = () => document.querySelector("[data-slot='enhancement-options']");

/** Puts one colorization on the current image, draws the list, and opens its options. */
const open = async (operation = colorization("delhi", "fp32")) => {
    useEnhancementStore.setState({ enhancements: new Map([[HOLIDAY.path, [operation]]]) });
    render(<EnhancementList />);

    // The row names the model off the catalogue, which is an `invoke`: waiting for it is waiting for
    // the tray's options to have arrived.
    await screen.findByText(/Delhi|Mumbai|Jaipur/);

    fireEvent.click(screen.getByRole("button", { name: /Colorization/ }));
    await waitFor(() => expect(panel()).not.toBeNull());
};

/** The tray's own options. */
const models = () => [
    ...(document.querySelector("[data-slot='model-tray']")?.querySelectorAll('[role="radio"]') ?? []),
];

/** Counts every write to the stacks from here on, which is what asks for a new run. */
const countWrites = () => {
    let writes = 0;
    useEnhancementStore.subscribe((state, previous) => {
        if (state.enhancements !== previous.enhancements) writes += 1;
    });

    return () => writes;
};

beforeEach(() => {
    localStorage.clear();
    forgetCatalogue();
    resetFileStore();
    useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
    openFiles(HOLIDAY);
});

describe("a colorization's options", () => {
    it("offer the three models at each quality, and nothing else", async () => {
        await open();

        expect(models()).toHaveLength(6);
        for (const model of ["Delhi", "Mumbai", "Jaipur"]) {
            expect(screen.getByRole("radio", { name: `${model} HD` })).toBeInTheDocument();
            expect(screen.getByRole("radio", { name: `${model} SD` })).toBeInTheDocument();
        }

        // Colorization takes no parameter, so there is no amount, no scale shortcut and nothing to type.
        const options = panel() as HTMLElement;
        expect(options.querySelector("[role='slider']")).toBeNull();
        expect(options.querySelector("input, [role='textbox']")).toBeNull();
        expect(options.querySelector("[data-slot='separator']")).toBeNull();
        expect(options.querySelectorAll("[role='radio']")).toHaveLength(models().length);
    });

    it("show the model in use as chosen", async () => {
        await open(colorization("mumbai", "fp16"));

        expect(screen.getByRole("radio", { name: "Mumbai SD" })).toBeChecked();
        expect(screen.getByRole("radio", { name: "Delhi HD" })).not.toBeChecked();
    });

    it("write the chosen model once, at the quality chosen", async () => {
        await open();
        const writes = countWrites();

        fireEvent.click(screen.getByRole("radio", { name: "Jaipur HD" }));

        await waitFor(() => expect(stack()).toEqual([colorization("jaipur", "fp32")]));
        expect(writes()).toBe(1);
    });

    it("change nothing by being opened and dismissed", async () => {
        await open();
        const before = stack();
        const writes = countWrites();

        fireEvent.click(screen.getByRole("button", { name: "Close" }));

        await waitFor(() => expect(panel()).toBeNull());
        expect(stack()).toBe(before);
        expect(writes()).toBe(0);
    });
});
