import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Preview } from "@/features/preview/Preview";
import "@/i18n";
import { ZOOM_BUTTON_STEP, ZOOM_MAX, ZOOM_MIN } from "@/lib/constants";
import { track } from "@/lib/faro";
import { useDrawerStore } from "@/stores/drawer";
import { useFileStore } from "@/stores/files";
import { usePreviewStore } from "@/stores/preview";
import { useTransformStore } from "@/stores/transform";
import { HOLIDAY, openFiles, render, resetFileStore, SUNSET, stubStripGeometry } from "@/test/support";
import { Drawer } from "./Drawer";

// Mocked at `lib/faro.ts`'s own boundary: what `track` does with an event is pinned by `faro.test.ts`.
vi.mock("@/lib/faro", () => ({
    track: vi.fn(),
    sendError: vi.fn(),
    pauseFaro: vi.fn(),
    // Untraced, as before Faro starts: the request goes out exactly as `invoke` alone would send it.
    traced: (_name: string, send: () => Promise<unknown>) => send(),
}));

// `convertFileSrc` reads a global Tauri's init script installs, which jsdom has none of. Needed
// because the cases below render the canvas beside the drawer: what the comparison group chooses is
// only meaningful as what the canvas then draws.
//
// `invoke` is mocked alongside it because closing an image talks to Rust: the enhancement store's
// per-file owner releases the enhanced result the backend is holding for the photograph that has gone.
// The cases below close images, and what the release asks for is `stores/enhancements.test.ts`'s
// subject rather than this file's.
vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
    invoke: vi.fn(() => Promise.resolve()),
}));

// And the webview's drag-drop channel, which `getCurrentWebview` reads another of those globals for:
// the canvas registers the drop listener on itself. What the listener does with a drop is
// `hooks/useDroppedImages.test.tsx`'s subject; here it only has to be registrable.
vi.mock("@tauri-apps/api/webview", () => ({
    getCurrentWebview: () => ({ onDragDropEvent: () => Promise.resolve(() => {}) }),
}));

// And the progress event, which `listen` reads a third of those globals for: a mounted canvas
// subscribes to it for as long as a file is open. What the listener does with a report is
// `hooks/useEnhancementRun.test.tsx`'s subject; here it only has to be registrable.
vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));

// jsdom has no Tauri OS plugin global, so `platform()` throws there. The file options menu on every
// thumbnail names the platform's file manager, which is what brings this into a drawer test.
vi.mock("@/ipc/os", () => ({ isMacOs: vi.fn(() => false), isWindows: vi.fn(() => false) }));

const renderEmpty = () => render(<Drawer />);

/** The drawer over the canvas, with one image open - which is what the shell renders. */
const renderWithImage = () => {
    openFiles(HOLIDAY);

    return render(
        <>
            <Preview />
            <Drawer />
        </>,
    );
};

const mode = (name: string) => screen.getByRole("radio", { name });

/**
 * The fold toggle, by its name.
 *
 * The name is the instruction rather than the state - Show images while the strip is hidden - so
 * asking for it by name is also asserting which of the two states the drawer is in.
 */
const fold = (name = "Show images") => screen.getByRole("button", { name });

/**
 * Select all, by either of the two labels it reads under.
 *
 * A regular expression rather than one of the two strings, because the label is itself under test:
 * the cases below assert what it says, and a query that already assumed the answer would fail on the
 * wrong line - or, worse, pass by finding nothing to compare.
 */
const selectAll = () => screen.getByLabelText(/select all/i);
const panes = () => screen.getAllByRole("img", { name: "Preview" });
const chips = () => [...document.querySelectorAll("[data-slot='preview-chip']")];

const selected = () => [...useFileStore.getState().selectedPaths];

/**
 * Drives the store from outside the tree, the way the strip's own checkboxes would from inside it.
 *
 * Wrapped in `act` because a bare `getState()` write is a zustand notification React has not been
 * told to expect: the subscribers re-render, but after the assertion has already read the old DOM.
 */
