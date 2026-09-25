import { describe, expect, it } from "vitest";
import { render } from "@/test/support";
import {
    PreviewFullIcon,
    PreviewSideIcon,
    PreviewSplitIcon,
    previewFullIconData,
    previewSideIconData,
    previewSplitIconData,
} from "./preview-modes";

describe.each([
    ["full", PreviewFullIcon, previewFullIconData],
    ["side", PreviewSideIcon, previewSideIconData],
    ["split", PreviewSplitIcon, previewSplitIconData],
])("the %s preview-mode icon", (_name, Icon, data) => {
    it("inherits its colour from the surrounding text", () => {
        // What makes these usable anywhere a lucide icon is: they take their colour from the text
        // around them, which is what lets the toggle group state its active and inactive colours once
        // on the item rather than on each icon.
        const { container } = render(<Icon />);

        const svg = container.querySelector("svg");

        expect(svg).not.toBeNull();
        expect(svg).toHaveAttribute("stroke", "currentColor");
        expect(svg).toHaveAttribute("viewBox", "0 0 24 24");
    });

    it("gives every path a key", () => {
        // Asserted against the data rather than by spying on `console.error`: React dedupes the
        // missing-key warning per owner component, and all three icons share lucide's `Icon` as their
        // owner - so a spy only ever sees the first one, and whichever icon rendered second would
        // regress in silence.
        for (const [, attributes] of data.node) {
            expect(attributes.key).toBeTruthy();
        }
    });
});
