import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import "@/i18n";
import { useSettingsStore } from "@/stores/settings";
import { render, resetSettingsStore } from "@/test/support";
import { TensorRTDialog } from "./TensorRTDialog";

beforeEach(() => {
    localStorage.clear();
    resetSettingsStore();
});

describe("TensorRTDialog", () => {
    it("says what TensorRT is, what its first run costs, and where the answer can be changed", () => {
        render(<TensorRTDialog />);

        expect(screen.getByRole("dialog")).toHaveAccessibleName("TensorRT Detected");
        expect(screen.getByRole("img", { name: "TensorRT" })).toBeInTheDocument();
        expect(screen.getByText(/We detected a GPU that supports TensorRT/)).toBeInTheDocument();
        expect(screen.getByText("Would you like to enable TensorRT?")).toBeInTheDocument();

        // The emphasis is its own element inside the sentence, rather than the sentence losing it.
        const emphasis = screen.getByText("This optimization step can take a few minutes the first time it runs");
        expect(emphasis).toHaveClass("font-bold");
        expect(emphasis.parentElement).toHaveTextContent(/must first optimize the model graph/);

        expect(screen.getByText("Settings")).toHaveClass("font-bold");
        expect(screen.getByText("AI processor")).toHaveClass("underline");
        expect(screen.getByText(/Regardless of what you choose now/)).toBeInTheDocument();
    });

    it.each([
        ["Yes", "auto"],
        ["No", "cuda"],
    ])("answered %s, sets the processor and records the answer", (answer, processor) => {
        useSettingsStore.setState({ processor: "coreml" });
        render(<TensorRTDialog />);

        fireEvent.click(screen.getByRole("button", { name: answer }));

        expect(useSettingsStore.getState()).toMatchObject({ processor, tensorrtAsked: true });
    });

    it("cannot be dismissed by Escape, by clicking away, or by a close button", () => {
        render(<TensorRTDialog />);

        expect(screen.queryByRole("button", { name: "Close" })).not.toBeInTheDocument();

        fireEvent.keyDown(document, { key: "Escape" });
        expect(screen.getByRole("dialog")).toBeInTheDocument();

        // Radix closes on `pointerdown` outside, which is what the prevented handler intercepts.
        fireEvent.pointerDown(document.body);
        fireEvent.click(document.body);
        expect(screen.getByRole("dialog")).toBeInTheDocument();

        // And nothing was recorded on the way: a dismissal is not an answer.
        expect(useSettingsStore.getState().tensorrtAsked).toBe(false);
    });
});
