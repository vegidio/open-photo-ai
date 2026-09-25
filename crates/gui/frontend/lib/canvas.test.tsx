import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { useSettingsStore } from "@/stores/settings";
import { applyBackground, render, stubCanvas2D } from "@/test/support";
import { useCanvasSurface } from "./canvas";

/**
 * The one question two regions ask - the preview today, the crop dialog when it is built.
 *
 * Tested through a probe that uses the pair the way a canvas does, rather than through `renderHook`:
 * half of what this returns is a node, and what matters about it is that it ends up *inside* the
 * element the classes are on.
 */
const Probe = () => {
    const { className, field } = useCanvasSurface();

    return (
        <div data-testid="canvas" className={className}>
            {field}
        </div>
    );
};

const canvas = () => screen.getByTestId("canvas");

const field = () => document.querySelector<HTMLElement>("[data-slot='particles']");

beforeEach(() => {
    localStorage.clear();
    useSettingsStore.setState(useSettingsStore.getInitialState(), true);
    stubCanvas2D();
});

describe("useCanvasSurface", () => {
    it("draws the dotted grid and no field where that is the choice", () => {
        applyBackground("dotted");

        render(<Probe />);

        expect(canvas()).toHaveClass("bg-[radial-gradient(var(--color-surface-dot)_1px,transparent_1px)]");
        expect(canvas()).toHaveClass("bg-size-[3rem_3rem]");
        expect(field()).not.toBeInTheDocument();
    });

    it("draws the field and no dots where that is the choice", () => {
        applyBackground("particles");

        render(<Probe />);

        // Exclusive rather than cumulative: the field replaces the dots, it is not layered over them.
        expect(canvas().className).not.toContain("radial-gradient");
        expect(field()).toBeInTheDocument();
        expect(canvas()).toContainElement(field());
    });

    it("keeps the same background colour under either surface", () => {
        // The two options differ in what is drawn over `--background` and in nothing else, which is
        // what makes switching between them a change of surface rather than a change of theme.
        applyBackground("dotted");
        const { unmount } = render(<Probe />);
        expect(canvas()).toHaveClass("bg-background");
        unmount();

        applyBackground("particles");
        render(<Probe />);
        expect(canvas()).toHaveClass("bg-background");
    });
});

/**
 * A restart, which is the claim the retired canvas store carried its own test for.
 *
 * It held the *applied* background as a second copy of the settings store's, and had to subscribe to
 * `persist`'s rehydration to stay in step with it - so "the remembered choice is still in effect
 * after a restart" was a wiring claim worth asserting. There is no second copy now; the canvas reads
 * the preference itself. The claim is still worth a test, because what it is really about is that
 * the canvas reads a *committed* value, and it is one line of storage away from being wrong again.
 */
describe("what a restart draws", () => {
    it("follows a background remembered from a previous session", async () => {
        localStorage.setItem("settings-storage", JSON.stringify({ state: { background: "dotted" }, version: 0 }));

        await useSettingsStore.persist.rehydrate();

        render(<Probe />);

        expect(canvas().className).toContain("radial-gradient");
        expect(field()).not.toBeInTheDocument();
    });
});
