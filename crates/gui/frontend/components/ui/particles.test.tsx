import { act, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, stubCanvas2D } from "@/test/support";
import { Particles } from "./particles";

/**
 * The vendored field, tested for the three things the adaptation is about rather than for what it
 * draws: jsdom provides no 2D context at all, so there is no picture here to assert on, and what a
 * particle does per frame is upstream's arithmetic carried over unchanged.
 *
 * What is this codebase's own - and what a later edit could silently undo - is that it costs the
 * pointer nothing and says nothing to a screen reader.
 */
describe("Particles", () => {
    beforeEach(stubCanvas2D);

    it("draws itself on a canvas", () => {
        const { container } = render(<Particles />);

        expect(container.querySelector("canvas")).toBeInTheDocument();
    });

    it("registers no pointer listener on the window", () => {
        // The divergence from upstream that has a cost attached: its `MousePosition` hook drives
        // React state from a `window` `mousemove`, which re-renders this component on every frame of
        // a pan gesture across the canvas. Asserted against `window` specifically, because that is
        // where upstream puts it - an element-level listener would not be the same bug.
        const listen = vi.spyOn(window, "addEventListener");

        render(<Particles />);

        expect(listen).not.toHaveBeenCalledWith("mousemove", expect.anything(), expect.anything());
        expect(listen).not.toHaveBeenCalledWith("mousemove", expect.anything());
    });

    it("is invisible to assistive technology and to the pointer", () => {
        const { container } = render(<Particles />);

        const field = container.querySelector("[data-slot='particles']");

        // `aria-hidden` is politeness; `pointer-events-none` is load-bearing. The canvas this is
        // drawn into carries the wheel-zoom and drag-to-pan handlers, and a full-bleed element over
        // them that could swallow an event would break both.
        expect(field).toHaveAttribute("aria-hidden", "true");
        expect(field).toHaveClass("pointer-events-none");
        expect(screen.queryByRole("img")).not.toBeInTheDocument();
    });
});

/**
 * The focus gate (design.md D4).
 *
 * Driven through the DOM's `focus`/`blur` on `window`, which is the fallback the component wires
 * beside Tauri's own `onFocusChanged` precisely so this environment has a way in - there is no Tauri
 * here to emit the real signal.
 *
 * `document.hasFocus` is stubbed rather than assumed: jsdom answers `false` by default, so without
 * it the loop would never start and every assertion below would pass against a component that does
 * nothing at all.
 */
describe("Particles while the window comes and goes", () => {
    /** Mounts a focused field and hands back the frame handles and the fake context. */
    const mount = () => {
        const context = stubCanvas2D();
        vi.spyOn(document, "hasFocus").mockReturnValue(true);

        const schedule = vi.spyOn(window, "requestAnimationFrame").mockReturnValue(1);
        const cancel = vi.spyOn(window, "cancelAnimationFrame");

        render(<Particles />);

        return { context, schedule, cancel };
    };

    it("animates while the window has focus", () => {
        const { schedule } = mount();

        expect(schedule).toHaveBeenCalled();
    });

    it("cancels the pending frame when the window loses focus, and leaves the field drawn", () => {
        const { context, schedule, cancel } = mount();
        schedule.mockClear();
        context.clearRect.mockClear();

        act(() => window.dispatchEvent(new Event("blur")));

        expect(cancel).toHaveBeenCalledWith(1);
        // Nothing further is scheduled, and - the half that makes the stop invisible - nothing wipes
        // the canvas. The last frame stays painted while the window is away.
        expect(schedule).not.toHaveBeenCalled();
        expect(context.clearRect).not.toHaveBeenCalled();
    });

    it("resumes from the field it stopped on when focus comes back", () => {
        const { context, schedule } = mount();
        act(() => window.dispatchEvent(new Event("blur")));
        schedule.mockClear();
        // `setTransform` is called by nothing but the initialiser, which is also what repopulates
        // the particle array - so it standing still is what "resumes rather than restarts" means.
        // The particles live in a ref, and a resume that rebuilt them would be visible as the whole
        // field reshuffling every time the user came back to the window: the alternative D4 rejects.
        context.setTransform.mockClear();

        act(() => window.dispatchEvent(new Event("focus")));

        expect(schedule).toHaveBeenCalledTimes(1);
        expect(context.setTransform).not.toHaveBeenCalled();
    });

    it("paints the field once on mount even if the window is behind another", () => {
        // The other side of "the field is still drawn while the window is away". The gate stops by
        // leaving the last frame painted, and a field that never painted a first one has no last one
        // to leave - so an application launched and switched away from before its window came up
        // would show a bare canvas until the user came back to it.
        const context = stubCanvas2D();
        vi.spyOn(document, "hasFocus").mockReturnValue(false);
        const schedule = vi.spyOn(window, "requestAnimationFrame").mockReturnValue(1);

        render(<Particles />);

        // Drawn, but not animating: the initialiser paints it, and the loop stays shut until focus.
        expect(context.arc).toHaveBeenCalled();
        expect(schedule).not.toHaveBeenCalled();
    });

    it("does not stack a second loop when focus is reported twice", () => {
        const { schedule } = mount();
        schedule.mockClear();

        // Both signals are wired at once - Tauri's and the DOM's - so a browser delivering both must
        // not leave a second, uncancellable loop running.
        act(() => window.dispatchEvent(new Event("focus")));
        act(() => window.dispatchEvent(new Event("focus")));

        expect(schedule).not.toHaveBeenCalled();
    });
});
