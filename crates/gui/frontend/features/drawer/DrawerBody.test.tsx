import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import type { ImageRecord } from "@/ipc/images";
import { useFileStore } from "@/stores/files";
import { openFiles, render, resetFileStore, stubStripGeometry } from "@/test/support";
import { DrawerBody } from "./DrawerBody";

vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

// jsdom has no Tauri OS plugin global, so `platform()` throws there. The file options menu on every
// thumbnail names the platform's file manager, which is what brings this into a drawer test.
vi.mock("@/ipc/os", () => ({ isMacOs: vi.fn(() => false), isWindows: vi.fn(() => false) }));

/** The strip's own constants, restated here so the arithmetic below is readable rather than derived. */
const STRIP_HEIGHT = 128;
const PADDING_Y = 12;
const GAP = 16;
const ITEM = STRIP_HEIGHT - 2 * PADDING_Y;
const STEP = ITEM + GAP;

/** Wide enough for a handful of thumbnails and narrower than the long list below. */
const STRIP_WIDTH = 640;

const image = (index: number): ImageRecord => ({
    path: `/Users/someone/Pictures/photo-${index}.png`,
    identity: `${index}`.padStart(16, "0"),
    width: 3000,
    height: 2000,
    extension: "png",
    size: 8_421_504,
});

const strip = () => screen.getByTestId("strip-root").querySelector("[data-slot='drawer-strip']") as HTMLElement;
const items = () => [...document.querySelectorAll("[data-slot='drawer-item']")];
const spacers = () => [...document.querySelectorAll("[data-slot='drawer-spacer']")] as HTMLElement[];
const widthOf = (element: HTMLElement | undefined) => Number.parseInt(element?.style.width ?? "0", 10);

const renderStrip = () =>
    render(
        <div data-testid="strip-root">
            <DrawerBody height={STRIP_HEIGHT} />
        </div>,
    );

/**
 * Scrolls the strip the way the user would.
 *
 * jsdom lays nothing out, so `scrollLeft` is pinned at 0 by a scroll width of 0 and cannot simply be
 * assigned. The virtualizer reads the property and listens for the event, which is exactly the pair
 * this replaces - there is nothing else about a real scroll that it observes.
 */
const scrollTo = (offset: number) => {
    const element = strip();
    Object.defineProperty(element, "scrollLeft", { configurable: true, value: offset });

    act(() => {
        fireEvent.scroll(element);
    });
};

beforeEach(() => {
    resetFileStore();

    // Without it the strip mounts nothing: jsdom lays nothing out, and the virtualizer computes no
    // range at all for a scroll element of zero size. The width and the height are passed rather than
    // taken as defaults because the arithmetic below is written against them.
    stubStripGeometry({ width: STRIP_WIDTH, height: STRIP_HEIGHT });
});

describe("DrawerBody", () => {
    it("mounts every thumbnail of a list that fits, and no spacer", () => {
        openFiles(image(0), image(1), image(2));
        renderStrip();

        expect(items()).toHaveLength(3);
        expect(spacers()).toHaveLength(0);
    });

    it("draws one thumbnail per open image, in the order they were opened", () => {
        openFiles(image(0), image(1), image(2));
        renderStrip();

        expect(items().map((item) => item.textContent)).toEqual(["photo-0.png", "photo-1.png", "photo-2.png"]);
    });

    describe("with several hundred images open", () => {
        const COUNT = 300;

        beforeEach(() => {
            openFiles(...Array.from({ length: COUNT }, (_, index) => image(index)));
        });

        it("mounts a bounded handful rather than one thumbnail per file", () => {
            renderStrip();

            // Roughly what fits across the strip plus the overscan on either side - and, crucially,
            // a number that does not grow with the 300 files behind it.
            expect(items().length).toBeLessThan(20);
        });

        it("stands the unmounted items in with a spacer on each side once it is scrolled into them", () => {
            renderStrip();

            // At rest the mounted set starts at the first file, so there is nothing before it to
            // stand in for: one spacer, at the tail.
            expect(spacers()).toHaveLength(1);

            scrollTo(100 * STEP);

            expect(spacers()).toHaveLength(2);
        });

        it("scrolls over an extent that accounts for every file, drawn or not", () => {
            renderStrip();
            scrollTo(100 * STEP);

            const [lead, tail] = spacers();

            // The two spacers plus the mounted items are the whole scrolling extent, which is the
            // 300 files' own width. Each spacer loses a `GAP` to the flex gap beside it, which is
            // the two the sum is short.
            expect(widthOf(lead) + widthOf(tail) + items().length * STEP).toBe(COUNT * STEP - 2 * GAP);
        });

        it("reaches the last file", () => {
            renderStrip();
            scrollTo(COUNT * STEP - STRIP_WIDTH);

            expect(screen.getByRole("button", { name: `photo-${COUNT - 1}.png` })).toBeInTheDocument();
        });
    });

    it("makes a thumbnail the current image when it is clicked", () => {
        openFiles(image(0), image(1), image(2));
        renderStrip();

        fireEvent.click(screen.getByRole("button", { name: "photo-2.png" }));

        expect(useFileStore.getState().currentIndex).toBe(2);
    });
});
