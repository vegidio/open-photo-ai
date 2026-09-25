import { useCallback, useRef } from "react";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import type { Event } from "@tauri-apps/api/event";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import { act, screen } from "@testing-library/react";
import { toast as sonner } from "sonner";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import "@/i18n";
import i18n from "@/i18n";
import type { ImageRecord } from "@/ipc/images";
import { useFileStore } from "@/stores/files";
import {
    CANVAS,
    HOLIDAY,
    ON_CANVAS as onCanvas,
    ON_NAVBAR as onNavbar,
    render,
    resetFileStore,
    SUNSET,
} from "@/test/support";
import { useDroppedImages } from "./useDroppedImages";

// The webview's own drag-drop channel, mocked because `getCurrentWebview` reads a global Tauri's init
// script installs and jsdom has none. What is kept is the shape: registration is asynchronous and
// answers with the function that unregisters it, which is what the unmount path depends on.
const unlisten = vi.fn();
let deliver: (event: Event<DragDropEvent>) => void;

vi.mock("@tauri-apps/api/webview", () => ({
    getCurrentWebview: () => ({
        onDragDropEvent: (handler: (event: Event<DragDropEvent>) => void) => {
            deliver = handler;
            return Promise.resolve(unlisten);
        },
    }),
}));

// The host question, which the hit test asks because the three platforms disagree about the units a
// drop's position is in. `platform()` reads a global the plugin's init script installs and jsdom has
// none; `ipc/os.test.ts` pins the wrapper against the real one.
vi.mock("@/ipc/os", () => ({ isMacOs: vi.fn(() => false), isWindows: vi.fn(() => false) }));

// Mocked at the wrappers rather than at `invoke`: both are pinned by `ipc/images.test.ts`, and these
// tests are about which paths reach the describe and what the user is told about the rest.
vi.mock("@/ipc/images", async (original) => ({
    ...(await original<typeof import("@/ipc/images")>()),
    inputExtensions: vi.fn(() => Promise.resolve(["jpg", "png", "nef", "tif"])),
    describeImages: vi.fn(() => Promise.resolve<ImageRecord[]>([])),
}));

const { describeImages } = await import("@/ipc/images");
const described = describeImages as Mock;

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

const { isWindows } = await import("@/ipc/os");
const onWindows = isWindows as Mock;

/**
 * A component that is nothing but the listener over a canvas, so what is under test is the hook.
 *
 * What the hook answers is written onto the element as `data-over`, which is how a boolean returned
 * to a component becomes something a test can read - the real caller draws a rectangle with it.
 */
const Dropper = () => {
    const canvas = useRef<HTMLDivElement>(null);

    const over = useDroppedImages(canvas);

    const measure = useCallback((element: HTMLDivElement | null) => {
        canvas.current = element;

        if (element) element.getBoundingClientRect = () => CANVAS;
    }, []);

    return <div ref={measure} data-over={over} />;
};

/** The third position this file needs, below the canvas where the folded drawer stands. */
const onDrawer = new PhysicalPosition(400, 560);

/** Drops `paths` on the canvas and lets the describe settle, which is when the store is written. */
const drop = async (...paths: string[]) => dropAt(onCanvas, ...paths);

/** The drag crossing into the window at `position`, which is wherever it crossed and not this canvas. */
const dragEnter = async (position: PhysicalPosition) => {
    await act(async () => {
        deliver({ event: "tauri://drag-enter", id: 1, payload: { type: "enter", paths: [HOLIDAY.path], position } });
    });
};

/** The drag moving on to `position`, which is the only event that reports where it is now. */
const dragOver = async (position: PhysicalPosition) => {
    await act(async () => {
        deliver({ event: "tauri://drag-over", id: 1, payload: { type: "over", position } });
    });
};

/** The drag crossing back out of the window, which carries no position at all. */
const dragLeave = async () => {
    await act(async () => {
        deliver({ event: "tauri://drag-leave", id: 1, payload: { type: "leave" } });
    });
};

/** The same, let go of at `position` - which decides whether the drop is this window's to open. */
const dropAt = async (position: PhysicalPosition, ...paths: string[]) => {
    await act(async () => {
        deliver({ event: "tauri://drag-drop", id: 1, payload: { type: "drop", paths, position } });
    });
};

/** The mount `beforeEach` makes, so a case that renders a second one still reads the first. */
let view: HTMLElement;

/** Whether the hook currently reports a drag over the canvas. */
const hovering = () => view.firstElementChild?.getAttribute("data-over") === "true";

const files = () => useFileStore.getState().files;
const toast = () => document.querySelector("[data-sonner-toast]");

