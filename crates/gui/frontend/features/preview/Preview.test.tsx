import { PhysicalPosition } from "@tauri-apps/api/dpi";
import type { Event } from "@tauri-apps/api/event";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import { act, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import "@/i18n";
import type { ImageRecord } from "@/ipc/images";
import { useDrawerStore } from "@/stores/drawer";
import { useFileStore } from "@/stores/files";
import { useSettingsStore } from "@/stores/settings";
import {
    applyBackground,
    CANVAS,
    HOLIDAY,
    ON_CANVAS as onCanvas,
    ON_NAVBAR as onNavbar,
    openFiles,
    render,
    resetFileStore,
    resetSettingsStore,
    stubCanvas2D,
} from "@/test/support";
import { Preview } from "./Preview";

// `convertFileSrc` reads a global Tauri's init script installs, which jsdom has none of. What it
// answers is asserted against in `PreviewImage.test.tsx`; here the canvas only has to be able to draw.
vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

// And the webview's drag-drop channel, which `getCurrentWebview` reads another of those globals for:
// the canvas registers the drop listener on itself. What the listener does with a drop is
// `hooks/useDroppedImages.test.tsx`'s subject; the handler is captured here because what a drag in
// progress *draws* is this file's.
let deliver: (event: Event<DragDropEvent>) => void;

vi.mock("@tauri-apps/api/webview", () => ({
    getCurrentWebview: () => ({
        onDragDropEvent: (handler: (event: Event<DragDropEvent>) => void) => {
            deliver = handler;
            return Promise.resolve(() => {});
        },
    }),
}));

// And the progress event, which `listen` reads a third of those globals for: a mounted canvas
// subscribes to it for as long as a file is open. What the listener does with a report is
// `hooks/useEnhancementRun.test.tsx`'s subject; here it only has to be registrable.
vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));

// The host question the hit test asks, for the reason `hooks/useDroppedImages.ts` gives. It reads a
// global the plugin's init script installs and jsdom has none.
vi.mock("@/ipc/os", () => ({ isMacOs: vi.fn(() => true), isWindows: vi.fn(() => false) }));

// The two calls the drop path makes once it has decided a drop is this rectangle's. Mocked at the
// wrappers rather than at `invoke`, as `hooks/useDroppedImages.test.tsx` mocks them and for the same
// reason: what is under test here is which rectangle takes a drop, not what Rust answers about the
// files in it. Without them a drop would fail in the IPC bridge, and a test asserting that nothing
// was opened would pass whether or not the rectangle refused it.
vi.mock("@/ipc/images", async (original) => ({
    ...(await original<typeof import("@/ipc/images")>()),
    inputExtensions: vi.fn(() => Promise.resolve(["jpg", "png", "nef", "tif"])),
    describeImages: vi.fn(() => Promise.resolve<ImageRecord[]>([])),
}));

const { describeImages } = await import("@/ipc/images");
const described = describeImages as Mock;

beforeEach(() => {
    described.mockClear();
    resetFileStore();
    localStorage.clear();
    useSettingsStore.setState(useSettingsStore.getInitialState(), true);
    resetSettingsStore();
    useDrawerStore.setState({ open: false });
    // The canvas draws a particle field on a fresh profile, and jsdom has no 2D context to draw on -
    // loudly, on the console, for every test that mounts this.
    stubCanvas2D();
});

/** The canvas's own element: the one the surface's classes are on and the field is drawn inside. */
const canvas = () => document.querySelector<HTMLElement>("[data-slot='preview-canvas']");

const field = () => document.querySelector<HTMLElement>("[data-slot='particles']");

/**
 * The region a drop is offered on and hit-tested against. It is always mounted, because it is the
 * hit test; what comes and goes with a drag is the shine on it.
 */
const region = () => document.querySelector<HTMLElement>("[data-slot='preview-drop-region']");

/** The mark itself, which is the part that comes and goes with a drag. */
const shine = () => document.querySelector<HTMLElement>("[data-slot='shine-border']");

