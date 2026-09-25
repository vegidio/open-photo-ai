import { invoke } from "@tauri-apps/api/core";
import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import "@/i18n";
import { renditionUrl } from "@/ipc/images";
import { THUMBNAIL_BOUND } from "@/lib/constants";
import { useAutopilotStore } from "@/stores/autopilot";
import { useEnhancementStore } from "@/stores/enhancements";
import { useFileStore } from "@/stores/files";
import { type ImageViewport, useTransformStore } from "@/stores/transform";
import {
    CATALOGUE,
    EXPORT_FORMATS,
    frame,
    FRAMING,
    HOLIDAY,
    openFiles,
    render,
    resetCropStore,
    resetFileStore,
    SUNSET,
} from "@/test/support";
import { Sidebar } from "./Sidebar";

// `convertFileSrc` reads a global Tauri's init script installs, which jsdom has none of. The
// miniature's URL is asserted against `renditionUrl`'s own answer below rather than against a literal
// - the point of the assertion is that the sidebar and the canvas ask for the same thing. `invoke`
// is beside it because the add menu asks for the catalogue on mount; what it does with the answer is
// `AddEnhancement.test.tsx`'s subject. An Autopilot analysis answers that the photograph needs nothing: what
// the window does with an answer is `hooks/useAutopilot.test.tsx`'s subject, and here only the asking is.
vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
    invoke: vi.fn((command: string) =>
        Promise.resolve(command === "suggest" ? [] : command === "export_formats" ? EXPORT_FORMATS : CATALOGUE),
    ),
}));

// The export dialog subscribes to its progress event as it opens; what it does with a report is its own tests'.
vi.mock("@/ipc/export", async (importOriginal) => ({
    ...(await importOriginal<typeof import("@/ipc/export")>()),
    onExportProgress: vi.fn(() => Promise.resolve(() => {})),
}));

const invoked = invoke as unknown as Mock;

/** Whether an Autopilot analysis was asked for. */
const suggested = () => invoked.mock.calls.some(([command]) => command === "suggest");

describe("Sidebar", () => {
    beforeEach(() => {
        localStorage.clear();
        resetFileStore();
        resetCropStore();
        useTransformStore.setState(useTransformStore.getInitialState(), true);
        useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
        useAutopilotStore.setState(useAutopilotStore.getInitialState(), true);
    });

    it("says there is no preview rather than showing an empty panel", () => {
        render(<Sidebar />);

        expect(screen.getByText("No preview available")).toBeInTheDocument();
    });

    it("draws a miniature of the current image in place of those words", () => {
        openFiles(HOLIDAY);

        render(<Sidebar />);

        expect(screen.queryByText("No preview available")).not.toBeInTheDocument();
        expect(screen.getByRole("img", { name: "Zoom & Crop" })).toBeInTheDocument();
    });

    it("draws the image the canvas is drawing, at the strip's bound", () => {
        openFiles(HOLIDAY);

        render(<Sidebar />);

        // The drawer's own `THUMBNAIL_BOUND` rather than no bound - see `useCurrentRendition`. With
        // nothing framed it shares the strip's URL as well as its bound, so Rust has already produced
        // this rendition and the miniature costs no second render.
        expect(screen.getByRole("img", { name: "Zoom & Crop" })).toHaveAttribute(
            "src",
            renditionUrl(HOLIDAY.identity ?? "", THUMBNAIL_BOUND),
        );
    });

    it("draws the framing when the current image carries one", () => {
        openFiles(HOLIDAY);
        frame(HOLIDAY);

        render(<Sidebar />);

        // The strip stays uncropped, so a framed miniature is its own rendition at the same bound -
        // see `useCurrentRendition`.
        expect(screen.getByRole("img", { name: "Zoom & Crop" })).toHaveAttribute(
            "src",
            renditionUrl(HOLIDAY.identity ?? "", THUMBNAIL_BOUND, FRAMING),
        );
    });

    it("keeps the export control unavailable while nothing is picked", () => {
        openFiles(HOLIDAY, SUNSET);
        act(() => useFileStore.getState().unselectAll());
        render(<Sidebar />);

        expect(screen.getByRole("button", { name: "Export image" })).toBeDisabled();
    });

    it("offers to export the one picked image in the singular", () => {
        // A batch opened into an empty window picks its first file.
        openFiles(HOLIDAY, SUNSET);
        render(<Sidebar />);

        expect(screen.getByRole("button", { name: "Export image" })).toBeEnabled();
    });

    it("offers to export several picked images in the plural, and opens the dialog over them", () => {
        openFiles(HOLIDAY, SUNSET);
        act(() => useFileStore.getState().selectAll());
        render(<Sidebar />);

        fireEvent.click(screen.getByRole("button", { name: "Export images" }));

        expect(screen.getByRole("dialog", { name: "Export" })).toBeInTheDocument();
        expect(screen.getByText("2 files ready · nothing written yet")).toBeInTheDocument();
    });

    it("offers Add enhancement once there is an image to add one to", () => {
        render(<Sidebar />);
        expect(screen.getByRole("button", { name: "Add enhancement" })).toBeDisabled();

        resetFileStore();
        openFiles(HOLIDAY);
        render(<Sidebar />);

        expect(screen.getAllByRole("button", { name: "Add enhancement" })[1]).toBeEnabled();
    });

    it("toggles Autopilot even with nothing open", () => {
        render(<Sidebar />);

        const autopilot = screen.getByRole("switch", { name: "Autopilot" });
        expect(autopilot).toBeChecked();

        // `fireEvent` rather than `user-event`: a click is the whole interaction here, and the
        // richer library is a dependency nothing else here would use.
        fireEvent.click(autopilot);

        // The store, not just the switch: Autopilot is a persisted preference, and a switch that
        // moved without the store following it would come back on after a restart.
        expect(useEnhancementStore.getState().autopilot).toBe(false);
    });

    it("analyses a photograph opened with Autopilot on", () => {
        openFiles(HOLIDAY);

        render(<Sidebar />);

        expect(suggested()).toBe(true);
    });

    it("does not analyse a photograph opened with Autopilot off", () => {
        useEnhancementStore.setState({ autopilot: false });
        openFiles(HOLIDAY);

        render(<Sidebar />);

        expect(suggested()).toBe(false);
    });
});

