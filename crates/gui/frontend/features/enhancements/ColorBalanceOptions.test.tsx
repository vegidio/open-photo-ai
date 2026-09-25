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

const balance = (codename: string, precision: Operation["precision"], bias: number): Operation => ({
    family: "color_balance",
    codename,
    precision,
    parameters: { bias },
});

/** What the current image is set to have done to it. */
const stack = () => useEnhancementStore.getState().enhancements.get(HOLIDAY.path);

/** Puts one colour balance on the current image, draws the list, and opens its options. */
const open = async (operation = balance("rio", "fp32", 0.5)) => {
    useEnhancementStore.setState({ enhancements: new Map([[HOLIDAY.path, [operation]]]) });
    render(<EnhancementList />);

    // The row names the model off the catalogue, which is an `invoke`: waiting for it is waiting for
    // the panel's bounds to have arrived.
    await screen.findByText(/Rio|São Paulo/);

    fireEvent.click(screen.getByRole("button", { name: /Color Balance/ }));
    await waitFor(() => expect(document.querySelector("[data-slot='enhancement-options']")).not.toBeNull());
};

const bias = () => screen.getByRole("textbox", { name: "Bias" }) as HTMLInputElement;

const type = (text: string) => fireEvent.change(bias(), { target: { value: text } });

beforeEach(() => {
    localStorage.clear();
    forgetCatalogue();
    resetFileStore();
    useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
    openFiles(HOLIDAY);
});

describe("a colour balance's options", () => {
    it("offer both models and the bias, labelled Bias, as a percentage over the published range", async () => {
        await open(balance("rio", "fp32", 0.25));

        expect(screen.getByRole("radio", { name: "Rio HD" })).toBeChecked();
        expect(screen.getByRole("radio", { name: "São Paulo HD" })).toBeInTheDocument();
        expect(bias().value).toBe("25");

        const slider = screen.getByRole("slider", { name: "Bias" });
        expect(slider).toHaveAttribute("aria-valuemin", "-100");
        expect(slider).toHaveAttribute("aria-valuemax", "100");
    });

    it("change nothing by being opened", async () => {
        await open();
        const before = stack();

        fireEvent.click(screen.getByRole("button", { name: "Close" }));

        await waitFor(() => expect(document.querySelector("[data-slot='enhancement-options']")).toBeNull());
        expect(stack()).toBe(before);
    });

    it("write a typed percentage as the library's unit bias", async () => {
        await open();

        type("25");

        await waitFor(() => expect(stack()).toEqual([balance("rio", "fp32", 0.25)]));
        expect(screen.getByRole("slider", { name: "Bias" })).toHaveAttribute("aria-valuenow", "25");
    });

    it("write nothing for a bare minus sign, and a negative bias once it has a digit", async () => {
        await open();
        const before = stack();

        type("-");
        expect(stack()).toBe(before);

        type("-30");

        await waitFor(() => expect(stack()).toEqual([balance("rio", "fp32", -0.3)]));
    });

    it("write nothing for an emptied field, and restore the bias when it is left", async () => {
        await open();
        const before = stack();

        type("");
        expect(stack()).toBe(before);

        fireEvent.blur(bias());

        expect(stack()).toBe(before);
        expect(bias().value).toBe("50");
    });

    it("bring a typed bias beyond the range down to the largest the model publishes", async () => {
        await open();

        type("150");

        await waitFor(() => expect(stack()).toEqual([balance("rio", "fp32", 1)]));
    });

    it("write a dragged bias once, when the slider is released", async () => {
        await open(balance("rio", "fp32", 0));

        // jsdom lays nothing out, so the track is given a 200px rectangle for the one element Radix
        // measures: -100..100 across it, so 100px is 0%, 60px is -40% and 130px is 30%.
        const root = document.querySelector("[data-slot='slider']") as HTMLElement;
        vi.spyOn(root, "getBoundingClientRect").mockReturnValue(
            DOMRect.fromRect({ x: 0, y: 0, width: 200, height: 16 }),
        );

        const writes: unknown[] = [];
        const unsubscribe = useEnhancementStore.subscribe((state) => writes.push(state.enhancements));

        fireEvent.pointerDown(root, { pointerId: 1, clientX: 100 });
        fireEvent.pointerMove(root, { pointerId: 1, clientX: 60 });
        fireEvent.pointerMove(root, { pointerId: 1, clientX: 130 });
        fireEvent.pointerUp(root, { pointerId: 1, clientX: 130 });
        unsubscribe();

        expect(writes).toHaveLength(1);
        expect(stack()).toEqual([balance("rio", "fp32", 0.3)]);
    });

    it("keep the bias when the model changes", async () => {
        await open(balance("rio", "fp32", 0.3));

        fireEvent.click(screen.getByRole("radio", { name: "São Paulo HD" }));

        await waitFor(() => expect(stack()).toEqual([balance("saopaulo", "fp32", 0.3)]));
    });
});