const write = (change: () => void) => act(change);

beforeEach(() => {
    resetFileStore();
    useDrawerStore.setState(useDrawerStore.getInitialState(), true);
    usePreviewStore.setState({ previewMode: "side" });

    // The strip is windowed, and jsdom lays nothing out: without a measurable scroll element the
    // virtualizer mounts no thumbnails at all, so every case about what the body draws would fail on
    // an empty container rather than on its subject.
    stubStripGeometry();
});

describe("Drawer", () => {
    it("disables every control that acts on an image", () => {
        renderEmpty();

        expect(fold()).toBeDisabled();
        expect(selectAll()).toBeDisabled();

        for (const mode of ["Full", "Side by Side", "Split"]) {
            expect(screen.getByRole("radio", { name: mode })).toBeDisabled();
        }

        // Radix marks the thumb, not the container, so this is the element that would actually
        // refuse a drag.
        expect(screen.getByRole("slider")).toHaveAttribute("data-disabled");
    });

    it("leaves Add images available, because it is how the empty state ends", () => {
        renderEmpty();

        expect(screen.getByRole("button", { name: "Add images" })).toBeEnabled();
    });

    it("leaves the comparison group unavailable while nothing is open", () => {
        renderEmpty();

        // And with no mode selected: the tray does not claim a comparison for an image that does not
        // exist, which is what the Wails app does with the same control. Read through `aria-checked`
        // rather than `data-state`, which on these items belongs to the tooltip wrapped around them.
        for (const name of ["Full", "Side by Side", "Split"]) {
            expect(mode(name)).toBeDisabled();
            expect(mode(name)).not.toBeChecked();
        }
    });

    it("starts folded, with its header still on screen", () => {
        const { container } = renderEmpty();

        // Translated down by exactly the body's height, so the 48px header stays flush with the
        // bottom edge.
        expect(container.firstElementChild).toHaveStyle({ transform: "translateY(128px)" });
        expect(fold()).toBeInTheDocument();
    });
});

describe("the comparison group, with an image open", () => {
    it("reports the comparison the store came up in", () => {
        usePreviewStore.setState({ previewMode: "split" });
        renderWithImage();

        expect(mode("Split")).toBeChecked();
        expect(mode("Full")).not.toBeChecked();
    });

    it("paints the chosen comparison off `aria-checked`, which the tooltip cannot overwrite", () => {
        usePreviewStore.setState({ previewMode: "split" });
        renderWithImage();

        const chosen = mode("Split");

        // The trap this pins, which `DrawerHeader` describes: the tooltip's `data-state="closed"` lands
        // on top of the toggle's "on", so every `data-[state=on]:*` rule the generated toggle ships with
        // is dead here. Styling has to hang off `aria-checked`, and this asserts both halves - that the
        // attribute is the true one, and that the other is not.
        expect(chosen).toHaveAttribute("aria-checked", "true");
        expect(chosen).not.toHaveAttribute("data-state", "on");
        expect(chosen).toHaveClass("aria-checked:bg-input");
    });

    it("changes what the canvas draws", () => {
        renderWithImage();

        // Side by side to begin with: two panes, both labelled.
        expect(panes()).toHaveLength(2);

        fireEvent.click(mode("Full"));

        // Full draws the enhanced image alone, which is the whole of what choosing it means: one
        // pane, and therefore one chip. Not asserted on what that chip *says* - this image carries
        // no enhancements, so the one pane draws the source and is labelled Original like any pane
        // drawing it. What each chip reads under is `PreviewImage.test.tsx`'s subject.
        expect(panes()).toHaveLength(1);
        expect(chips()).toHaveLength(1);

        fireEvent.click(mode("Side by Side"));
        expect(panes()).toHaveLength(2);

        expect(vi.mocked(track).mock.calls).toEqual([
            ["preview_mode_changed", { mode: "full" }],
            ["preview_mode_changed", { mode: "side" }],
        ]);
    });

    it("keeps the comparison chosen when it is pressed again", () => {
        renderWithImage();

        // Radix reports a deselection as an empty string; a canvas with no comparison at all is not
        // one of the three states this application has.
        fireEvent.click(mode("Side by Side"));

        expect(usePreviewStore.getState().previewMode).toBe("side");
        expect(panes()).toHaveLength(2);
        expect(track).not.toHaveBeenCalled();
    });

    it("leaves nothing in the header drawn and dead", () => {
        renderWithImage();

        expect(screen.getByRole("slider", { name: "Zoom" })).not.toHaveAttribute("data-disabled");

        expect(fold()).toBeEnabled();
        expect(selectAll()).toBeEnabled();
        expect(screen.getByRole("button", { name: "Zoom out" })).toBeEnabled();
        expect(screen.getByRole("button", { name: "Zoom in" })).toBeEnabled();

        for (const name of ["Full", "Side by Side", "Split"]) {
            expect(mode(name)).toBeEnabled();
        }
    });

    it("still offers Add images, which was never gated on there being one", () => {
        renderWithImage();

        expect(screen.getByRole("button", { name: "Add images" })).toBeEnabled();
    });
});

