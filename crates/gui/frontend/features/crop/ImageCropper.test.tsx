import { createRef } from "react";
import type { CropperRef } from "react-advanced-cropper";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, stubCanvas2D } from "@/test/support";
import { ImageCropper } from "./ImageCropper";

/**
 * What jsdom can say about the widget, which is not much and is stated here so the next reader does
 * not go looking for the rest.
 *
 * There is no layout in jsdom, and the widget *is* layout: the stencil, the drag, the wheel zoom and
 * the rotated canvas are all measured against boxes that are all zero here. What this file covers is
 * that the wrapper mounts and that the ref reaches through it, which is the half the controller
 * depends on - every one of its handlers goes through that ref, so a wrapper that swallowed it would
 * leave a dialog whose controls all silently did nothing. The framing itself is covered by hand on
 * macOS.
 *
 * The stencil's own styling - the undimmed surround and the permanently visible thirds grid - is not
 * covered here either, and cannot be: the widget mounts no stencil until an image has loaded, and
 * jsdom loads none. Nor is the clip that keeps a magnified photograph from spilling into the ring
 * the drag handles live in, for the same reason and one more: the widget renders no background at
 * all until it has a state, so there is nothing in this DOM to assert a clip on. All three are
 * checked by eye, and `ImageCropper.tsx` says what each of them is for.
 */
describe("ImageCropper", () => {
    beforeEach(() => {
        stubCanvas2D();
    });

    it("mounts and draws the widget", () => {
        render(<ImageCropper src="opai://localhost/abc" onChange={vi.fn()} onReady={vi.fn()} />);

        // The widget's own root class rather than a `data-slot`: `<Cropper>` does not spread unknown
        // props onto its element, so the slot convention the rest of this codebase queries by does
        // not reach the DOM here.
        expect(document.querySelector(".advanced-cropper")).toBeInTheDocument();
    });

    it("fills the pane it is given", () => {
        const { container } = render(<ImageCropper src="opai://localhost/abc" onChange={vi.fn()} onReady={vi.fn()} />);

        // The reference's geometry, and the shape of two bugs if it drifts: a box sized to the
        // photograph instead draws it smaller than the space reserved for it, and any clip tighter
        // than this element cuts the drag handles, which overhang the rectangle's edge by design.
        const root = container.querySelector<HTMLElement>(".advanced-cropper");

        expect(root?.className).toContain("size-full");
        expect(root?.className).toContain("p-1.5");
        expect(root?.style.aspectRatio).toBe("");
    });

    it("asks for the rendition the way the canvas asks for it", () => {
        const { container } = render(<ImageCropper src="opai://localhost/abc" onChange={vi.fn()} onReady={vi.fn()} />);

        // `crossOrigin` defaults to true in this widget, which makes the request a CORS one - and the
        // `opai://` scheme sends no `Access-Control-Allow-Origin`, so the photograph never loads.
        for (const image of container.querySelectorAll("img")) {
            expect(image.getAttribute("crossorigin")).toBeNull();
        }
    });

    it("forwards its ref, which is the controller's whole way in", () => {
        const ref = createRef<CropperRef>();

        render(<ImageCropper ref={ref} src="opai://localhost/abc" onChange={vi.fn()} onReady={vi.fn()} />);

        expect(ref.current).not.toBeNull();
        // The five the controller actually calls. Named rather than counted, so a version that
        // renamed one fails here rather than in a dialog whose buttons stopped working.
        for (const method of ["getState", "getCoordinates", "setCoordinates", "rotateImage", "flipImage"] as const) {
            expect(typeof ref.current?.[method]).toBe("function");
        }
    });
});
