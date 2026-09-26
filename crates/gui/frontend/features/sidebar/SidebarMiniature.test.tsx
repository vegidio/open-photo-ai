import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import type { Face } from "@/ipc/faces";
import { renditionUrl } from "@/ipc/images";
import { THUMBNAIL_BOUND } from "@/lib/constants";
import { useFacesStore } from "@/stores/faces";
import { type ImageViewport, useTransformStore } from "@/stores/transform";
import {
    FRAMING,
    frame,
    HOLIDAY,
    openFiles,
    render,
    resetCropStore,
    resetFileStore,
    resetSettingsStore,
    stubCanvas2D,
} from "@/test/support";
import { SidebarMiniature } from "./SidebarMiniature";

// `convertFileSrc` reads a global Tauri's init script installs, which jsdom has none of.
vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

describe("SidebarMiniature", () => {
    beforeEach(() => {
        localStorage.clear();
        resetFileStore();
        resetCropStore();
        resetSettingsStore();
        useTransformStore.setState(useTransformStore.getInitialState(), true);
        stubCanvas2D();
    });

    it("says there is no preview rather than showing an empty panel", () => {
        render(<SidebarMiniature />);

        expect(screen.getByText("No preview available")).toBeInTheDocument();
    });

    it("draws the current image at the strip's own bound", () => {
        openFiles(HOLIDAY);

        render(<SidebarMiniature />);

        expect(screen.getByRole("img", { name: "Zoom & Crop" })).toHaveAttribute(
            "src",
            renditionUrl(HOLIDAY.identity ?? "", THUMBNAIL_BOUND),
        );
    });

    it("draws the framed photograph where one is framed", () => {
        openFiles(HOLIDAY);
        frame(HOLIDAY);

        render(<SidebarMiniature />);

        expect(screen.getByRole("img", { name: "Zoom & Crop" })).toHaveAttribute(
            "src",
            renditionUrl(HOLIDAY.identity ?? "", THUMBNAIL_BOUND, FRAMING),
        );
    });

    it("still tracks which part of the photograph the canvas is on", () => {
        openFiles(HOLIDAY);

        render(<SidebarMiniature />);

        const viewport: ImageViewport = { x: 0.2, y: 0.1, width: 0.5, height: 0.6 };
        act(() => useTransformStore.getState().setViewport(viewport));

        // The rectangle is positioned in percentages of the `<img>`'s own box, which is the
        // photograph's box rather than the panel's.
        expect(document.querySelector("[data-slot='sidebar-viewport']")).toHaveStyle({
            left: "20%",
            top: "10%",
            width: "50%",
            height: "60%",
        });
    });

    describe("the way in to Crop/Rotate", () => {
        it("is drawn with the miniature", () => {
            openFiles(HOLIDAY);

            render(<SidebarMiniature />);

            expect(screen.getByRole("button", { name: "Crop/Rotate" })).toBeInTheDocument();
        });

        it("is absent along with it, which is the one control that is hidden rather than disabled", () => {
            render(<SidebarMiniature />);

            // The carve-out `gui-shell` records - see `SidebarMiniature`.
            expect(screen.queryByRole("button", { name: "Crop/Rotate" })).not.toBeInTheDocument();
        });

        it("opens the surface for the current image", () => {
            openFiles(HOLIDAY);

            render(<SidebarMiniature />);
            expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

            fireEvent.click(screen.getByRole("button", { name: "Crop/Rotate" }));

            expect(screen.getByRole("dialog", { name: "Crop/Rotate" })).toBeInTheDocument();
        });

        it("closes it again when the surface is dismissed", () => {
            openFiles(HOLIDAY);

            render(<SidebarMiniature />);
            fireEvent.click(screen.getByRole("button", { name: "Crop/Rotate" }));
            fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

            expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
        });
    });

    it("draws no face box over the miniature", () => {
        // The Select faces chooser is the only surface that draws them, and the miniature is the
        // third of the three the requirement names. `FaceBoxes` has one production consumer today,
        // so this catches a second being added here rather than pinning behaviour that varies.
        const identity = HOLIDAY.identity ?? "";
        const face: Face = {
            bounding_box: { min: { x: 500, y: 400 }, max: { x: 800, y: 700 } },
            landmarks: [
                { x: 600, y: 500 },
                { x: 700, y: 500 },
                { x: 650, y: 600 },
                { x: 600, y: 650 },
                { x: 700, y: 650 },
            ],
            confidence: 0.9,
            restorable: true,
            key: "500,400,800,700",
        };

        openFiles(HOLIDAY);
        act(() => {
            useFacesStore.getState().setFaces(identity, undefined, [face]);
            useFacesStore.getState().setFaceChoice(identity, { skipped: [face.key], restored: [] });
        });

        render(<SidebarMiniature />);

        expect(screen.getByRole("img")).toBeInTheDocument();
        expect(document.querySelectorAll("[data-slot='face-boxes']")).toHaveLength(0);

        useFacesStore.setState(useFacesStore.getInitialState(), true);
    });
});
