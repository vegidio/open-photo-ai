import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import type { Face } from "@/ipc/faces";
import { faceKey } from "@/lib/faces";
import { FRAMING, render } from "@/test/support";
import { FaceBoxes } from "./FaceBoxes";

/** A face whose box is a tenth of `HOLIDAY`'s 3000 x 2000, a tenth in from its top-left corner. */
const TENTH: Face = {
    bounding_box: { min: { x: 300, y: 200 }, max: { x: 600, y: 400 } },
    landmarks: [
        { x: 400, y: 250 },
        { x: 500, y: 250 },
        { x: 450, y: 300 },
        { x: 400, y: 350 },
        { x: 500, y: 350 },
    ],
    confidence: 0.9,
};

/** A second face, so a test can tell one box from another. */
const OTHER: Face = { ...TENTH, bounding_box: { min: { x: 1500, y: 1000 }, max: { x: 1800, y: 1200 } } };

const toggled = vi.fn();

const boxes = (props: Partial<Parameters<typeof FaceBoxes>[0]> = {}) =>
    render(
        <FaceBoxes
            width={3000}
            height={2000}
            faces={[TENTH, OTHER]}
            skipped={new Set()}
            onToggle={toggled}
            {...props}
        />,
    );

const box = (number: number) => screen.getByRole("button", { name: `Toggle face ${number}` });

beforeEach(() => {
    toggled.mockClear();
});

describe("the boxes over a photograph's faces", () => {
    it("places a box at its share of the photograph's own dimensions", () => {
        // 300 of 3000 across and 200 of 2000 down, 300 wide and 200 tall: a tenth on every edge.
        boxes();

        expect(box(1).style).toMatchObject({ left: "10%", top: "10%", width: "10%", height: "10%" });
    });

    it("draws one box per face found", () => {
        boxes();

        expect(screen.getAllByRole("button")).toHaveLength(2);
        expect(box(2).style).toMatchObject({ left: "50%", top: "50%" });
    });

    it("takes its shares against the dimensions it is given", () => {
        // The coordinates are in the framed photograph's pixels, so the same face is a different
        // share of a 1200 x 1600 framing than it is of the 3000 x 2000 file.
        boxes({ width: FRAMING.width, height: FRAMING.height });

        expect(box(1).style).toMatchObject({ left: "25%", top: "12.5%", width: "25%", height: "12.5%" });
    });

    it("draws nothing at all when the photograph's size is not known", () => {
        render(<FaceBoxes faces={[TENTH, OTHER]} skipped={new Set()} onToggle={toggled} />);

        expect(screen.queryAllByRole("button")).toHaveLength(0);
    });

    it("tells a chosen face from a skipped one", () => {
        boxes({ skipped: new Set([faceKey(TENTH)]) });

        expect(box(1)).toHaveAttribute("aria-pressed", "false");
        expect(box(1).className).toContain("border-foreground-dim");
        expect(box(2)).toHaveAttribute("aria-pressed", "true");
        expect(box(2).className).toContain("border-warning");
    });

    it("names each box by the number of the face it is over", () => {
        // The only thing that tells one box from another to a user who cannot see the picture.
        boxes();

        expect(box(1)).toBeInTheDocument();
        expect(box(2)).toBeInTheDocument();
    });

    it("reports the face itself when a box is operated", () => {
        boxes();

        fireEvent.click(box(2));

        expect(toggled).toHaveBeenCalledExactlyOnceWith(OTHER);
    });

    it("is operable from the keyboard", () => {
        // A button, so it is in the tab order and answers Enter and Space without anything here
        // arranging it. jsdom dispatches the click a key press produces, which is what this asserts.
        boxes();

        box(1).focus();
        expect(box(1)).toHaveFocus();

        fireEvent.keyDown(box(1), { key: "Enter" });
        fireEvent.click(box(1));

        expect(toggled).toHaveBeenCalledWith(TENTH);
    });
});
