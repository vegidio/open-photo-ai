import { act, screen } from "@testing-library/react";
import { toast } from "sonner";
import { afterEach, describe, expect, it } from "vitest";
import { DRAWER_BLEEDING, DRAWER_HEIGHT } from "@/lib/constants";
import { useDrawerStore } from "@/stores/drawer";
import { render } from "@/test/support";

/**
 * The toaster is mounted once, for the whole application.
 *
 * `toast()` is a function call rather than an element, so it goes to whatever toaster is mounted:
 * a test that mounted its own would pass while production had none, and the feature tests that
 * assert on a message would each be asserting against their own copy. Rendering through `render`
 * - which wraps in `AppProviders`, the tree that ships - is what makes this a check on production's
 * own mounting.
 */
describe("AppProviders", () => {
    // Sonner's store is module-level and outlives the render, replaying anything undismissed to the
    // next toaster that subscribes - so a notice raised here would otherwise follow the suite around.
    afterEach(() => {
        toast.dismiss();
        useDrawerStore.setState(useDrawerStore.getInitialState(), true);
    });

    /** Sonner mounts the region with the first notice rather than at render, so one is always raised. */
    const raise = async (message: string) => {
        await act(async () => {
            toast(message);
        });
        await screen.findByText(message);

        return document.querySelector<HTMLElement>("[data-sonner-toaster]");
    };

    it("shows a raised toast once", async () => {
        render(<span />);

        await act(async () => {
            toast.error("Couldn't open the log file.");
        });

        expect(await screen.findAllByText("Couldn't open the log file.")).toHaveLength(1);
    });

    /**
     * Where notices land, which is a property of the window rather than of any one of them.
     *
     * The bottom right corner sonner defaults to is over the drawer, which runs the full width of the
     * canvas - so both halves of this matter and neither is worth stating without the other. The
     * offset is compared against the drawer's own constant rather than against 72, so the test says
     * what the rule is instead of restating today's arithmetic.
     */
    it("floats notices above the folded drawer, centred", async () => {
        render(<span />);

        const toaster = await raise("Two images were added.");

        expect(toaster).toHaveAttribute("data-y-position", "bottom");
        expect(toaster).toHaveAttribute("data-x-position", "center");
        expect(toaster?.style.getPropertyValue("--offset-bottom")).toBe(`${DRAWER_BLEEDING + 24}px`);
        expect(toaster?.style.getPropertyValue("--mobile-offset-bottom")).toBe(`${DRAWER_BLEEDING + 24}px`);
    });

    /**
     * The other half of the same rule: the drawer covers two different amounts of the window, and a
     * notice that cleared only the folded header would be behind the strip whenever it was showing.
     *
     * Compared against the two constants rather than against 200, and derived from both of them
     * rather than from one: the reference pairs the same two numbers in its `useNotify`, for the
     * reason its comment gives - change one without the other and the notice either overlaps the
     * drawer or floats above nothing.
     */
    it("clears the unfolded drawer's body as well", async () => {
        render(<span />);
        act(() => useDrawerStore.getState().setOpen(true));

        const toaster = await raise("Two images were added.");

        expect(toaster?.style.getPropertyValue("--offset-bottom")).toBe(`${DRAWER_HEIGHT + DRAWER_BLEEDING + 24}px`);
        expect(toaster?.style.getPropertyValue("--mobile-offset-bottom")).toBe(
            `${DRAWER_HEIGHT + DRAWER_BLEEDING + 24}px`,
        );
    });

    it("moves a notice already on screen when the drawer unfolds under it", async () => {
        render(<span />);
        await raise("Two images were added.");

        act(() => useDrawerStore.getState().setOpen(true));

        // Sonner re-reads the offset on each render, so the notice rides up with the strip rather
        // than being left sitting behind it.
        const toaster = document.querySelector<HTMLElement>("[data-sonner-toaster]");
        expect(toaster?.style.getPropertyValue("--offset-bottom")).toBe(`${DRAWER_HEIGHT + DRAWER_BLEEDING + 24}px`);
    });

    /**
     * That a notice is drawn in the colour of what it is reporting, rather than in one card for every
     * type.
     *
     * jsdom applies no stylesheet, so what is assertable is the pair sonner's own rules key off -
     * the type it was raised as, and that it is allowed to colour by it - plus the variables this
     * application points at its palette. A `data-rich-colors` of `false` is exactly the regression
     * worth catching: every notice black, whatever went wrong.
     */
    it("colours a warning in the palette's warning tokens", async () => {
        render(<span />);

        await act(async () => {
            toast.warning("The file “invoice.pdf” is not supported.");
        });

        const raised = (await screen.findByText("The file “invoice.pdf” is not supported.")).closest(
            "[data-sonner-toast]",
        );
        const toaster = document.querySelector<HTMLElement>("[data-sonner-toaster]");

        expect(raised).toHaveAttribute("data-type", "warning");
        expect(raised).toHaveAttribute("data-rich-colors", "true");
        expect(toaster?.style.getPropertyValue("--warning-text")).toBe("var(--warning)");
    });
});
