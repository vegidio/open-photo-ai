import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RATIOS } from "@/features/crop/ratios";
import "@/i18n";
import { render } from "@/test/support";
import { AspectRatios } from "./AspectRatios";

describe("AspectRatios", () => {
    it("draws all ten options", () => {
        render(<AspectRatios selected="free" onSelect={vi.fn()} />);

        expect(screen.getAllByRole("button")).toHaveLength(RATIOS.length);
    });

    it("names the two that are words from the catalogue and the rest as they read", () => {
        render(<AspectRatios selected="free" onSelect={vi.fn()} />);

        expect(screen.getByRole("button", { name: "Free" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "Square" })).toBeInTheDocument();
        expect(screen.getByRole("button", { name: "16:9" })).toBeInTheDocument();
    });

    it("marks exactly the one in force", () => {
        render(<AspectRatios selected="3:2" onSelect={vi.fn()} />);

        const marked = screen.getAllByRole("button").filter((option) => option.ariaPressed === "true");

        expect(marked).toHaveLength(1);
        expect(marked[0]).toHaveTextContent("3:2");
    });

    it("hands the chosen option's key and its ratio to the controller", () => {
        const onSelect = vi.fn();
        render(<AspectRatios selected="free" onSelect={onSelect} />);

        fireEvent.click(screen.getByRole("button", { name: "16:9" }));

        expect(onSelect).toHaveBeenCalledWith("16:9");
    });

    it("hands the free option's own key", () => {
        const onSelect = vi.fn();
        render(<AspectRatios selected="16:9" onSelect={onSelect} />);

        fireEvent.click(screen.getByRole("button", { name: "Free" }));

        expect(onSelect).toHaveBeenCalledWith("free");
    });

    it("draws each option as a rectangle in its own proportions", () => {
        render(<AspectRatios selected="free" onSelect={vi.fn()} />);

        const landscape = screen.getByRole("button", { name: "16:9" }).querySelector("span > span");
        const portrait = screen.getByRole("button", { name: "9:16" }).querySelector("span > span");

        // The table's 21x12 and 12x21 at `RATIO_BOX_SCALE`, rounded per edge - so this moves with
        // that constant, which is the one knob the grid's size has.
        expect(landscape).toHaveStyle({ width: "26px", height: "15px" });
        expect(portrait).toHaveStyle({ width: "15px", height: "26px" });
    });
});