describe("the fold toggle", () => {
    it("says what it will do and draws which state the drawer is in", () => {
        renderWithImage();

        // Folded: the instruction is Show images and the chevrons point up, out of the header.
        expect(fold()).toBeInTheDocument();
        expect(fold().querySelector("svg")).toHaveClass("lucide-chevrons-up");

        fireEvent.click(fold());

        expect(fold("Hide images")).toBeInTheDocument();
        expect(fold("Hide images").querySelector("svg")).toHaveClass("lucide-chevrons-down");
    });

    it("unfolds the drawer and folds it again", () => {
        renderWithImage();

        fireEvent.click(fold());
        expect(useDrawerStore.getState().open).toBe(true);

        fireEvent.click(fold("Hide images"));
        expect(useDrawerStore.getState().open).toBe(false);
    });

    it("stays unavailable while no image is open", () => {
        renderEmpty();

        fireEvent.click(fold());

        expect(fold()).toBeDisabled();
        expect(useDrawerStore.getState().open).toBe(false);
    });
});

describe("Select all", () => {
    /** Two images open, with the first picked - which is what opening a batch into an empty window does. */
    const renderTwo = () => {
        openFiles(HOLIDAY, SUNSET);

        return render(<Drawer />);
    };

    it("reports nothing picked, and offers to select all", () => {
        renderTwo();
        write(() => useFileStore.getState().unselectAll());

        expect(selectAll()).not.toBeChecked();
        expect(selectAll()).toHaveAttribute("data-state", "unchecked");
        expect(screen.getByText("Select all")).toBeInTheDocument();
    });

    it("reports a partial selection", () => {
        renderTwo();

        // Radix's own third state, so it is announced as mixed rather than as an unticked box.
        expect(selectAll()).toHaveAttribute("aria-checked", "mixed");
        expect(selectAll()).toHaveAttribute("data-state", "indeterminate");
        expect(screen.getByText("Select all")).toBeInTheDocument();
    });

    it("reports every image picked, and offers to unselect them", () => {
        renderTwo();
        write(() => useFileStore.getState().selectAll());

        expect(selectAll()).toBeChecked();
        expect(screen.getByText("Unselect all")).toBeInTheDocument();
    });

    it("picks everything from none and from some alike", () => {
        renderTwo();
        write(() => useFileStore.getState().unselectAll());

        fireEvent.click(selectAll());
        expect(selected().sort()).toEqual([HOLIDAY.path, SUNSET.path].sort());

        write(() => useFileStore.getState().unselectAll());
        write(() => useFileStore.getState().toggleSelected(HOLIDAY.path));

        fireEvent.click(selectAll());
        expect(selected().sort()).toEqual([HOLIDAY.path, SUNSET.path].sort());
    });

    it("unpicks everything once everything is picked", () => {
        renderTwo();
        write(() => useFileStore.getState().selectAll());

        fireEvent.click(selectAll());

        expect(selected()).toEqual([]);
    });

    it("stays unavailable while no image is open", () => {
        renderEmpty();

        expect(selectAll()).toBeDisabled();
        expect(screen.getByText("Select all")).toBeInTheDocument();
    });
});

