import { describe, expect, it } from "vitest";
import { render } from "@/test/support";
import { ShineBorder } from "./shine-border";

/**
 * The port, tested for what the port is about rather than for what it looks like.
 *
 * jsdom lays nothing out and animates nothing, so there is no sweeping gradient here to assert on -
 * and the mask, the gradient and the background size are upstream's, carried over unchanged. What
 * is worth pinning is what a caller hands the element: the two custom properties the utility and
 * the mask read, the colour list becoming one gradient in the order given, and that an element
 * wearing a shine still receives everything the pointer does.
 */
describe("ShineBorder", () => {
    const shine = () => document.querySelector<HTMLElement>("[data-slot='shine-border']");

    it("costs the pointer nothing", () => {
        render(<ShineBorder />);

        // Full-bleed over whatever is wearing it, which is the whole reason this matters: an
        // element that intercepted a pointer would break whatever the shine was drawn to advertise.
        expect(shine()).toHaveClass("pointer-events-none");
        expect(shine()).toHaveAttribute("aria-hidden", "true");
    });

    it("hands the duration to the keyframe as a property, not as a rule", () => {
        render(<ShineBorder duration={5} />);

        // The whole reason the animation can be a single Tailwind utility: what a caller varies
        // arrives on the element, so `animate-shine` names no duration of its own. See the
        // `@theme inline` note in `style.css` for why the `inline` there is load-bearing.
        expect(shine()?.style.getPropertyValue("--duration")).toBe("5s");
        expect(shine()).toHaveClass("motion-safe:animate-shine");
    });

    it("leaves a shine out of the sweep for a user who asked for less motion", () => {
        render(<ShineBorder />);

        // `motion-safe:` rather than an unconditional `animate-shine`: the border stays, only the
        // sweep goes. Upstream's, and kept deliberately - this is decoration.
        expect(shine()?.className).not.toMatch(/(^| )animate-shine/);
    });

    it("sets the ring's thickness where both the padding and the mask read it", () => {
        render(<ShineBorder borderWidth={2} />);

        // One property, two consumers: the padding is what the mask's `exclude` leaves behind, so
        // this number *is* the border's width rather than something kept equal to it.
        expect(shine()?.style.getPropertyValue("--border-width")).toBe("2px");
    });

    it("takes a single colour to the gradient as it stands", () => {
        render(<ShineBorder shineColor="var(--activity)" />);

        // A token rather than a literal is the case the application actually uses, and it has to
        // survive to the element unaltered for the gradient to resolve it there.
        expect(shine()?.style.getPropertyValue("--shine-colors")).toBe("var(--activity)");
    });

    it("runs a list of colours through one gradient, in the order given", () => {
        render(<ShineBorder shineColor={["var(--success-bright)", "var(--activity)"]} />);

        // Several colours are one shine, not several: they are stops in the same gradient, so the
        // order is what the eye sees travelling past and not an implementation detail.
        expect(shine()?.style.getPropertyValue("--shine-colors")).toBe("var(--success-bright),var(--activity)");
    });

    it("inherits the radius of whatever it is drawn in", () => {
        render(<ShineBorder />);

        // The component carries no radius of its own, which is what lets the mark trace exactly the
        // box that wears it however that box is rounded.
        expect(shine()).toHaveClass("rounded-[inherit]");
    });

    it("lets a caller's own style win over its declarations", () => {
        render(<ShineBorder style={{ backgroundSize: "100% 100%" }} />);

        // Spread last, upstream's order: a caller reaching for `style` at all is reaching past
        // something this component already set - here the 300% that leaves most of the gradient
        // off the box, which is what there is to sweep.
        expect(shine()).toHaveStyle({ backgroundSize: "100% 100%" });
    });
});
