import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import { forgetCatalogue } from "@/ipc/catalogue";
import { track } from "@/lib/faro";
import { useEnhancementStore } from "@/stores/enhancements";
import { useSettingsStore } from "@/stores/settings";
import { CATALOGUE, HOLIDAY, openFiles, render, resetFileStore } from "@/test/support";
import { AddEnhancement } from "./AddEnhancement";
import { EnhancementList } from "./EnhancementList";

// Mocked at `lib/faro.ts`'s own boundary: what `track` does with an event is pinned by `faro.test.ts`.
vi.mock("@/lib/faro", () => ({
    track: vi.fn(),
    sendError: vi.fn(),
    pauseFaro: vi.fn(),
    // Untraced, as before Faro starts: the request goes out exactly as `invoke` alone would send it.
    traced: (_name: string, send: () => Promise<unknown>) => send(),
}));

// The catalogue is an `invoke`, and what the menu adds is built from it: the model, its precision
// and the tier behind it all come from what the library publishes.
vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(() => Promise.resolve(CATALOGUE)),
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

/** The control, and the menu it opens. */
const button = () => screen.getByRole("button", { name: "Add enhancement" });
const upscale = () => screen.findByRole("menuitem", { name: "Upscale" });

/**
 * Opens the menu.
 *
 * Through the keyboard, as `FileOptionsMenu.test.tsx` opens its own: jsdom dispatches no
 * `pointerdown` from a `click`, and Radix's trigger opens on the pointer rather than on the click.
 */
const open = () => fireEvent.keyDown(button(), { key: "Enter" });

/** What the current image is set to have done to it. */
const stack = () => useEnhancementStore.getState().enhancements.get(HOLIDAY.path);