describe("the fold itself", () => {
    /** The drawer with two images open, which is the state the strip has something to draw in. */
    const renderUnfolded = () => {
        openFiles(HOLIDAY, SUNSET);

        return render(<Drawer />);
    };

    /** The drawer alone with one image open, so the rendered root is the drawer's own element. */
    const renderFolded = () => {
        openFiles(HOLIDAY);

        return render(<Drawer />);
    };

    it("translates the body out of view when folded and into place when unfolded", () => {
        const { container } = renderFolded();

        // One image, so nothing has unfolded it: the body is pushed down by exactly its own height
        // and the 48px header is what is left on screen.
        expect(container.firstElementChild).toHaveStyle({ transform: "translateY(128px)" });

        fireEvent.click(fold());

        expect(container.firstElementChild).not.toHaveStyle({ transform: "translateY(128px)" });
    });

    it("draws the strip of open images in the body", () => {
        renderUnfolded();

        expect(screen.getByRole("button", { name: "holiday.png" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "sunset over the harbour.nef" })).toBeInTheDocument();
    });

    it("folds when the canvas outside it is clicked", () => {
        renderUnfolded();
        expect(useDrawerStore.getState().open).toBe(true);

        // `pointerDown` rather than `click`, which is the event the drawer listens for: a press that
        // begins outside it is the honest answer to "did the user reach past the drawer".
        fireEvent.pointerDown(document.body);

        expect(useDrawerStore.getState().open).toBe(false);
    });

    it("stays unfolded when one of its own thumbnails is clicked", () => {
        renderUnfolded();

        fireEvent.pointerDown(screen.getByRole("button", { name: "sunset over the harbour.nef" }));

        expect(useDrawerStore.getState().open).toBe(true);
    });

    it("folds when every open image is closed", async () => {
        renderUnfolded();
        expect(useDrawerStore.getState().open).toBe(true);

        fireEvent.keyDown(screen.getByRole("button", { name: "File options for holiday.png" }), { key: "Enter" });
        fireEvent.click(await screen.findByRole("menuitem", { name: "Close all images" }));

        await waitFor(() => expect(useDrawerStore.getState().open).toBe(false));
    });

    it("folds when the last remaining image is closed singly", async () => {
        // The other route to an empty strip, and the reason this is keyed on the count rather than
        // written into the action that emptied it.
        openFiles(HOLIDAY);
        useDrawerStore.getState().setOpen(true);
        render(<Drawer />);

        fireEvent.keyDown(screen.getByRole("button", { name: "File options for holiday.png" }), { key: "Enter" });
        fireEvent.click(await screen.findByRole("menuitem", { name: "Close image" }));

        await waitFor(() => expect(useDrawerStore.getState().open).toBe(false));
    });

    it("stays unfolded when one of several images is closed", async () => {
        // Closing is not the same as emptying: there is still a strip with something to say.
        renderUnfolded();

        fireEvent.keyDown(screen.getByRole("button", { name: "File options for holiday.png" }), { key: "Enter" });
        fireEvent.click(await screen.findByRole("menuitem", { name: "Close image" }));

        await waitFor(() => expect(useFileStore.getState().files).toHaveLength(1));
        expect(useDrawerStore.getState().open).toBe(true);
    });

    it("stays unfolded while a thumbnail's file options menu is being operated", async () => {
        // The problem the portal into the drawer exists to solve - see `Drawer`: a menu portalled to the
        // body would fold the drawer out from under the user's hand.
        renderUnfolded();

        fireEvent.keyDown(screen.getByRole("button", { name: "File options for sunset over the harbour.nef" }), {
            key: "Enter",
        });

        const item = await screen.findByRole("menuitem", { name: "Close image" });
        fireEvent.pointerDown(item);

        expect(useDrawerStore.getState().open).toBe(true);
    });

    it("draws that menu inside itself rather than on the body", () => {
        // What makes the case above true rather than incidental. Asserted on where the menu landed,
        // because a menu that happened not to fold the drawer for some other reason would pass the
        // case above and break the next control that portals out of the strip.
        const { container } = renderUnfolded();

        fireEvent.keyDown(screen.getByRole("button", { name: "File options for holiday.png" }), { key: "Enter" });

        const menu = screen.getByRole("menu");
        expect(container.firstElementChild?.contains(menu)).toBe(true);
    });

    it("hands over from one thumbnail's menu to another's in a single press", async () => {
        // Radix's `modal` default would disable pointer events outside the open menu, so the press on
        // the second control would never reach it: the first menu stays up and the second never
        // opens. `FileOptionsMenu` turns that off - see the comment on its `modal`.
        renderUnfolded();

        fireEvent.keyDown(screen.getByRole("button", { name: "File options for holiday.png" }), { key: "Enter" });
        expect(await screen.findByRole("menuitem", { name: "Close image" })).toBeInTheDocument();

        fireEvent.keyDown(screen.getByRole("button", { name: "File options for sunset over the harbour.nef" }), {
            key: "Enter",
        });

        // Exactly one menu, and it is the second control's: two open at once is the bug this pins.
        await waitFor(() => expect(screen.getAllByRole("menu")).toHaveLength(1));
    });

    it("still folds when the canvas is pressed with that menu open", () => {
        // The other half of the rule: a menu the drawer opened is part of the drawer, and everything
        // else is still outside it.
        renderUnfolded();

        fireEvent.keyDown(screen.getByRole("button", { name: "File options for holiday.png" }), { key: "Enter" });
        fireEvent.pointerDown(document.body);

        expect(useDrawerStore.getState().open).toBe(false);
    });

    it("stays unfolded when one of its own controls is used", () => {
        renderUnfolded();

        fireEvent.pointerDown(selectAll());

        expect(useDrawerStore.getState().open).toBe(true);
    });

    it("listens for a click away only while it is unfolded", () => {
        const add = vi.spyOn(document, "addEventListener");
        renderFolded();

        // Folded, and costing the document nothing: the listener is registered by the effect that
        // the open state guards, so a window that never unfolds the drawer never installs one.
        expect(add).not.toHaveBeenCalledWith("pointerdown", expect.anything());

        fireEvent.click(fold());

        expect(add).toHaveBeenCalledWith("pointerdown", expect.anything());
    });
});

