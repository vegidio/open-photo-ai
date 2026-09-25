import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { DialogTitleBar } from "@/components/ui/dialog-title-bar";
import "@/i18n";
import { render } from "@/test/support";

/** The bar as every caller mounts it: inside a dialog, which is the only place it means anything. */
const mount = (onOpenChange?: (open: boolean) => void) =>
    render(
        <Dialog open {...(onOpenChange && { onOpenChange })}>
            <DialogContent showCloseButton={false}>
                <DialogTitleBar title="Settings" />
            </DialogContent>
        </Dialog>,
    );

describe("DialogTitleBar", () => {
    it("names the dialog for assistive technology", () => {
        mount();

        // The accessible name rather than the visible text: the two are the same string here by
        // construction, and it is the announced one that a bar drawing a plain span would lose.
        expect(screen.getByRole("dialog")).toHaveAccessibleName("Settings");
    });

    it("closes the dialog it sits in", () => {
        const onOpenChange = vi.fn();
        mount(onOpenChange);

        fireEvent.click(screen.getByRole("button", { name: "Close" }));

        // Through the dialog's own close channel, which is what Escape also goes through - so a
        // caller that has to undo something on dismissal has one place to do it rather than two.
        expect(onOpenChange).toHaveBeenCalledWith(false);
    });

    it("draws no close control for a dialog that has to be answered", () => {
        render(
            <Dialog open>
                <DialogContent showCloseButton={false}>
                    <DialogTitleBar title="TensorRT Detected" showCloseButton={false} />
                </DialogContent>
            </Dialog>,
        );

        expect(screen.getByRole("dialog")).toHaveAccessibleName("TensorRT Detected");
        expect(screen.queryByRole("button", { name: "Close" })).not.toBeInTheDocument();
    });
});