describe("adding an enhancement", () => {
    beforeEach(() => {
        localStorage.clear();
        forgetCatalogue();
        resetFileStore();
        useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
        useSettingsStore.setState({ models: {}, autopilotExcluded: [] });
        vi.mocked(track).mockClear();
    });

    it("is unavailable with no image open", () => {
        render(<AddEnhancement />);

        expect(button()).toBeDisabled();
    });

    it("goes live as soon as an image is open", () => {
        openFiles(HOLIDAY);
        render(<AddEnhancement />);

        expect(button()).toBeEnabled();
    });

    it("offers every enhancement this application presents, in pipeline order", async () => {
        openFiles(HOLIDAY);
        render(<AddEnhancement />);

        open();

        // All seven: every family the wire can carry is in `ENHANCEMENTS`, which the typecheck holds.
        expect(await upscale()).toBeInTheDocument();

        // Denoise first, then face recovery, colorization, light adjustment, colour balance and sharpen in
        // that order and all before upscale, and the menu says so: it draws `ENHANCEMENTS` itself, which is
        // what keeps the menu and the chain in one order.
        const offered = screen.getAllByRole("menuitem");
        expect(offered.map((item) => item.textContent)).toEqual([
            "Denoise",
            "Face Recovery",
            "Colorization",
            "Light Adjustment",
            "Color Balance",
            "Sharpen",
            "Upscale",
        ]);
    });

    it("adds it at the user's default model, at a scale suited to the photograph", async () => {
        openFiles(HOLIDAY);
        useSettingsStore.setState({ models: { upscale: "kyoto_fp16" } });

        render(<AddEnhancement />);
        open();
        fireEvent.click(await upscale());

        // HOLIDAY is 3000x2000 - six megapixels, so the largest bucket and no enlargement by
        // default. The model and its precision are the stored selection, split through the
        // catalogue's own codename.
        await waitFor(() =>
            expect(stack()).toEqual([{ family: "upscale", codename: "kyoto", precision: "fp16", scale: 1 }]),
        );
        expect(track).toHaveBeenCalledExactlyOnceWith("enhancement_added", { family: "upscale", source: "manual" });
    });

    it("adds a light adjustment at the user's default model and a bias of 50%, with its options closed", async () => {
        openFiles(HOLIDAY);
        render(
            <>
                <AddEnhancement />
                <EnhancementList />
            </>,
        );

        open();
        fireEvent.click(await screen.findByRole("menuitem", { name: "Light Adjustment" }));

        await waitFor(() =>
            expect(stack()).toEqual([{ family: "light_adjustment", codename: "paris", precision: "fp32", bias: 0.5 }]),
        );
        // The user opens the options from the row; adding an enhancement does not open them for any family.
        expect(await screen.findByText("Paris, 50%, HD")).toBeInTheDocument();
        expect(document.querySelector("[data-slot='enhancement-options']")).toBeNull();
    });

    it("adds a colour balance at the user's default model and a bias of 50%, with its options closed", async () => {
        openFiles(HOLIDAY);
        render(
            <>
                <AddEnhancement />
                <EnhancementList />
            </>,
        );

        open();
        fireEvent.click(await screen.findByRole("menuitem", { name: "Color Balance" }));

        await waitFor(() =>
            expect(stack()).toEqual([{ family: "color_balance", codename: "rio", precision: "fp32", bias: 0.5 }]),
        );
        expect(await screen.findByText("Rio, 50%, HD")).toBeInTheDocument();
        expect(document.querySelector("[data-slot='enhancement-options']")).toBeNull();
    });

    it("adds a denoise at the user's default model and a strength of 100%, with its options closed", async () => {
        openFiles(HOLIDAY);
        render(
            <>
                <AddEnhancement />
                <EnhancementList />
            </>,
        );

        open();
        fireEvent.click(await screen.findByRole("menuitem", { name: "Denoise" }));

        // The model's own output, and Stockholm because the catalogue lists it first.
        await waitFor(() =>
            expect(stack()).toEqual([{ family: "denoise", codename: "stockholm", precision: "fp32", strength: 1 }]),
        );
        expect(await screen.findByText("Stockholm, 100%, HD")).toBeInTheDocument();
        expect(document.querySelector("[data-slot='enhancement-options']")).toBeNull();
    });

    it("adds a sharpen at the user's default model and a strength of 100%, with its options closed", async () => {
        openFiles(HOLIDAY);
        render(
            <>
                <AddEnhancement />
                <EnhancementList />
            </>,
        );

        open();
        fireEvent.click(await screen.findByRole("menuitem", { name: "Sharpen" }));

        // The model's own output, and Moscow because the catalogue lists it first.
        await waitFor(() =>
            expect(stack()).toEqual([{ family: "sharpen", codename: "moscow", precision: "fp32", strength: 1 }]),
        );
        expect(await screen.findByText("Moscow, 100%, HD")).toBeInTheDocument();
        expect(document.querySelector("[data-slot='enhancement-options']")).toBeNull();
    });

    it("adds a colorization at the user's default model and nothing else, with its options closed", async () => {
        openFiles(HOLIDAY);
        render(
            <>
                <AddEnhancement />
                <EnhancementList />
            </>,
        );

        open();
        fireEvent.click(await screen.findByRole("menuitem", { name: "Colorization" }));

        // Colorization takes no parameter, and Delhi because the catalogue lists it first.
        await waitFor(() =>
            expect(stack()).toEqual([{ family: "colorization", codename: "delhi", precision: "fp32" }]),
        );
        expect(await screen.findByText("Delhi, HD")).toBeInTheDocument();
        expect(document.querySelector("[data-slot='enhancement-options']")).toBeNull();
    });

    it("still offers, and adds, an enhancement Autopilot may not suggest", async () => {
        openFiles(HOLIDAY);
        useSettingsStore.setState({ autopilotExcluded: ["face_recovery", "upscale"] });

        render(<AddEnhancement />);
        open();

        // The exclusion governs what Autopilot proposes, not what the user can do: the menu is the
        // same with every family switched off as with none.
        const offered = await screen.findAllByRole("menuitem");
        expect(offered.map((item) => item.textContent)).toEqual([
            "Denoise",
            "Face Recovery",
            "Colorization",
            "Light Adjustment",
            "Color Balance",
            "Sharpen",
            "Upscale",
        ]);
        expect(await upscale()).not.toHaveAttribute("data-disabled");

        fireEvent.click(await upscale());

        await waitFor(() => expect(stack()?.map(({ family }) => family)).toEqual(["upscale"]));
    });

    it("shows an enhancement already in the stack as unavailable", async () => {
        openFiles(HOLIDAY);
        useEnhancementStore
            .getState()
            .addEnhancement(HOLIDAY.path, { family: "upscale", codename: "kyoto", precision: "fp32", scale: 2 });

        render(<AddEnhancement />);
        open();

        // Unavailable rather than absent: a menu that changed shape as a stack was built up would
        // move the remaining entries under the pointer.
        expect(await upscale()).toHaveAttribute("data-disabled");
    });

    it("leaves the stack alone when the same enhancement is chosen twice", async () => {
        openFiles(HOLIDAY);
        render(<AddEnhancement />);

        open();
        fireEvent.click(await upscale());
        await waitFor(() => expect(stack()).toHaveLength(1));

        open();
        fireEvent.click(await upscale());

        expect(stack()).toHaveLength(1);
    });
});