describe("the drawer unfolding itself", () => {
    /** Mounted with nothing open, so every batch below arrives at a drawer that is already on screen. */
    const renderEmptyWindow = () => render(<Drawer />);

    const open = () => useDrawerStore.getState().open;

    it("stays folded for a single image", () => {
        renderEmptyWindow();

        write(() => openFiles(HOLIDAY));

        // With one image open the strip says nothing the rest of the window is not already saying.
        expect(open()).toBe(false);
    });

    it("unfolds when a second image arrives", () => {
        renderEmptyWindow();
        write(() => openFiles(HOLIDAY));

        write(() => openFiles(SUNSET));

        // With two, the strip is the only place the second one is visible at all.
        expect(open()).toBe(true);
    });

    it("unfolds for a batch that opens several at once", () => {
        renderEmptyWindow();

        write(() => openFiles(HOLIDAY, SUNSET));

        expect(open()).toBe(true);
    });

    it("unfolds again when further images arrive after the user folded it", () => {
        renderEmptyWindow();
        write(() => openFiles(HOLIDAY, SUNSET));

        fireEvent.click(fold("Hide images"));
        expect(open()).toBe(false);

        write(() =>
            openFiles({ ...HOLIDAY, path: "/Users/someone/Pictures/harbour.png", identity: "aaaabbbbccccdddd" }),
        );

        // Arriving images are the more recent statement of what the user wants to see, which is what
        // the reference does: its effect is keyed on the count and opens on every change above one.
        expect(open()).toBe(true);
    });

    it("stays folded for a batch that adds nothing", () => {
        renderEmptyWindow();
        write(() => openFiles(HOLIDAY, SUNSET));

        fireEvent.click(fold("Hide images"));

        write(() => openFiles(HOLIDAY, SUNSET));

        // Keyed on the count rather than on the batch, so re-opening the same folder does not reopen
        // a drawer the user has just put away.
        expect(open()).toBe(false);
    });
});

