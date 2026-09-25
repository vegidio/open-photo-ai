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

const light = (codename: string, precision: Operation["precision"], bias: number): Operation => ({
    family: "light_adjustment",
    codename,
    precision,
    parameters: { bias },
});

/** What the current image is set to have done to it. */
const stack = () => useEnhancementStore.getState().enhancements.get(HOLIDAY.path);

/** Puts one light adjustment on the current image, draws the list, and opens its options. */
const open = async (operation = light("paris", "fp32", 0.5)) => {
    useEnhancementStore.setState({ enhancements: new Map([[HOLIDAY.path, [operation]]]) });
    render(<EnhancementList />);

    // The row names the model off the catalogue, which is an `invoke`: waiting for it is waiting for
    // the panel's bounds to have arrived.
    await screen.findByText(/Paris|Lyon/);

    fireEvent.click(screen.getByRole("button", { name: /Light Adjustment/ }));
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

describe("a light adjustment's options", () => {
    it("offer both models and the bias as a percentage, over the published range", async () => {
        await open(light("paris", "fp32", 0.25));

        expect(screen.getByRole("radio", { name: "Paris HD" })).toBeChecked();
        expect(screen.getByRole("radio", { name: "Lyon SD" })).toBeInTheDocument();
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

        await waitFor(() => expect(stack()).toEqual([light("paris", "fp32", 0.25)]));
        expect(screen.getByRole("slider", { name: "Bias" })).toHaveAttribute("aria-valuenow", "25");
    });

    it("write nothing for a bare minus sign, and a negative bias once it has a digit", async () => {
        await open();
        const before = stack();

        type("-");
        expect(stack()).toBe(before);

        type("-30");

        await waitFor(() => expect(stack()).toEqual([light("paris", "fp32", -0.3)]));
    });

    it("bring a typed bias beyond the range down to the largest the model publishes", async () => {
        await open();

        type("150");

        await waitFor(() => expect(stack()).toEqual([light("paris", "fp32", 1)]));
    });

    it("keep the bias when the model changes", async () => {
        await open(light("paris", "fp32", -0.4));

        fireEvent.click(screen.getByRole("radio", { name: "Lyon SD" }));

        await waitFor(() => expect(stack()).toEqual([light("lyon", "fp16", -0.4)]));
    });
});