/** The same, with the drawer unfolded: its 128px body stands on the bottom of the region. */
const WITH_DRAWER = new DOMRect(0, 40, 800, 384);

const measure = (rect: DOMRect = CANVAS) => {
    const element = region();

    if (element) element.getBoundingClientRect = () => rect;
};

/** Over the strip of thumbnails an unfolded drawer shows, which covers the bottom of the canvas. */
const onOpenDrawer = new PhysicalPosition(400, 500);

/** Over the part of the canvas an unfolded drawer leaves, which still offers and still takes a drop. */
const aboveDrawer = new PhysicalPosition(400, 200);

/** A drag arriving over the canvas, which is one `enter` - the event that carries where it crossed in. */
const dragOnto = async (position: PhysicalPosition, rect?: DOMRect) => {
    measure(rect);

    await act(async () => {
        deliver({ event: "tauri://drag-enter", id: 1, payload: { type: "enter", paths: [HOLIDAY.path], position } });
    });
};

const dragAway = async () => {
    await act(async () => {
        deliver({ event: "tauri://drag-leave", id: 1, payload: { type: "leave" } });
    });
};

const letGo = async (position: PhysicalPosition, ...paths: string[]) => {
    await act(async () => {
        deliver({ event: "tauri://drag-drop", id: 1, payload: { type: "drop", paths, position } });
    });
};

describe("Preview", () => {
    it("states the invitation as two lines while nothing is open", () => {
        const { container } = render(<Preview />);

        // The `<br>` is the assertion. The catalogue carries it inside the string so a translator can
        // move or drop the break, and it only survives because the component renders the key through
        // `<Trans>` - `t()` would put `<br/>` on screen as visible text, which no text query would
        // catch.
        expect(container.querySelector("p")).toContainHTML("Drag and drop images<br>to start editing them");
    });

    it("offers the other way in", () => {
        render(<Preview />);

        expect(screen.getByRole("button", { name: "Browse images" })).toBeEnabled();
    });

    it("draws the image in place of the invitation once one is open", () => {
        openFiles(HOLIDAY);

        const { container } = render(<Preview />);

        expect(container.querySelector("p")).toBeNull();
        expect(screen.queryByRole("button", { name: "Browse images" })).not.toBeInTheDocument();
        expect(screen.getAllByRole("img", { name: "Preview" })).not.toHaveLength(0);
    });
});

/**
 * What the canvas is made of, in both of its states.
 *
 * The surface is the canvas rather than something the empty canvas carries, so opening a photograph
 * must not change it - which is the half that would be easy to lose, since the two states render
 * entirely different children.
 */
describe("the canvas the preview is drawn on", () => {
    it("draws the chosen surface whether or not an image is open", () => {
        applyBackground("dotted");

        const { rerender } = render(<Preview />);
        expect(canvas()).toHaveClass("bg-[radial-gradient(var(--color-surface-dot)_1px,transparent_1px)]");

        openFiles(HOLIDAY);
        rerender(<Preview />);

        expect(canvas()).toHaveClass("bg-[radial-gradient(var(--color-surface-dot)_1px,transparent_1px)]");
        expect(field()).not.toBeInTheDocument();
    });

    it("draws the invitation over the particle field, not behind it", () => {
        applyBackground("particles");

        const { container } = render(<Preview />);

        expect(canvas()).toContainElement(field());
        // The field is `absolute inset-0`, so what the canvas has to say only stays legible because
        // its container is lifted over it - which is what the design's own `z-index:1` does.
        const lifted = container.querySelector(".z-10");
        expect(lifted).toContainElement(container.querySelector("p"));
        expect(lifted).not.toContainElement(field());
    });

    it("draws the photograph over the particle field too", () => {
        applyBackground("particles");
        openFiles(HOLIDAY);

        const { container } = render(<Preview />);

        expect(field()).toBeInTheDocument();
        const photograph = screen.getAllByRole("img", { name: "Preview" })[0] ?? null;
        expect(container.querySelector(".z-10")).toContainElement(photograph);
    });
});