/**
 * The zoom control: a slider that reports as well as sets, and a step button either side of it.
 *
 * Read through the store rather than off the canvas, because what this control acts on is the
 * current image's transform - the canvas is one of three things that then draw from it, and the
 * sidebar's rectangle is another.
 */
describe("the zoom control", () => {
    const slider = () => screen.getByRole("slider", { name: "Zoom" });
    const zoomOut = () => screen.getByRole("button", { name: "Zoom out" });
    const zoomIn = () => screen.getByRole("button", { name: "Zoom in" });

    const scaleOf = (record: typeof HOLIDAY) =>
        useTransformStore.getState().transforms.get(record.identity ?? "")?.scale;

    const magnify = (record: typeof HOLIDAY, scale: number) =>
        act(() => useTransformStore.getState().setTransform(record.identity ?? "", { scale, x: 0, y: 0 }));

    beforeEach(() => useTransformStore.setState(useTransformStore.getInitialState(), true));

    it("reports the current image's magnification", () => {
        openFiles(HOLIDAY);
        magnify(HOLIDAY, 3);

        render(<Drawer />);

        expect(slider()).toHaveAttribute("aria-valuenow", "3");
    });

    it("follows a magnification made by any other route", () => {
        openFiles(HOLIDAY);
        render(<Drawer />);

        expect(slider()).toHaveAttribute("aria-valuenow", "1");

        // The wheel over the canvas writes the same slot, so the thumb has to move with it - a
        // control that only ever set the value would sit at 1x over an image magnified to 8.
        magnify(HOLIDAY, 4.5);

        expect(slider()).toHaveAttribute("aria-valuenow", "4.5");
    });

    it("reports the magnification of the image the canvas is drawing, not the one just left", () => {
        openFiles(HOLIDAY, SUNSET);
        magnify(HOLIDAY, 6);

        render(<Drawer />);
        expect(slider()).toHaveAttribute("aria-valuenow", "6");

        act(() => useFileStore.getState().setCurrentIndex(1));

        // The second photograph has never been magnified, so it reads fitted - which is the whole of
        // what keeping a transform per photograph buys.
        expect(slider()).toHaveAttribute("aria-valuenow", "1");
    });

    it("steps the current image down and up", () => {
        openFiles(HOLIDAY);
        magnify(HOLIDAY, 3);

        render(<Drawer />);

        fireEvent.click(zoomOut());
        expect(scaleOf(HOLIDAY)).toBe(3 - ZOOM_BUTTON_STEP);

        fireEvent.click(zoomIn());
        fireEvent.click(zoomIn());
        expect(scaleOf(HOLIDAY)).toBe(3 + ZOOM_BUTTON_STEP);
    });

    it("holds at each end rather than refusing the step", () => {
        openFiles(HOLIDAY);
        magnify(HOLIDAY, ZOOM_MAX);

        render(<Drawer />);

        fireEvent.click(zoomIn());
        expect(scaleOf(HOLIDAY)).toBe(ZOOM_MAX);

        magnify(HOLIDAY, ZOOM_MIN);
        fireEvent.click(zoomOut());
        expect(scaleOf(HOLIDAY)).toBe(ZOOM_MIN);
    });

    it("leaves the position alone, because it is not pointed at any part of the photograph", () => {
        openFiles(HOLIDAY);
        act(() => useTransformStore.getState().setTransform(HOLIDAY.identity ?? "", { scale: 2, x: -120, y: -40 }));

        render(<Drawer />);
        fireEvent.click(zoomIn());

        // No anchor either: the canvas holds the middle of the pane still for a control that names
        // no point of the photograph.
        expect(useTransformStore.getState().transforms.get(HOLIDAY.identity ?? "")).toEqual({
            scale: 2 + ZOOM_BUTTON_STEP,
            x: -120,
            y: -40,
        });
    });

    it("stays unavailable while no image is open", () => {
        renderEmpty();

        expect(slider()).toHaveAttribute("data-disabled");
        expect(zoomOut()).toBeDisabled();
        expect(zoomIn()).toBeDisabled();
    });

    /**
     * The bubble over the thumb: the only place this control says what magnification it is sitting at.
     *
     * `pointerOver` and `pointerOut` rather than `pointerEnter` and `pointerLeave`, because React
     * synthesises the enter/leave pair from the over/out pair - firing the synthetic names directly
     * reaches no handler, and the cases below would pass against a component that had none.
     */
    describe("the value bubble", () => {
        const bubble = () => screen.queryByRole("tooltip");

        it("says what magnification the thumb is sitting at while the pointer rests on it", () => {
            openFiles(HOLIDAY);
            magnify(HOLIDAY, 2.5);

            render(<Drawer />);
            expect(bubble()).not.toBeInTheDocument();

            fireEvent.pointerOver(slider());
            expect(bubble()).toHaveTextContent("2.5x");

            fireEvent.pointerOut(slider());
            expect(bubble()).not.toBeInTheDocument();
        });

        it("trims a trailing zero, so a whole magnification reads 4x rather than 4.0x", () => {
            openFiles(HOLIDAY);

            // 4.000000000000001 is what a run of wheel notches actually leaves in the store - the
            // wheel steps by 0.05 - so the rounding is load-bearing, not cosmetic.
            magnify(HOLIDAY, 4.000000000000001);

            render(<Drawer />);
            fireEvent.pointerOver(slider());

            expect(bubble()).toHaveTextContent("4x");
        });

        it("keeps narrating through a drag, after the pointer has slipped off the thumb", () => {
            openFiles(HOLIDAY);
            render(<Drawer />);

            fireEvent.pointerOver(slider());
            fireEvent.pointerDown(slider());

            // The thumb is 16px across and a drag is a hand movement, so the pointer leaves it
            // constantly. Radix's own tooltip behaviour closes on the press and will not reopen until
            // the pointer has left and come back, which is why this control drives `open` itself.
            fireEvent.pointerOut(slider());
            magnify(HOLIDAY, 3.2);

            expect(bubble()).toHaveTextContent("3.2x");

            // On the document, because that is where a drag that started on the thumb ends.
            fireEvent.pointerUp(document);
            expect(bubble()).not.toBeInTheDocument();
        });

        it("narrates a magnification made from the keyboard", () => {
            openFiles(HOLIDAY);
            render(<Drawer />);

            act(() => slider().focus());

            // Focus alone is not the cue: a mouse drag leaves the thumb focused, and a bubble that
            // opened on focus would then hang over the header until something else was clicked.
            expect(bubble()).not.toBeInTheDocument();

            fireEvent.keyDown(slider(), { key: "ArrowRight" });
            expect(bubble()).toHaveTextContent("1.1x");

            fireEvent.blur(slider());
            expect(bubble()).not.toBeInTheDocument();
        });

        it("stays silent while no image is open", () => {
            renderEmpty();

            // A disabled Radix slider still receives pointer events - the thumb is a `<span>`, which
            // no `:disabled` rule ever matches - so without an explicit guard a greyed-out control
            // would still narrate 1x under the pointer.
            fireEvent.pointerOver(slider());

            expect(bubble()).not.toBeInTheDocument();
        });
    });
});
