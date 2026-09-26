import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import type { Face } from "@/ipc/faces";
import { useCropStore } from "@/stores/crop";
import { useFacesStore } from "@/stores/faces";
import { FRAMING, HOLIDAY, openFiles, render, resetFileStore } from "@/test/support";
import { pictureFit, SelectFacesDialog } from "./SelectFacesDialog";

vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

const identity = HOLIDAY.identity ?? "";

const face = (left: number): Face => ({
    bounding_box: { min: { x: left, y: 4 }, max: { x: left + 3, y: 7 } },
    landmarks: [
        { x: left + 1, y: 5 },
        { x: left + 2, y: 5 },
        { x: left + 1.5, y: 6 },
        { x: left + 1, y: 6.5 },
        { x: left + 2, y: 6.5 },
    ],
    confidence: 0.9,
    restorable: true,
    key: `${left},4,${left + 3},7`,
});

const FIRST = face(0);
const SECOND = face(20);

const closed = vi.fn();

const found = (faces: Face[], crop?: typeof FRAMING) => useFacesStore.getState().setFaces(identity, crop, faces);

const mount = () => render(<SelectFacesDialog open onClose={closed} />);

const box = (number: number) => screen.getByRole("button", { name: `Toggle face ${number}` });

const applyButton = () => screen.getByRole("button", { name: /Apply to/ });

/** What the store holds for the photograph, which only Apply is allowed to change. */
const stored = () => useFacesStore.getState().choices.get(identity);

const picture = () => document.querySelector("[data-slot='faces-picture']") as HTMLElement;

const dialog = () => document.querySelector("[data-slot='dialog-content']") as HTMLElement;

beforeEach(() => {
    vi.clearAllMocks();
    resetFileStore();
    useFacesStore.setState(useFacesStore.getInitialState(), true);
    useCropStore.setState(useCropStore.getInitialState(), true);
    openFiles(HOLIDAY);
});

describe("the Select faces dialog", () => {
    it("names itself through its title bar", async () => {
        found([FIRST, SECOND]);
        mount();

        expect(await screen.findByRole("dialog", { name: "Select faces" })).toBeInTheDocument();
    });

    it("draws the photograph bounded, with a box over each face found", () => {
        found([FIRST, SECOND]);
        mount();

        expect(screen.getByRole("img")).toHaveAttribute("src", `opai://localhost/${identity}?size=2048`);
        expect(box(1)).toBeInTheDocument();
        expect(box(2)).toBeInTheDocument();
    });

    it("asks for the framed photograph, carrying both the bound and the framing", () => {
        // The faces are in the framed photograph's pixels, so a chooser drawing the unframed file
        // would put every box in the wrong place and one the crop removed over nothing at all.
        useCropStore.getState().setCrop(identity, FRAMING);
        found([FIRST], FRAMING);
        mount();

        expect(screen.getByRole("img")).toHaveAttribute(
            "src",
            `opai://localhost/${identity}?size=2048&crop=100,50,1200,1600,-1500,h`,
        );
    });

    it("draws the picture box at the photograph's own proportions", () => {
        found([FIRST]);
        mount();

        expect(picture().style.aspectRatio).toBe("3000 / 2000");
    });

    it("sizes both the picture and the dialog from the window", () => {
        // A browser and jsdom serialize a math expression differently, so what is asserted here is
        // that both were applied; `pictureFit` is where the arithmetic itself is pinned.
        found([FIRST]);
        mount();

        expect(picture().style.height).toContain("100vh");
        expect(picture().style.height).toContain("100vw");
        expect(dialog().style.width).toContain("100vw");
        expect(dialog().style.width).toContain("100vh");
    });

    it("falls back to what the rendition decoded to for a photograph that could not be measured", () => {
        // The file decoded but its header did not parse, and there is no framing to answer with
        // either. The loaded picture's own size is exact whenever it is under the dialog's bound.
        resetFileStore();
        openFiles({ path: HOLIDAY.path, identity, extension: "png", size: 1 });
        found([FIRST]);
        mount();

        expect(screen.queryByRole("button", { name: "Toggle face 1" })).not.toBeInTheDocument();

        const img = screen.getByRole("img");
        Object.defineProperty(img, "naturalWidth", { value: 30 });
        Object.defineProperty(img, "naturalHeight", { value: 20 });
        fireEvent.load(img);

        expect(box(1).style).toMatchObject({ left: "0%", width: "10%" });
    });

    it("draws the picture box at the framed proportions for a framed photograph", () => {
        useCropStore.getState().setCrop(identity, FRAMING);
        found([FIRST], FRAMING);
        mount();

        expect(picture().style.aspectRatio).toBe("1200 / 1600");
    });

    it("reports how many of the faces found will be restored", () => {
        found([FIRST, SECOND]);
        mount();

        expect(applyButton()).toHaveTextContent("Apply to 2 of 2 faces");

        fireEvent.click(box(1));

        expect(applyButton()).toHaveTextContent("Apply to 1 of 2 faces");
    });

    it("greys a box as it is skipped and leaves the store alone", () => {
        found([FIRST, SECOND]);
        mount();

        fireEvent.click(box(1));

        expect(box(1)).toHaveAttribute("aria-pressed", "false");
        expect(stored()).toBeUndefined();
    });

    it("commits the choice when it is applied, and closes", () => {
        found([FIRST, SECOND]);
        mount();

        fireEvent.click(box(2));
        fireEvent.click(applyButton());

        expect(stored()).toEqual({ skipped: [SECOND.key], restored: [] });
        expect(closed).toHaveBeenCalledOnce();
    });

    it("discards the choice when the close box is used", async () => {
        found([FIRST, SECOND]);
        mount();

        fireEvent.click(box(1));
        fireEvent.click(screen.getByRole("button", { name: "Close" }));

        await waitFor(() => expect(closed).toHaveBeenCalledOnce());
        expect(stored()).toBeUndefined();
    });

    it("discards the choice when the keyboard's cancel is used", async () => {
        found([FIRST, SECOND]);
        mount();

        fireEvent.click(box(1));
        fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });

        await waitFor(() => expect(closed).toHaveBeenCalledOnce());
        expect(stored()).toBeUndefined();
    });

    it("discards the choice when something outside it is acted on", async () => {
        found([FIRST, SECOND]);
        mount();

        fireEvent.click(box(1));

        // Radix registers the outside-press listener a tick after the layer mounts, so the press has
        // to come after that rather than in the same turn as the render.
        await waitFor(() => expect(box(1)).toHaveAttribute("aria-pressed", "false"));

        fireEvent.pointerDown(document.body);
        fireEvent.click(document.body);

        await waitFor(() => expect(closed).toHaveBeenCalledOnce());
        expect(stored()).toBeUndefined();
    });

    it("says how a box is used", () => {
        found([FIRST]);
        mount();

        expect(screen.getByText("Click a box to include or skip a face.")).toBeInTheDocument();
    });
});