/**
 * The miniature's own box, and the rectangle drawn over it.
 *
 * The two are one subject: the rectangle is positioned in percentages, and a percentage only means
 * something once the element it is a percentage of is the photograph rather than a fixed-height
 * block with the photograph letterboxed inside it.
 */
describe("the miniature's viewport rectangle", () => {
    const miniature = () => document.querySelector("[data-slot='sidebar-miniature']");
    const rectangle = () => document.querySelector("[data-slot='sidebar-viewport']");

    const show = (viewport: ImageViewport) => act(() => useTransformStore.getState().setViewport(viewport));

    // Its own, because a `beforeEach` belongs to the block it is written in: without this the
    // rectangle a previous case published, and the image it was published for, are both still there.
    beforeEach(() => {
        resetFileStore();
        useTransformStore.setState(useTransformStore.getInitialState(), true);
    });

    it("wraps the photograph in a box that hugs it, and only while one is open", () => {
        render(<Sidebar />);
        expect(miniature()).toBeNull();

        resetFileStore();
        openFiles(HOLIDAY);
        render(<Sidebar />);

        // The `<img>`'s own box is the photograph's box: bounded rather than stretched to the panel's
        // height, which is what makes a percentage over it a percentage of the photograph.
        expect(miniature()).toBeInTheDocument();
        expect(screen.getAllByRole("img", { name: "Zoom & Crop" })[0]).toHaveClass("max-h-36");
    });

    it("surrounds the whole miniature while the photograph is drawn whole", () => {
        openFiles(HOLIDAY);
        render(<Sidebar />);

        show({ x: 0, y: 0, width: 1, height: 1 });

        // The true answer rather than a special case: all of the photograph is being shown.
        expect(rectangle()).toHaveStyle({ left: "0%", top: "0%", width: "100%", height: "100%" });
    });

    it("is inset over a magnified photograph", () => {
        openFiles(HOLIDAY);
        render(<Sidebar />);

        show({ x: 0.25, y: 0.1, width: 0.5, height: 0.4 });

        // Which is how a user zoomed to 8x knows where in the frame they are.
        expect(rectangle()).toHaveStyle({ left: "25%", top: "10%", width: "50%", height: "40%" });
    });

    it("moves with what the canvas is showing", () => {
        openFiles(HOLIDAY);
        render(<Sidebar />);

        show({ x: 0.25, y: 0.1, width: 0.5, height: 0.4 });
        show({ x: 0.5, y: 0.1, width: 0.5, height: 0.4 });

        expect(rectangle()).toHaveStyle({ left: "50%" });
    });

    it("is absent while no image is open", () => {
        // Nothing has drawn, so nothing has published - and there is no miniature to draw over.
        render(<Sidebar />);

        expect(rectangle()).toBeNull();
    });
});