/**
 * What the canvas draws while a drag is over it, and only while one is.
 *
 * The rectangle is drawn here rather than in either child because the canvas is what accepts a drop
 * in both of its states - the photograph and the invitation are what is *inside* the region, not
 * what decides whether there is one.
 */
describe("the region a dragged file will be taken into", () => {
    it("draws nothing while nothing is being dragged", () => {
        render(<Preview />);

        expect(shine()).not.toBeInTheDocument();
    });

    it("draws itself while a drag is over the canvas", async () => {
        render(<Preview />);

        await dragOnto(onCanvas);

        expect(shine()).toBeInTheDocument();
    });

    it("sweeps both of the design's colours around the border as one mark", async () => {
        render(<Preview />);

        await dragOnto(onCanvas);

        // One gradient carrying the pair, rather than four beams carrying one colour each: the
        // shine is the whole border at once, so the two arrive together and what travels is which
        // part of the gradient is over which edge. Order matters - it is the order they are drawn
        // in - which is why the list is asserted rather than its membership.
        expect(shine()?.style.getPropertyValue("--shine-colors")).toBe("var(--success-bright),var(--activity)");
    });

    it("sweeps at the pace the canvas was tuned to", async () => {
        render(<Preview />);

        await dragOnto(onCanvas);

        // The duration reaches the keyframe as a custom property rather than as a value baked into
        // a rule, which is what lets one `animate-shine` utility serve every caller. Five seconds
        // rather than upstream's fourteen: a drag that hovers for a second or two still has to show
        // movement, or the mark reads as a static gradient.
        expect(shine()?.style.getPropertyValue("--duration")).toBe("5s");
        expect(shine()).toHaveClass("motion-safe:animate-shine");
    });

    it("takes the region's own radius rather than a curve of its own", async () => {
        render(<Preview />);

        await dragOnto(onCanvas);

        // The beams this replaced could not: a rigid square travelling an `offset-path` had to be
        // given a corner scaled to its own length or it stalled on one, so the rectangle the user
        // saw was never quite the rectangle drawn. A mask has no such trouble - it inherits the
        // 12px and is the shape.
        expect(region()).toHaveClass("rounded-xl");
        expect(shine()).toHaveClass("rounded-[inherit]");
    });

    it("draws the shine and nothing under it", async () => {
        render(<Preview />);

        await dragOnto(onCanvas);

        // A `--border` track was drawn here first, under the mark. On a rectangle this size it read
        // as a grey box being drawn around the canvas rather than as the ground under a light, and
        // the shine needs none: it is over every edge already.
        expect(region()?.className).not.toMatch(/\bborder(-|$| )/);
        expect(shine()?.style.getPropertyValue("--border-width")).toBe("2px");
    });

    it("draws nothing for a drag over another region of the window", async () => {
        render(<Preview />);

        await dragOnto(onNavbar);

        expect(shine()).not.toBeInTheDocument();
    });

    it("stops drawing itself when the drag leaves", async () => {
        render(<Preview />);
        await dragOnto(onCanvas);

        await dragAway();

        expect(shine()).not.toBeInTheDocument();
    });

    it("stops drawing itself when the files are let go of", async () => {
        render(<Preview />);
        await dragOnto(onCanvas);

        await letGo(onCanvas);

        expect(shine()).not.toBeInTheDocument();
    });

    it("draws itself over a photograph exactly as it does over the invitation", async () => {
        openFiles(HOLIDAY);
        render(<Preview />);

        await dragOnto(onCanvas);

        // The canvas accepts a drop in both states, so it offers one in both. Over the photograph
        // rather than behind it: the content is lifted to `z-10`, and this is above that.
        expect(shine()).toBeInTheDocument();
        expect(screen.getAllByRole("img", { name: "Preview" })).not.toHaveLength(0);
        expect(region()).toHaveClass("z-20");
    });

    it("traces the canvas's own border rather than a rectangle inside it", async () => {
        render(<Preview />);

        await dragOnto(onCanvas);

        // What is drawn is the region that accepts the drop, and the region is this canvas - so the
        // mark is on its edges. Inset, it read as a second rectangle floating inside the canvas,
        // which is not a region that accepts anything. See design.md D5.
        expect(region()).toHaveClass("inset-x-0", "top-0");
        expect(canvas()).toContainElement(region());
    });

    it("is the rectangle the drop is hit-tested against, not a second one drawn to match", async () => {
        render(<Preview />);

        // The spec's promise is that what is offered and what is taken up are the same rectangle.
        // One element carries both, so it is mounted whether or not anything is being dragged -
        // there is no second element, and no pair of rules that could come to disagree.
        expect(region()).toBeInTheDocument();
        expect(shine()).not.toBeInTheDocument();
    });

    it("says nothing, to anyone", async () => {
        render(<Preview />);

        await dragOnto(onCanvas);

        // Decorative, and the empty canvas already says in words what the region means - which is
        // also why a drag, something a screen reader user is not performing, announces nothing.
        expect(region()).toHaveAttribute("aria-hidden", "true");
        expect(region()?.textContent).toBe("");
    });

    it("costs the pointer nothing, in the region and in the shine", async () => {
        render(<Preview />);

        await dragOnto(onCanvas);

        // The failure this change could introduce, and the reason it is asserted rather than trusted
        // to a class: this is a full-bleed element over the canvas that carries the drop itself and
        // the wheel-zoom and drag-to-pan handlers. One that intercepted a pointer event would break
        // the very drop it exists to advertise, silently.
        expect(region()).toHaveClass("pointer-events-none");
        expect(shine()).toHaveClass("pointer-events-none");
    });
});

