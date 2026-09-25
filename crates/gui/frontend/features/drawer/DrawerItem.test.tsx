import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import type { ImageRecord } from "@/ipc/images";
import { FILE_MENU_GLYPH, THUMBNAIL_BOUND } from "@/lib/constants";
import { useFileStore } from "@/stores/files";
import { frame, HOLIDAY, openFiles, render, resetCropStore, resetFileStore, SUNSET } from "@/test/support";
import { DrawerItem } from "./DrawerItem";

// The global Tauri's init script installs, which jsdom has none of. Spelled as the macOS form so the
// bound the item asks for is readable in the `src` it produces.
vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

// jsdom has no Tauri OS plugin global, so `platform()` throws there. The file options menu on every
// thumbnail names the platform's file manager, which is what brings this into a drawer test.
vi.mock("@/ipc/os", () => ({ isMacOs: vi.fn(() => false), isWindows: vi.fn(() => false) }));

const onClick = vi.fn();

const renderItem = (file: ImageRecord, { index = 0, current = false } = {}) =>
    render(<DrawerItem file={file} index={index} current={current} onClick={onClick} />);

const thumbnail = (name: string) => screen.getByRole("button", { name });
const selected = () => [...useFileStore.getState().selectedPaths];

beforeEach(() => {
    resetFileStore();
    resetCropStore();
});

