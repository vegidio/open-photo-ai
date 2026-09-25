import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { Drawer } from "@/features/drawer/Drawer";
import { PreviewEmpty } from "@/features/preview/PreviewEmpty";
import "@/i18n";
import type { ImageRecord } from "@/ipc/images";
import { useFileStore } from "@/stores/files";
import { HOLIDAY, render, resetFileStore, SUNSET } from "@/test/support";

// Mocked at the IPC wrapper rather than at `invoke`: what the picker does with the two words is
// `ipc/images.ts`'s contract and is pinned by its own test. These are about what the two controls ask
// for and what becomes of the answer.
vi.mock("@/ipc/images", async (original) => ({
    ...(await original<typeof import("@/ipc/images")>()),
    openImages: vi.fn(() => Promise.resolve<ImageRecord[]>([])),
}));

const { openImages } = await import("@/ipc/images");

// Mocked at `lib/faro.ts`'s own boundary: what `track` does with an event is pinned by `faro.test.ts`.
vi.mock("@/lib/faro", () => ({
    track: vi.fn(),
    sendError: vi.fn(),
    pauseFaro: vi.fn(),
    // Untraced, as before Faro starts: the request goes out exactly as `invoke` alone would send it.
    traced: (_name: string, send: () => Promise<unknown>) => send(),
}));
const { track } = await import("@/lib/faro");
const tracked = track as Mock;

// jsdom has no Tauri OS plugin global, so `platform()` throws there. The file options menu on every
// thumbnail names the platform's file manager, which is what brings this into a drawer test.
vi.mock("@/ipc/os", () => ({ isMacOs: vi.fn(() => false), isWindows: vi.fn(() => false) }));
const picker = openImages as Mock;

/** Presses a control and lets the picker's promise settle, which is when the store is written. */
const press = async (name: string) => {
    await act(async () => {
        fireEvent.click(screen.getByRole("button", { name }));
    });
};

const files = () => useFileStore.getState().files;

beforeEach(() => {
    resetFileStore();
    tracked.mockClear();
});

describe("the two controls that add images", () => {
    it.each([
        ["Browse images", () => render(<PreviewEmpty />)],
        // Available whether or not an image is open, which is why the drawer is rendered in the state
        // where every other control on it is unavailable.
        ["Add images", () => render(<Drawer />)],
    ])("%s opens the picker with the catalogue's own wording", async (name, mount) => {
        mount();

        await press(name);

        // Both strings translated on this side, because the Rust command deliberately holds none: a
        // second catalogue there would be a second thing to keep in step with thirteen locales.
        expect(picker).toHaveBeenCalledWith("Select Image", "Images");
    });

    it.each([
        ["Browse images", () => render(<PreviewEmpty />)],
        ["Add images", () => render(<Drawer />)],
    ])("%s opens what the user chose", async (name, mount) => {
        picker.mockResolvedValueOnce([HOLIDAY, SUNSET]);
        mount();

        await press(name);

        // The same two records, in the order Rust described them, reaching the store by the same
        // route: one hook behind two controls is what makes the two buttons one behaviour.
        expect(files()).toEqual([HOLIDAY, SUNSET]);
    });

    it.each([
        ["Browse images", "empty", () => render(<PreviewEmpty />)],
        ["Add images", "browse", () => render(<Drawer />)],
    ])("%s sends files_added with its own source, the count and the types alone", async (name, source, mount) => {
        picker.mockResolvedValueOnce([HOLIDAY, SUNSET]);
        mount();

        await press(name);

        expect(tracked).toHaveBeenCalledExactlyOnceWith("files_added", { count: 2, source, formats: "nef,png" });
    });

    it("says nothing and changes nothing when the picker is dismissed", async () => {
        // An empty answer is a dismissal, or a picker the platform would not open, which Rust cannot
        // tell apart and does not pretend to. Both mean the user added no images.
        picker.mockResolvedValueOnce([]);
        render(<PreviewEmpty />);

        await press("Browse images");

        expect(files()).toEqual([]);
        expect(tracked).not.toHaveBeenCalled();
        // A notice about the user changing their mind is a notice about nothing. Queried through the
        // toaster's own region rather than by text, because there is no text to name.
        expect(document.querySelector("[data-sonner-toast]")).toBeNull();
    });

    it("keeps the images already open when a dismissal follows them", async () => {
        picker.mockResolvedValueOnce([HOLIDAY]);
        render(<PreviewEmpty />);
        await press("Browse images");

        picker.mockResolvedValueOnce([]);
        await press("Browse images");

        expect(files()).toEqual([HOLIDAY]);
    });

    it("reports a failed picker to the console and nowhere else", async () => {
        // Rust has already written it to the log file, and there is no screen for it: the only
        // failure this command has is the application closing mid-selection.
        const logged = vi.spyOn(console, "error").mockImplementation(() => {});
        picker.mockRejectedValueOnce({ kind: "openImages", message: "the files could not be described" });
        render(<PreviewEmpty />);

        await press("Browse images");

        expect(logged).toHaveBeenCalled();
        expect(files()).toEqual([]);
        expect(document.querySelector("[data-sonner-toast]")).toBeNull();
    });
});