/**
 * What an unfolded drawer does to the region, which is the question `useDroppedImages` left open
 * when the drawer could not yet cover the canvas.
 *
 * The canvas is inset by the drawer's folded 48px header permanently, and unfolding slides the
 * 128px body up over it rather than resizing anything - so the canvas's own bottom edge is in the
 * middle of an open drawer, and a region that used it would draw across the thumbnails and accept a
 * drop let go of on them.
 */
describe("the region while the drawer is unfolded", () => {
    beforeEach(() => {
        useDrawerStore.setState({ open: true });
    });

    it("stops at the top of the strip rather than at the canvas's bottom edge", () => {
        render(<Preview />);

        // The body's height, in the canvas's own coordinates: the header is already inset.
        expect(region()).toHaveStyle({ bottom: "128px" });
    });

    it("sits flush with the canvas's bottom edge again once the drawer is folded", () => {
        useDrawerStore.setState({ open: false });

        render(<Preview />);

        expect(region()).toHaveStyle({ bottom: "0px" });
    });

    it("draws nothing for a drag over the strip, and opens nothing let go of there", async () => {
        render(<Preview />);

        await dragOnto(onOpenDrawer, WITH_DRAWER);

        expect(shine()).not.toBeInTheDocument();

        await letGo(onOpenDrawer, HOLIDAY.path);

        // The same rectangle answers both, so a position that draws nothing cannot open anything:
        // the strip of thumbnails is not the canvas, however much of it the drawer is standing on.
        // The describe rather than the store alone: it is the call the drop path makes once it has
        // accepted, so it fails if the rectangle took the drop and Rust merely answered with nothing.
        expect(described).not.toHaveBeenCalled();
        expect(useFileStore.getState().files).toEqual([]);
    });

    it("still draws and still accepts above the strip", async () => {
        described.mockResolvedValueOnce([HOLIDAY]);
        render(<Preview />);

        await dragOnto(aboveDrawer, WITH_DRAWER);

        expect(shine()).toBeInTheDocument();

        await letGo(aboveDrawer, HOLIDAY.path);

        // The drawer shortens the region; it does not put the canvas out of use. What is above the
        // strip is still the rectangle that offers the drop, so it is still the one that takes it.
        expect(described).toHaveBeenCalledWith([HOLIDAY.path]);
        expect(useFileStore.getState().files).toEqual([HOLIDAY]);
    });
});
