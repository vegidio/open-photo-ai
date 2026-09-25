import { useCallback, useRef } from "react";
import { act } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { render } from "@/test/support";
import { useResizeObserver } from "./useResizeObserver";

/**
 * A `ResizeObserver` whose callbacks can be fired on demand.
 *
 * The shared setup installs a no-op stub, because jsdom lays nothing out and Radix's Slider measures
 * itself on mount. A no-op is the right global for every other test and useless here: what this hook
 * is *for* is the second measurement, and a stub that never calls back cannot produce one.
 */
const controllable = () => {
    const callbacks = new Set<ResizeObserverCallback>();
    const disconnect = vi.fn();

    vi.stubGlobal(
        "ResizeObserver",
        class {
            constructor(private readonly callback: ResizeObserverCallback) {}
            observe() {
                callbacks.add(this.callback);
            }
            unobserve() {}
            disconnect() {
                callbacks.delete(this.callback);
                disconnect();
            }
        },
    );

    return {
        disconnect,
        /** Reports a resize to everything observing, the way the platform would. */
        resize: () =>
            act(() => {
                for (const callback of callbacks) callback([], {} as ResizeObserver);
            }),
    };
};

/** A component measuring its own element, which is how every caller of this hook uses it. */
const Measured = ({ measure }: { measure: (element: Element) => void }) => {
    const ref = useRef<HTMLDivElement>(null);

    useResizeObserver(
        useCallback(() => ref.current, []),
        measure,
    );

    return <div ref={ref} data-testid="measured" />;
};

describe("useResizeObserver", () => {
    it("measures the element once it is mounted", () => {
        const measure = vi.fn();

        const { getByTestId } = render(<Measured measure={measure} />);

        expect(measure).toHaveBeenCalledTimes(1);
        expect(measure).toHaveBeenCalledWith(getByTestId("measured"));
    });

    it("measures again when the element resizes", () => {
        const { resize } = controllable();
        const measure = vi.fn();

        render(<Measured measure={measure} />);
        expect(measure).toHaveBeenCalledTimes(1);

        resize();

        expect(measure).toHaveBeenCalledTimes(2);
    });

    it("stops observing when the component goes away", () => {
        const { disconnect } = controllable();

        render(<Measured measure={vi.fn()} />).unmount();

        expect(disconnect).toHaveBeenCalled();
    });

    /**
     * The guard the reference's comment names as the easy one to drop. Without it the hook throws on
     * `new ResizeObserver` in any environment that provides none - a bare test renderer, a prerender -
     * and takes the whole component down with it rather than losing the second measurement.
     */
    it("measures once and does not throw where the platform has no observer", () => {
        vi.stubGlobal("ResizeObserver", undefined);
        const measure = vi.fn();

        expect(() => render(<Measured measure={measure} />)).not.toThrow();
        expect(measure).toHaveBeenCalledTimes(1);
    });
});
