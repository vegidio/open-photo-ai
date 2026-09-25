import { useEffect } from "react";

/**
 * Measures an element and re-measures it whenever it resizes.
 *
 * Ported from the reference, whose own comment records what is easy to drop when the three lines are
 * written out by hand at each call site - the immediate first measurement, the observer, and the
 * disconnect on cleanup. The part that is easiest to drop is the `typeof ResizeObserver` guard,
 * without which the component throws in any environment that does not provide one: jsdom has none,
 * so every test that mounted the strip would fail on the hook rather than on what it is about.
 *
 * **Both callbacks must be stable** - wrap them in `useCallback` at the call site. They are the
 * effect's dependencies, and an inline closure would tear down and rebuild the observer on every
 * render.
 *
 * `getElement` is a callback rather than a ref because it is read inside the effect, after the
 * render that attaches the ref: a ref object passed in would be read at the same moment either way,
 * and the callback form is what lets a caller reach an element it does not hold a ref to.
 */
export const useResizeObserver = (
    getElement: () => Element | null | undefined,
    measure: (element: Element) => void,
) => {
    useEffect(() => {
        const element = getElement();
        if (!element) return;

        const onResize = () => measure(element);

        // Measured once before the observer is built, and not only because an observer fires on
        // observe: an environment without one measures here or never.
        onResize();

        if (typeof ResizeObserver === "undefined") return;

        const observer = new ResizeObserver(onResize);
        observer.observe(element);

        return () => observer.disconnect();
    }, [getElement, measure]);
};