describe("DrawerItem", () => {
    it("draws the whole photograph whatever framing it carries", () => {
        frame(HOLIDAY);

        const { container } = renderItem(HOLIDAY);

        // Unframed deliberately, for the reason the comment on `DrawerItem`'s `<img>` gives - and
        // asserted rather than left implicit because a cropped miniature beside an uncropped strip
        // reads as an oversight.
        expect(container.querySelector("img")).toHaveAttribute(
            "src",
            `opai://localhost/${HOLIDAY.identity}?size=${THUMBNAIL_BOUND}`,
        );
    });

    it("is a button named after the file rather than after its path", () => {
        renderItem(SUNSET);

        // The record's path is several folders deep; what the bar draws and what the button is
        // announced as is the name at the end of it.
        expect(thumbnail("sunset over the harbour.nef")).toBeInTheDocument();
    });

    it("keeps the item reachable by the file's name once the bar is no longer inside it", () => {
        // The bar is a sibling of the button, so it lends the button no name - and an unnamed button
        // renders identically and is invisible in every other assertion in this file. This is the one
        // that would catch it.
        renderItem(SUNSET);

        const item = thumbnail("sunset over the harbour.nef");

        expect(item).toHaveAttribute("aria-label", "sunset over the harbour.nef");
        expect(item.querySelector("img")).toBeInTheDocument();
    });

    it("draws the three-dot control as a button of its own, nested in neither direction", () => {
        // A button inside a button is invalid markup, and it is the same nesting the checkbox is a
        // sibling to avoid. Both directions, because the fix for one is a natural way to cause the
        // other.
        renderItem(HOLIDAY);

        const item = thumbnail("holiday.png");
        const trigger = screen.getByRole("button", { name: "File options for holiday.png" });

        expect(trigger).not.toBe(item);
        expect(item.contains(trigger)).toBe(false);
        expect(trigger.contains(item)).toBe(false);
    });

    it("sizes the trigger to the glyph, which is what the menu's offset is measured against", () => {
        // Padding here would move the menu sideways and nothing would fail - see `FILE_MENU_GLYPH`.
        // jsdom lays nothing out, so the constant is what is assertable - and it is the whole of the
        // defence.
        renderItem(HOLIDAY);

        const trigger = screen.getByRole("button", { name: "File options for holiday.png" });

        expect(trigger.style.width).toBe(`${FILE_MENU_GLYPH}px`);
        expect(trigger.style.height).toBe(`${FILE_MENU_GLYPH}px`);
    });

    it("reports its own index when it is pressed", () => {
        renderItem(SUNSET, { index: 7 });

        fireEvent.click(thumbnail("sunset over the harbour.nef"));

        // The index goes back to the parent rather than the item writing the store, so the strip can
        // hold one stable handler and this component can stay memoized.
        expect(onClick).toHaveBeenCalledExactlyOnceWith(7);
    });

    it("asks for the photograph at the strip's own bound", () => {
        const { container } = renderItem(HOLIDAY);

        // 384 rather than the reference's 100: the item is a 104px square filled by a cover crop, so
        // on a 2x display the short edge needs 208 device pixels - which a 16:9 photograph reaches
        // only at 370 on its long edge. `THUMBNAIL_BOUND` has the rest of the arithmetic.
        expect(container.querySelector("img")).toHaveAttribute("src", `opai://localhost/${HOLIDAY.identity}?size=384`);
    });

    it("defers the pixels until the thumbnail is near the visible part of the strip", () => {
        const { container } = renderItem(HOLIDAY);

        expect(container.querySelector("img")).toHaveAttribute("loading", "lazy");
    });

    it("draws the image decoratively, so the file is announced once and as a button", () => {
        const { container } = renderItem(HOLIDAY);

        // An image labelled `common.previewAlt` inside a button labelled holiday.png would be
        // announced twice, once with the wrong noun.
        expect(container.querySelector("img")).toHaveAttribute("alt", "");
        expect(screen.queryByRole("img", { name: "Preview" })).not.toBeInTheDocument();
    });

    it("picks and unpicks the file without changing which image is current", () => {
        openFiles(HOLIDAY, SUNSET);
        useFileStore.getState().setCurrentIndex(1);
        renderItem(SUNSET, { index: 1 });

        const checkbox = screen.getByRole("checkbox", { name: "Select sunset over the harbour.nef" });

        fireEvent.click(checkbox);
        expect(selected().sort()).toEqual([HOLIDAY.path, SUNSET.path].sort());

        fireEvent.click(checkbox);
        expect(selected()).toEqual([HOLIDAY.path]);

        // Picking is what the export queue will read; being current is what the canvas draws.
        expect(useFileStore.getState().currentIndex).toBe(1);
        expect(onClick).not.toHaveBeenCalled();
    });

    it("reports whether the file is already picked", () => {
        openFiles(HOLIDAY);
        renderItem(HOLIDAY);

        // Opening into an empty window picks the first file, so this one arrives ticked.
        expect(screen.getByRole("checkbox", { name: "Select holiday.png" })).toBeChecked();
    });

    it("marks the current image, and marks no other", () => {
        const { container: currentItem } = renderItem(HOLIDAY, { current: true });
        const { container: otherItem } = renderItem(SUNSET, { index: 1 });

        const wrapper = (container: HTMLElement) => container.querySelector("[data-slot='drawer-item']");

        // The outline and the inverted name bar together, which is what the strip says "this one"
        // with. Both are asserted because either alone reads as a styling accident.
        expect(wrapper(currentItem)).toHaveClass("outline-primary");
        expect(wrapper(otherItem)).not.toHaveClass("outline-primary");

        // Queried off the item rather than off the button: the name bar is a sibling of it, which is
        // what lets the three-dot control be a real trigger instead of a button inside a button.
        expect(wrapper(currentItem)?.querySelector(".bg-foreground\\/85")).toBeInTheDocument();
        expect(wrapper(otherItem)?.querySelector(".bg-foreground\\/85")).toBeNull();
    });

    /** A file whose bytes could not be read has no identity, so nothing can serve its pixels. */
    it("draws the surface and the name bar for a file with no identity", () => {
        // Rebuilt without the key rather than with `identity: undefined`: `exactOptionalPropertyTypes`
        // is on, so an optional property means the key is genuinely absent - which is what Rust sends
        // for a file whose bytes it could not read.
        const { identity: _identity, ...unreadable } = SUNSET;
        const { container } = renderItem(unreadable);

        const image = container.querySelector("img");
        expect(image).not.toHaveAttribute("src");

        // Still in the strip, still named, still pickable: the failure is that the photograph is not
        // drawn, not that the file has stopped being one of the open images.
        expect(thumbnail("sunset over the harbour.nef")).toBeInTheDocument();
        expect(screen.getByRole("checkbox", { name: "Select sunset over the harbour.nef" })).toBeEnabled();
        expect(container.querySelector(".bg-surface-thumbnail")).toBeInTheDocument();
    });
});
