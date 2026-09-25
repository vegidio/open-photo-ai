import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import "@/i18n";
import type { Operation } from "@/ipc/enhance";
import { useEnhancementStore } from "@/stores/enhancements";
import { FRAMING, frame, HOLIDAY, openFiles, render, resetCropStore, resetFileStore } from "@/test/support";
import { NavbarDimensions } from "./NavbarDimensions";

/** An upscale of the given factor, as a chooser built from the catalogue hands one over. */
const upscale = (scale: number): Operation => ({
    family: "upscale",
    codename: "kyoto",
    precision: "fp32",
    parameters: { scale },
});

const stack = (path: string, ...operations: Operation[]) =>
    act(() => useEnhancementStore.setState({ enhancements: new Map([[path, operations]]) }));

/**
 * The block itself, addressed by its own slot.
 *
 * By slot rather than by the text it draws, because the panel repeats both the title and the figure -
 * the comparison names the size the block is reporting - so once it is open a query over the document
 * finds two of each.
 */
const trigger = () => document.querySelector("[data-slot='navbar-dimensions-trigger']") as HTMLElement;

/** What the block itself reads, which is the one number the navbar shows without being asked. */
const reported = () => trigger().querySelector("span:last-of-type")?.textContent;

/** Rests the pointer on the block, which is what opens the comparison. */
const rest = () => fireEvent.pointerEnter(trigger());

/** The label and figure of one row of the comparison, or `undefined` where the row is absent. */
const row = (label: string) => screen.queryByText(label)?.nextElementSibling?.textContent;

beforeEach(() => {
    resetFileStore();
    resetCropStore();
    useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
});

describe("the navbar's dimensions", () => {
    it("reports the file's own size for an image that is neither framed nor enlarged", () => {
        openFiles(HOLIDAY);

        render(<NavbarDimensions file={HOLIDAY} />);

        expect(reported()).toBe("3000 x 2000");
    });

    it("reports the framing's size for an image that carries one", () => {
        openFiles(HOLIDAY);
        frame(HOLIDAY);

        render(<NavbarDimensions file={HOLIDAY} />);

        // What the user is working on rather than what is on disk: the framing is the photograph as
        // far as everything downstream of the crop is concerned, the canvas included.
        expect(reported()).toBe(`${FRAMING.width} x ${FRAMING.height}`);
    });

    it("reports the enhanced result's size for an image the stack enlarges", () => {
        openFiles(HOLIDAY);
        stack(HOLIDAY.path, upscale(2));

        render(<NavbarDimensions file={HOLIDAY} />);

        // The reference's own rule: an enlargement is what the user is working towards, so it wins
        // over both the framing and the file.
        expect(reported()).toBe("6000 x 4000");
    });

    it("enlarges the framing rather than the file", () => {
        openFiles(HOLIDAY);
        frame(HOLIDAY);
        stack(HOLIDAY.path, upscale(2));

        render(<NavbarDimensions file={HOLIDAY} />);

        expect(reported()).toBe(`${FRAMING.width * 2} x ${FRAMING.height * 2}`);
    });

    it("compares the original and the output when the pointer rests on it", () => {
        openFiles(HOLIDAY);
        stack(HOLIDAY.path, upscale(2));

        render(<NavbarDimensions file={HOLIDAY} />);
        rest();

        expect(row("Original")).toBe("3000 x 2000");
        expect(row("Output")).toBe("6000 x 4000");

        // Absent without a framing, where Cropped and Original would be one number.
        expect(screen.queryByText("Cropped")).not.toBeInTheDocument();
    });

    it("puts the framing between the original and the output when there is one", () => {
        openFiles(HOLIDAY);
        frame(HOLIDAY);
        stack(HOLIDAY.path, upscale(2));

        render(<NavbarDimensions file={HOLIDAY} />);
        rest();

        expect(row("Original")).toBe("3000 x 2000");
        expect(row("Cropped")).toBe(`${FRAMING.width} x ${FRAMING.height}`);
        expect(row("Output")).toBe(`${FRAMING.width * 2} x ${FRAMING.height * 2}`);
    });

    it("folds the comparison away when the pointer leaves", () => {
        openFiles(HOLIDAY);

        render(<NavbarDimensions file={HOLIDAY} />);
        rest();

        expect(screen.getByText("Output")).toBeInTheDocument();

        fireEvent.pointerLeave(trigger());

        expect(screen.queryByText("Output")).not.toBeInTheDocument();
    });

    it("draws nothing at all for a photograph it could not measure", () => {
        // Built by leaving the keys out rather than setting them to `undefined`, which under
        // `exactOptionalPropertyTypes` is not the same record.
        const { width, height, ...unmeasured } = HOLIDAY;
        openFiles(unmeasured);

        const { container } = render(<NavbarDimensions file={unmeasured} />);

        // The separator is inside the block rather than beside it, so a rule with nothing on one side
        // of it is not what an unmeasurable photograph leaves behind.
        expect(container.querySelector("[data-slot='navbar-dimensions']")).toBeNull();
        expect(screen.queryByText("Dimensions")).not.toBeInTheDocument();
    });

    it("still reports the framing of a photograph it could not measure", () => {
        const { width, height, ...unmeasured } = HOLIDAY;
        openFiles(unmeasured);
        frame(unmeasured);

        render(<NavbarDimensions file={unmeasured} />);
        rest();

        // A framing is a rectangle the user dragged, so it has dimensions whatever the header said.
        // There is no Original to compare it against, which is the honest answer rather than a gap.
        expect(reported()).toBe(`${FRAMING.width} x ${FRAMING.height}`);
        expect(screen.queryByText("Original")).not.toBeInTheDocument();
        expect(row("Cropped")).toBe(`${FRAMING.width} x ${FRAMING.height}`);
    });
});