describe("how the picture is fitted to the window", () => {
    // The chrome, added up once here as the component adds it up once there: 32px of inset at the top
    // and 32 at the bottom, a border on each, the 48px title bar and its rule, the 52px footer and its
    // rule, and the 2px the picture is padded by - and along the width, the two insets, the two
    // borders and the two pads.
    const VERTICAL = 172;
    const HORIZONTAL = 70;
    const FRAME = 6;

    it("takes the smaller of the two edges, in both of them", () => {
        // Whichever edge binds, binds in both - which is what keeps the picture the photograph's own
        // shape at whatever size that leaves.
        expect(pictureFit(3000, 2000)).toEqual({
            picture: `min(100vh - ${VERTICAL}px, (100vw - ${HORIZONTAL}px) * 2000 / 3000)`,
            dialog: `calc(min(100vw - ${HORIZONTAL}px, (100vh - ${VERTICAL}px) * 3000 / 2000) + ${FRAME}px)`,
        });
    });

    it("turns the proportions over for a portrait photograph", () => {
        expect(pictureFit(2000, 3000).picture).toBe(
            `min(100vh - ${VERTICAL}px, (100vw - ${HORIZONTAL}px) * 3000 / 2000)`,
        );
    });

    it("gives the dialog the picture's width plus its own frame", () => {
        // The border on each side and the 2px the picture is padded by: the dialog is exactly as wide
        // as the picture it holds, so nothing else can decide its width.
        expect(pictureFit(2000, 2000).dialog).toBe(
            `calc(min(100vw - ${HORIZONTAL}px, (100vh - ${VERTICAL}px) * 2000 / 2000) + ${FRAME}px)`,
        );
    });

    it("is written in viewport units in both terms, so neither waits on the other to lay out", () => {
        // The circle this avoids: a picture filling the dialog's height while the dialog takes its
        // width from the picture is resolved by a browser dropping the aspect - which draws the
        // photograph letterboxed with every face box standing over the letterbox.
        const { picture, dialog } = pictureFit(4000, 1000);

        for (const value of [picture, dialog]) {
            expect(value).toContain("100vh");
            expect(value).toContain("100vw");
            expect(value).not.toContain("%");
        }
    });
});