beforeEach(() => {
    resetFileStore();
    tracked.mockClear();

    // Sonner keeps its notices in a module-level store that outlives the render, and replays every
    // one that has not been dismissed to each new `Toaster` that subscribes - so a notice raised by
    // one case would reappear in the next case's window. Dismissed here, before the mount below, so
    // that `toast()` reports what the case under test raised and not what the file has raised so far.
    sonner.dismiss();

    // The listener under test, mounted before every case: the hook registers it on mount, and
    // `deliver` above is the handler it registered.
    view = render(<Dropper />).container;
});

describe("images dropped on the canvas", () => {
    it("opens what the application can open and names what it cannot", async () => {
        described.mockResolvedValueOnce([HOLIDAY, SUNSET]);

        await drop(HOLIDAY.path, "/Users/someone/Documents/invoice.pdf", SUNSET.path);

        // The photographs are described in one call and the document never reaches it: a batch the
        // user assembled is not refused because one file in it is not a photograph.
        expect(described).toHaveBeenCalledWith([HOLIDAY.path, SUNSET.path]);
        expect(files()).toEqual([HOLIDAY, SUNSET]);

        // Named by its own name rather than by its path, as the reference names it, and phrased by
        // the catalogue because the count decides the sentence.
        expect(
            await screen.findByText(i18n.t("toasts.unsupportedFiles", { count: 1, names: "invoice.pdf" })),
        ).toBeInTheDocument();

        // What was admitted and what was refused, by count and type: never a name.
        expect(tracked.mock.calls).toEqual([
            ["files_refused", { count: 1, formats: "pdf" }],
            ["files_added", { count: 2, source: "drop", formats: "nef,png" }],
        ]);
    });

    it("describes nothing when there is nothing it can open", async () => {
        await drop("/Users/someone/Movies/holiday.mov", "/Users/someone/Documents/invoice.pdf");

        // Not "describe nothing and add nothing": describing reads every byte of every file to
        // compute its identity, which for a dropped video is the whole video.
        expect(described).not.toHaveBeenCalled();
        expect(files()).toEqual([]);
        expect(tracked).toHaveBeenCalledExactlyOnceWith("files_refused", { count: 2, formats: "mov,pdf" });
        expect(
            await screen.findByText(i18n.t("toasts.unsupportedFiles", { count: 2, names: "holiday.mov, invoice.pdf" })),
        ).toBeInTheDocument();
    });

    it("says nothing when everything dropped can be opened", async () => {
        described.mockResolvedValueOnce([HOLIDAY, SUNSET]);

        await drop(HOLIDAY.path, SUNSET.path);

        expect(files()).toEqual([HOLIDAY, SUNSET]);
        expect(toast()).toBeNull();
        expect(tracked.mock.calls.map(([event]) => event)).toEqual(["files_added"]);
    });

    it("judges a file by its extension however it is cased, and refuses a name that has none", async () => {
        described.mockResolvedValueOnce([HOLIDAY]);

        await drop("/Users/someone/Pictures/HOLIDAY.PNG", "/Users/someone/Pictures/Holidays 2026");

        expect(described).toHaveBeenCalledWith(["/Users/someone/Pictures/HOLIDAY.PNG"]);
        // A dropped folder is a name with no extension, which no decoder lists - so it is refused
        // rather than hashed.
        expect(
            await screen.findByText(i18n.t("toasts.unsupportedFiles", { count: 1, names: "Holidays 2026" })),
        ).toBeInTheDocument();
    });

    it("opens nothing for a drag that is only passing over the window", async () => {
        await dragEnter(onCanvas);
        await dragOver(onCanvas);
        await dragLeave();

        // The three report where a drag is; nothing has been let go of, so there is nothing for any
        // of them to open or to say. What they do drive is the hover below.
        expect(described).not.toHaveBeenCalled();
        expect(files()).toEqual([]);
        expect(toast()).toBeNull();
    });

    it("ignores a drop let go of anywhere but the canvas", async () => {
        await dropAt(onNavbar, HOLIDAY.path);
        await dropAt(onDrawer, SUNSET.path);

        // Not even the extension list is asked for, and nothing is said to the user: a photograph let
        // go of over the navbar or the drawer was let go of over a region that never offered to take
        // it, which is not the same as a file this application cannot open.
        expect(described).not.toHaveBeenCalled();
        expect(files()).toEqual([]);
        expect(toast()).toBeNull();
    });

    it("takes the canvas's top edge and not the first pixel under its bottom one", async () => {
        described.mockResolvedValueOnce([HOLIDAY]);

        await dropAt(new PhysicalPosition(0, CANVAS.bottom), SUNSET.path);
        expect(described).not.toHaveBeenCalled();

        await dropAt(new PhysicalPosition(0, CANVAS.top), HOLIDAY.path);

        // The rectangle owns the pixels it is drawn on: its top row is the canvas, and the row at its
        // bottom is the first row of whatever is drawn under it.
        expect(files()).toEqual([HOLIDAY]);
    });

    it("reads the position in the DOM's own pixels off a Retina Mac, where Tauri reports points", async () => {
        described.mockResolvedValueOnce([HOLIDAY]);

        // The type says physical and macOS reports points, so a Retina scale factor changes nothing
        // about how a position is read. Divided by it - which is what the type invites - the drawer's
        // 560 would halve to 280 and land in the middle of the canvas.
        vi.spyOn(window, "devicePixelRatio", "get").mockReturnValue(2);

        await dropAt(onDrawer, SUNSET.path);
        expect(described).not.toHaveBeenCalled();

        await dropAt(onCanvas, HOLIDAY.path);

        expect(files()).toEqual([HOLIDAY]);
    });

    it("divides the position by the scale factor on Windows, which reports device pixels", async () => {
        described.mockResolvedValueOnce([HOLIDAY]);
        onWindows.mockReturnValue(true);
        vi.spyOn(window, "devicePixelRatio", "get").mockReturnValue(2);

        // Both are read at half what they say: 60 is the navbar's 30 rather than the canvas, and 1000
        // is the canvas's 500 rather than a position past the bottom of a 600px window.
        await dropAt(new PhysicalPosition(400, 60), SUNSET.path);
        expect(described).not.toHaveBeenCalled();

        await dropAt(new PhysicalPosition(400, 1000), HOLIDAY.path);

        expect(files()).toEqual([HOLIDAY]);
    });

    it("stops listening when the window's shell goes away", async () => {
        // Its own mount, unmounted inside the test rather than by the cleanup that follows it, so the
        // count below is this one's and not the one every other case leaves behind.
        const { unmount } = render(<Dropper />);

        await act(async () => {
            unmount();
        });

        // Awaited through the registration promise, which is the whole reason the cleanup cannot be
        // a bare call: a reloaded webview in development would otherwise accumulate one handler per
        // reload, and every one of them would open the next drop again.
        expect(unlisten).toHaveBeenCalledTimes(1);
    });
});

