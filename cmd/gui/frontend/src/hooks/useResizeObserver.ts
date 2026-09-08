import { useEffect } from 'react';

// Measures an element and re-measures it whenever it resizes.
//
// The pattern this replaces was written out twice - the immediate first measurement, the ResizeObserver, and the
// disconnect on cleanup - and the part that is easy to drop when copying it is the `typeof ResizeObserver` guard,
// without which the component throws in any environment that does not provide one (a test renderer, a prerender).
//
// Both callbacks must be stable, so wrap them in useCallback at the call site: they are the effect's dependencies, and
// an inline closure would tear down and rebuild the observer on every render. `getElement` is a callback rather than a
// ref because not every caller has one - ZoomImage reaches its element through a third-party component's instance.
export const useResizeObserver = (
    getElement: () => Element | null | undefined,
    measure: (element: Element) => void,
) => {
    useEffect(() => {
        const element = getElement();
        if (!element) return;

        const onResize = () => measure(element);

        onResize();

        if (typeof ResizeObserver === 'undefined') return;

        const observer = new ResizeObserver(onResize);
        observer.observe(element);

        return () => observer.disconnect();
    }, [getElement, measure]);
};
