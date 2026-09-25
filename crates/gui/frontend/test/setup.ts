// The `/vitest` entry point rather than the bare package: it is the one that declares the matchers
// against Vitest's `Assertion` interface. This file lives under `frontend`, which tsconfig.json
// already includes, so that augmentation is picked up by `tsc --noEmit` without a `types` entry.

import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// Registered by hand because `globals` is off in vite.config.ts: Testing Library's automatic cleanup
// hooks onto a *global* `afterEach`. Without this every render stays in `document.body` for the rest
// of the file, which is silent for in-container queries but breaks `screen`.
afterEach(cleanup);

// jsdom implements no layout, so it ships no `ResizeObserver` - and Radix measures its Slider thumb
// with one the moment the component mounts, which throws before any assertion runs. A no-op is the
// honest stub rather than a gap papered over: there is nothing to observe in an environment that
// lays nothing out, and no test here asserts on a measured size. It lives in the shared setup
// because every future Radix component that measures itself needs it, not just the Slider.
globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
};

// jsdom implements no layout, so it ships no `scrollIntoView` either - and Radix calls it on the
// highlighted item every time a Select opens, which throws before any assertion runs. The same
// honest stub as the observer above: there is nothing to scroll in an environment that lays nothing
// out. It is on the prototype rather than on one element because the settings dialog's nav scrolls
// rows a test has not addressed, and a test that wants to observe a scroll spies on this.
Element.prototype.scrollIntoView = function scrollIntoView() {};

// jsdom implements no pointer capture either - `setPointerCapture` and its two companions are simply
// absent from `Element` - and the canvas calls all three to pan a magnified photograph. The same
// honest stub as the two above: capture is about which element keeps receiving moves once the
// pointer leaves it, and nothing in an environment without layout or a real pointer can leave
// anything. The set is per element, so a test can still assert that a drag took the capture.
const captured = new WeakMap<Element, Set<number>>();
const captures = (element: Element) => {
    const existing = captured.get(element);
    if (existing) return existing;

    const fresh = new Set<number>();
    captured.set(element, fresh);

    return fresh;
};

Element.prototype.setPointerCapture = function setPointerCapture(pointerId: number) {
    captures(this).add(pointerId);
};

Element.prototype.releasePointerCapture = function releasePointerCapture(pointerId: number) {
    captures(this).delete(pointerId);
};

Element.prototype.hasPointerCapture = function hasPointerCapture(pointerId: number) {
    return captures(this).has(pointerId);
};

// jsdom implements no canvas either - `getContext` is present but throws a "Not implemented" notice
// to the console and answers `null`, for every call, in every test that mounts something which
// draws. The particle field behind the preview is one, so that is most of them. The same honest stub
// as the three above: there is nothing to rasterise in an environment with no layout and no pixels,
// and the field's own frame already guards a missing context.
//
// A no-op context rather than `null`, because `null` is what jsdom already answers: a component
// would take the "no canvas here" path in every test, and nothing that draws would ever be exercised
// at all. A test that wants to observe what was drawn installs its own over this - see
// `stubCanvas2D` in `test/support.tsx`, which spies and hands back the calls.
const context2d = () =>
    new Proxy(
        {},
        {
            get: (target: Record<string, unknown>, property: string) => {
                // Properties that are written and read back (`fillStyle`) rather than called: kept
                // on the target so an assignment is not simply swallowed.
                if (property in target) return target[property];

                return () => undefined;
            },
            set: (target: Record<string, unknown>, property: string, value: unknown) => {
                target[property] = value;

                return true;
            },
        },
    );

HTMLCanvasElement.prototype.getContext = function getContext(kind: string) {
    return kind === "2d" ? (context2d() as unknown as CanvasRenderingContext2D) : null;
} as HTMLCanvasElement["getContext"];
