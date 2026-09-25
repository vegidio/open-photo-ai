import { useState } from "react";
import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, type Mock, vi } from "vitest";
import "@/i18n";
import { openLink } from "@/ipc/links";
import { APP_COPYRIGHT, APP_NAME } from "@/lib/constants";
import { render } from "@/test/support";
import { AboutDialog } from "./AboutDialog";

// The IPC wrapper rather than its transport: `ipc/links.test.ts` pins the wire name, so this file is
// only about what the dialog does with it.
vi.mock("@/ipc/links", () => ({ openLink: vi.fn(() => Promise.resolve()) }));

const opened = openLink as unknown as Mock;

/** The dialog with its open state held the way the navbar holds it, so a dismissal is observable. */
const Harness = () => {
    const [open, setOpen] = useState(true);

    return <AboutDialog open={open} onOpenChange={setOpen} version="26.9.0" />;
};

describe("AboutDialog", () => {
    it("names the application, the running build and its author", async () => {
        render(<Harness />);

        expect(screen.getByRole("dialog")).toHaveAccessibleName("About");
        expect(screen.getByRole("img", { name: "App Icon" })).toBeInTheDocument();
        expect(screen.getByText(APP_NAME)).toBeInTheDocument();
        expect(await screen.findByText("Version 26.9.0")).toBeInTheDocument();
        expect(screen.getByText(APP_COPYRIGHT)).toBeInTheDocument();
    });

    it("lets the version and the copyright be selected, and nothing else", async () => {
        render(<Harness />);

        expect(await screen.findByText("Version 26.9.0")).toHaveClass("select-text");
        expect(screen.getByText(APP_COPYRIGHT)).toHaveClass("select-text");

        expect(screen.getByText(APP_NAME)).not.toHaveClass("select-text");
        expect(screen.getByRole("button", { name: "Github" })).not.toHaveClass("select-text");
    });

    it.each([
        ["Github", "repository"],
        ["vinicius.io", "website"],
    ])("sends %s by its own name", (label, link) => {
        render(<Harness />);

        fireEvent.click(screen.getByRole("button", { name: label }));

        expect(opened).toHaveBeenCalledExactlyOnceWith(link);
    });

    it("draws the links as buttons, so no click can navigate the window", () => {
        render(<Harness />);

        expect(screen.queryAllByRole("link")).toHaveLength(0);
        expect(screen.getByRole("button", { name: "Github" })).toHaveAttribute("type", "button");
    });

    it("stays open, and logs why, when a page could not be opened", async () => {
        const failure = { kind: "openLink", message: "no application is registered for https" };
        opened.mockRejectedValueOnce(failure);
        const logged = vi.spyOn(console, "error").mockImplementation(() => {});
        render(<Harness />);

        fireEvent.click(screen.getByRole("button", { name: "vinicius.io" }));

        await waitFor(() => expect(logged).toHaveBeenCalledWith(expect.any(String), failure));
        expect(screen.getByRole("dialog")).toBeInTheDocument();
    });

    it("is dismissed by its close control", () => {
        render(<Harness />);

        fireEvent.click(screen.getByRole("button", { name: "Close" }));

        expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("is dismissed by Escape", () => {
        render(<Harness />);

        fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });

        expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("is dismissed by a click outside it", async () => {
        render(<Harness />);

        // The overlay, which is what a click beside the card lands on. A whole press rather than a
        // pointerdown: Radix dismisses on the click a primary-button press ends in. And it arms its
        // listener a tick after mounting, so the press is retried until it lands on an armed layer.
        const overlay = document.querySelector("[data-slot='dialog-overlay']");
        expect(overlay).not.toBeNull();

        await waitFor(() => {
            fireEvent.pointerDown(overlay as Element);
            fireEvent.click(overlay as Element);

            expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
        });
    });
});
