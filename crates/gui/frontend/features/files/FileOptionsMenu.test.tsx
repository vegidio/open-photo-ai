import { fireEvent, screen, waitFor } from "@testing-library/react";
import { toast as sonner } from "sonner";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import i18n from "@/i18n";
import { revealImage } from "@/ipc/images";
import { isMacOs, isWindows } from "@/ipc/os";
import { useFileStore } from "@/stores/files";
import { useTransformStore } from "@/stores/transform";
import { HOLIDAY, openFiles, render, resetFileStore, SUNSET } from "@/test/support";
import { FileOptionsMenu, NAVBAR_ANCHOR } from "./FileOptionsMenu";

// The reveal is pinned by `ipc/images.test.ts` against the wire; here it is mocked so this file is
// about what the menu asks for rather than about what `invoke` sends.
vi.mock("@/ipc/images", async (importOriginal) => ({
    ...(await importOriginal<typeof import("@/ipc/images")>()),
    revealImage: vi.fn(() => Promise.resolve()),
}));

// jsdom has no Tauri OS plugin global, so `platform()` throws there - which is why every component
// test mocks this module rather than the global. See `ipc/os.test.ts`, which pins the wrapper itself.
vi.mock("@/ipc/os", () => ({ isMacOs: vi.fn(() => false), isWindows: vi.fn(() => false) }));

const revealed = revealImage as unknown as Mock;
const onMacOs = isMacOs as unknown as Mock;
const onWindows = isWindows as unknown as Mock;

const paths = () => useFileStore.getState().files.map((file) => file.path);
const toast = () => document.querySelector("[data-sonner-toast]");

/** The menu, behind a trigger named so a test can find it without knowing what either call site draws. */
const mount = (file = HOLIDAY) =>
    render(
        <FileOptionsMenu file={file} anchor={NAVBAR_ANCHOR}>
            <button type="button">options</button>
        </FileOptionsMenu>,
    );

/**
 * Opens the menu.
 *
 * Through the keyboard, as `GeneralPage.test.tsx` opens a Radix select: jsdom dispatches no
 * `pointerdown` from a `click`, and Radix's trigger opens on the pointer rather than on the click.
 */
const open = () => fireEvent.keyDown(screen.getByRole("button", { name: "options" }), { key: "Enter" });

/** Opens the menu and chooses the item with this name. */
const choose = async (name: RegExp) => {
    open();
    fireEvent.click(await screen.findByRole("menuitem", { name }));
};

beforeEach(() => {
    vi.clearAllMocks();

    // Sonner keeps its notices in a module-level store that outlives the render and replays every one
    // that has not been dismissed to each new `Toaster` - so a notice raised by one case would
    // reappear in the next. See `useDroppedImages.test.tsx`, which says the same thing at length.
    sonner.dismiss();

    onMacOs.mockReturnValue(false);
    onWindows.mockReturnValue(false);
    revealed.mockResolvedValue(undefined);
    resetFileStore();
    useTransformStore.setState(useTransformStore.getInitialState(), true);
});

describe("FileOptionsMenu", () => {
    it("offers the three actions, with the reveal separated from the two closes", async () => {
        openFiles(HOLIDAY, SUNSET);
        mount();

        open();

        // The order is the design's and the reference's, and it is the order the separator's position
        // is only meaningful against.
        const items = await screen.findAllByRole("menuitem");
        expect(items.map((item) => item.textContent)).toEqual([
            "Close image",
            "Close all images",
            "Show in File Manager",
        ]);

        expect(screen.getByRole("separator")).toBeInTheDocument();
    });

    it("closes the file it was handed, not the current one", async () => {
        // The whole of why the menu takes a file rather than reading the store: a thumbnail's menu acts
        // on the photograph whose thumbnail carries it, which is very often not the one being drawn.
        openFiles(HOLIDAY, SUNSET);
        mount(SUNSET);

        await choose(/^Close image$/);

        await waitFor(() => expect(paths()).toEqual([HOLIDAY.path]));
    });

    it("closes every open image", async () => {
        openFiles(HOLIDAY, SUNSET);
        mount();

        await choose(/^Close all images$/);

        await waitFor(() => expect(paths()).toEqual([]));
    });

    it("asks for the file it was handed to be shown in the file manager", async () => {
        openFiles(HOLIDAY, SUNSET);
        mount(SUNSET);

        await choose(/^Show in/);

        expect(revealed).toHaveBeenCalledWith(SUNSET.path);
    });

    it("names the file manager the platform actually has", async () => {
        // One translatable sentence per platform through i18next's `context`, rather than a product
        // name interpolated into a sentence whose word order is not universal.
        for (const [platform, name] of [
            ["darwin", "Show in Finder"],
            ["windows", "Show in Explorer"],
            ["other", "Show in File Manager"],
        ] as const) {
            onMacOs.mockReturnValue(platform === "darwin");
            onWindows.mockReturnValue(platform === "windows");

            const { unmount } = mount();
            open();

            expect(await screen.findByRole("menuitem", { name })).toBeInTheDocument();

            unmount();
        }
    });

    it("reports a reveal that did not happen", async () => {
        // A file can be moved or deleted after it was opened, and a menu item that quietly does nothing
        // reads as a broken one. The catalogue's sentence goes to the user; the reason goes to the
        // console, which is why this asserts on the former.
        console.error = vi.fn();
        revealed.mockRejectedValueOnce({ kind: "revealImage", message: "no such file or directory" });
        openFiles(HOLIDAY);
        mount();

        await choose(/^Show in/);

        expect(await screen.findByText(i18n.t("errors.revealFileFailed"))).toBeInTheDocument();
    });

    it("says nothing when the reveal succeeds", async () => {
        openFiles(HOLIDAY);
        mount();

        await choose(/^Show in/);

        await waitFor(() => expect(revealed).toHaveBeenCalled());
        expect(toast()).toBeNull();
    });

    it("shuts before it writes to the store", async () => {
        // Two of the three actions destroy the trigger this menu is anchored to, so the menu has to be
        // gone by the time the unmount happens - otherwise Radix returns focus into a subtree that no
        // longer exists. Asserted as "no menu is left open afterwards", which is what that comes to
        // from outside the component.
        openFiles(HOLIDAY, SUNSET);
        mount();

        await choose(/^Close all images$/);

        await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
        expect(paths()).toEqual([]);
    });
});
