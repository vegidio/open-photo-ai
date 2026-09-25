import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import { useCropStore } from "@/stores/crop";
import { useTransformStore } from "@/stores/transform";
import {
    applyBackground,
    HOLIDAY,
    openFiles,
    render,
    resetCropStore,
    resetFileStore,
    resetSettingsStore,
    stubCanvas2D,
} from "@/test/support";
import { CropRotate } from "./CropRotate";

// `convertFileSrc` reads a global Tauri's init script installs, which jsdom has none of.
vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

describe("CropRotate", () => {
    beforeEach(() => {
        localStorage.clear();
        resetFileStore();
        resetCropStore();
        resetSettingsStore();
        useTransformStore.setState(useTransformStore.getInitialState(), true);
        stubCanvas2D();
        openFiles(HOLIDAY);
    });

    it("draws every control screen 18 draws", () => {
        render(<CropRotate open onClose={vi.fn()} />);

        // Nothing on this surface is inert.
        expect(screen.getByRole("button", { name: "Free" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "9:16" })).toBeInTheDocument();
        expect(screen.getByRole("textbox", { name: "w" })).toBeInTheDocument();
        expect(screen.getByRole("textbox", { name: "h" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Swap width and height" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Rotate 90 degrees" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Flip horizontally" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Flip vertically" })).toBeInTheDocument();
        expect(screen.getByRole("slider")).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Reset" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Apply" })).toBeInTheDocument();
    });

    it("says the photograph can be magnified, since nothing else on it does", () => {
        render(<CropRotate open onClose={vi.fn()} />);

        expect(screen.getByText(/mouse wheel or the trackpad pinch/)).toBeInTheDocument();
    });

    it("is named by its title bar rather than announced as an unnamed dialog", () => {
        render(<CropRotate open onClose={vi.fn()} />);

        // Through `DialogTitleBar`, which renders a `DialogTitle` - so the visible title and the
        // announced one are the same string by construction rather than by a matching `aria-label`
        // somebody has to remember to update.
        expect(screen.getByRole("dialog", { name: "Crop/Rotate" })).toBeInTheDocument();
    });

    it("draws nothing at all while it is closed", () => {
        render(<CropRotate open={false} onClose={vi.fn()} />);

        expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    describe("the way out", () => {
        it("stays open when the area outside it is clicked", () => {
            const onClose = vi.fn();
            render(<CropRotate open onClose={onClose} />);

            fireEvent.pointerDown(document.body);

            expect(onClose).not.toHaveBeenCalled();
            expect(screen.getByRole("dialog")).toBeInTheDocument();
        });

        it("dismisses from the keyboard", () => {
            const onClose = vi.fn();
            render(<CropRotate open onClose={onClose} />);

            fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });

            expect(onClose).toHaveBeenCalled();
        });

        it("dismisses from the close box", () => {
            const onClose = vi.fn();
            render(<CropRotate open onClose={onClose} />);

            fireEvent.click(screen.getByRole("button", { name: "Close" }));

            expect(onClose).toHaveBeenCalled();
        });

        it("dismisses from Cancel", () => {
            const onClose = vi.fn();
            render(<CropRotate open onClose={onClose} />);

            fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

            expect(onClose).toHaveBeenCalled();
        });

        it("writes nothing to either store on any of them", () => {
            render(<CropRotate open onClose={vi.fn()} />);

            fireEvent.click(screen.getByRole("button", { name: "Rotate 90 degrees" }));
            fireEvent.click(screen.getByRole("button", { name: "16:9" }));
            fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

            expect(useCropStore.getState().crops.size).toBe(0);
            expect(useTransformStore.getState().transforms.size).toBe(0);
        });
    });

    describe("the surface behind the photograph", () => {
        it("is dotted, as screen 18 draws it", () => {
            applyBackground("dotted");
            render(<CropRotate open onClose={vi.fn()} />);

            expect(screen.getByRole("dialog").querySelector("[class*='radial-gradient']")).toBeInTheDocument();
        });

        it("is still dotted for a user whose canvas is the particle field", () => {
            applyBackground("particles");
            render(<CropRotate open onClose={vi.fn()} />);

            // The canvas keeps its own choice - only this dialog is fixed.
            expect(document.querySelector("[data-slot='particles']")).toBeNull();
            expect(screen.getByRole("dialog").querySelector("[class*='radial-gradient']")).toBeInTheDocument();
        });
    });
});