/**
 * The second answer the same listener gives: whether a drag is over the canvas right now (D1, D4).
 *
 * It is the `isOver` the drop path already makes, asked earlier - which is the point. A second hook
 * with its own `onDragDropEvent` would need its own copy of the scale correction and the hit test,
 * and two copies are how the rectangle that is drawn and the rectangle that accepts a drop come to
 * disagree.
 */
describe("a drag over the canvas", () => {
    it("reports nothing until a drag arrives", () => {
        expect(hovering()).toBe(false);
    });

    it("reports a drag that enters the window elsewhere and moves onto the canvas", async () => {
        // The case an `enter`-only implementation misses, and the reason the position is recomputed
        // on both: `enter` is a *window* event, fired once wherever the drag crossed in. Arriving
        // over the navbar and sliding down onto the canvas produces exactly this sequence and never
        // a second `enter`.
        await dragEnter(onNavbar);
        expect(hovering()).toBe(false);

        await dragOver(onCanvas);

        expect(hovering()).toBe(true);
    });

    it("reports a drag that crosses straight into the canvas", async () => {
        await dragEnter(onCanvas);

        expect(hovering()).toBe(true);
    });

    it("stops reporting when the drag moves off the canvas without leaving the window", async () => {
        await dragEnter(onCanvas);

        await dragOver(onDrawer);

        // No special case for crossing out: the same recomputation that lit it up puts it out, which
        // is what makes the mark track the rectangle rather than the window.
        expect(hovering()).toBe(false);
    });

    it("stops reporting when the drag leaves the window", async () => {
        await dragEnter(onCanvas);

        await dragLeave();

        // `leave` carries no position, which is why it clears unconditionally rather than being
        // hit-tested like the two above.
        expect(hovering()).toBe(false);
    });

    it("stops reporting when the files are let go of, and still opens them", async () => {
        described.mockResolvedValueOnce([HOLIDAY]);
        await dragEnter(onCanvas);

        await drop(HOLIDAY.path);

        expect(hovering()).toBe(false);
        expect(files()).toEqual([HOLIDAY]);
    });

    it("stops reporting when the files are let go of somewhere that takes none", async () => {
        await dragEnter(onCanvas);

        await dropAt(onNavbar, HOLIDAY.path);

        // The mark goes out because the drag is over, not because this window took anything.
        expect(hovering()).toBe(false);
        expect(described).not.toHaveBeenCalled();
    });

    it("answers for the rectangle the drop is hit-tested against, not for the window", async () => {
        // One hit test behind both answers: a position the drop path refuses is a position that
        // draws nothing, and the two cannot be made to disagree without editing the same line twice.
        await dragOver(onNavbar);
        expect(hovering()).toBe(false);

        await dragOver(onDrawer);
        expect(hovering()).toBe(false);

        await dragOver(new PhysicalPosition(0, CANVAS.top));
        expect(hovering()).toBe(true);

        await dragOver(new PhysicalPosition(0, CANVAS.bottom));
        expect(hovering()).toBe(false);
    });
});
