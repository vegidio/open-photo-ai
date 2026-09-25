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

const denoise = (codename: string, precision: Operation["precision"], strength: number): Operation => ({
    family: "denoise",
    codename,
    precision,
    parameters: { strength },
});

/** What the current image is set to have done to it. */
const stack = () => useEnhancementStore.getState().enhancements.get(HOLIDAY.path);

/** Puts one denoise on the current image, draws the list, and opens its options. */
const open = async (operation = denoise("stockholm", "fp32", 1)) => {
    useEnhancementStore.setState({ enhancements: new Map([[HOLIDAY.path, [operation]]]) });
    render(<EnhancementList />);

    // The row names the model off the catalogue, which is an `invoke`: waiting for it is waiting for
    // the panel's bounds to have arrived.
    await screen.findByText(/Stockholm|Gothenburg|Malmö/);

    fireEvent.click(screen.getByRole("button", { name: /Denoise/ }));
    await waitFor(() => expect(document.querySelector("[data-slot='enhancement-options']")).not.toBeNull());
};

const strength = () => screen.getByRole("textbox", { name: "Strength" }) as HTMLInputElement;

const type = (text: string) => fireEvent.change(strength(), { target: { value: text } });

beforeEach(() => {
    localStorage.clear();
    forgetCatalogue();
    resetFileStore();
    useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
    openFiles(HOLIDAY);
});

describe("a denoise's options", () => {
    it("offer the three models and the strength, labelled Strength, as a percentage over the published range", async () => {
        await open(denoise("stockholm", "fp32", 1.25));

        expect(screen.getByRole("radio", { name: "Stockholm HD" })).toBeChecked();
        expect(screen.getByRole("radio", { name: "Gothenburg HD" })).toBeInTheDocument();
        expect(screen.getByRole("radio", { name: "Malmö HD" })).toBeInTheDocument();
        expect(strength().value).toBe("125");

        const slider = screen.getByRole("slider", { name: "Strength" });
        expect(slider).toHaveAttribute("aria-valuemin", "0");
        expect(slider).toHaveAttribute("aria-valuemax", "300");
    });

    it("change nothing by being opened", async () => {
        await open();
        const before = stack();

        fireEvent.click(screen.getByRole("button", { name: "Close" }));

        await waitFor(() => expect(document.querySelector("[data-slot='enhancement-options']")).toBeNull());
        expect(stack()).toBe(before);
    });

    it("write a typed percentage as the library's unit strength", async () => {
        await open();

        type("150");

        await waitFor(() => expect(stack()).toEqual([denoise("stockholm", "fp32", 1.5)]));
        expect(screen.getByRole("slider", { name: "Strength" })).toHaveAttribute("aria-valuenow", "150");
    });

    it("write nothing for an emptied field, and restore the strength when it is left", async () => {
        await open();
        const before = stack();

        type("");
        expect(stack()).toBe(before);

        fireEvent.blur(strength());

        expect(stack()).toBe(before);
        expect(strength().value).toBe("100");
    });

    it("bring a typed strength beyond the range down to the largest the model publishes", async () => {
        await open();

        type("400");

        await waitFor(() => expect(stack()).toEqual([denoise("stockholm", "fp32", 3)]));
    });

    it("bring a typed negative strength up to none", async () => {
        await open();

        type("-20");

        await waitFor(() => expect(stack()).toEqual([denoise("stockholm", "fp32", 0)]));
    });

    it("write a dragged strength once, when the slider is released", async () => {
        await open();

        // jsdom lays nothing out, so the track is given a 200px rectangle for the one element Radix
        // measures: 0..300 across it, so 100px is 150%, 50px is 75% and 150px is 225%.
        const root = document.querySelector("[data-slot='slider']") as HTMLElement;
        vi.spyOn(root, "getBoundingClientRect").mockReturnValue(
            DOMRect.fromRect({ x: 0, y: 0, width: 200, height: 16 }),
        );

        const writes: unknown[] = [];
        const unsubscribe = useEnhancementStore.subscribe((state) => writes.push(state.enhancements));

        fireEvent.pointerDown(root, { pointerId: 1, clientX: 100 });
        fireEvent.pointerMove(root, { pointerId: 1, clientX: 50 });
        expect(strength().value).toBe("75");
        fireEvent.pointerMove(root, { pointerId: 1, clientX: 150 });
        fireEvent.pointerUp(root, { pointerId: 1, clientX: 150 });
        unsubscribe();

        expect(writes).toHaveLength(1);
        expect(stack()).toEqual([denoise("stockholm", "fp32", 2.25)]);
    });

    it("keep the strength when the model changes", async () => {
        await open(denoise("stockholm", "fp32", 1.5));

        fireEvent.click(screen.getByRole("radio", { name: "Malmö HD" }));

        await waitFor(() => expect(stack()).toEqual([denoise("malmo", "fp32", 1.5)]));
    });
});
