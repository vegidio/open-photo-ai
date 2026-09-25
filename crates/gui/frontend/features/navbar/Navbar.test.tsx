import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import "@/i18n";
import { isOutdated } from "@/ipc/app";
import { openLink } from "@/ipc/links";
import { isMacOs } from "@/ipc/os";
import { APP_NAME } from "@/lib/constants";
import { track } from "@/lib/faro";
import { useFileStore } from "@/stores/files";
import { HOLIDAY, openFiles, render, resetFileStore, SUNSET } from "@/test/support";
import { Navbar } from "./Navbar";

// Both IPC wrappers rather than their transports: `ipc/app.test.ts` and `ipc/os.test.ts` already pin
// the wire name and the global respectively, so this file is only about what the navbar does with
// the answers. `@/ipc/os` in particular *has* to be mocked - the real `platform()` reads a global
// the plugin's init script installs, which does not exist in jsdom.
vi.mock("@/ipc/app", () => ({
    appVersion: vi.fn(() => Promise.resolve("26.9.0")),
    isOutdated: vi.fn(() => Promise.resolve(false)),
}));
vi.mock("@/ipc/links", () => ({ openLink: vi.fn(() => Promise.resolve()) }));
// Mocked at `lib/faro.ts`'s own boundary: what `track` does with an event is pinned by `faro.test.ts`.
vi.mock("@/lib/faro", () => ({
    track: vi.fn(),
    sendError: vi.fn(),
    pauseFaro: vi.fn(),
    // Untraced, as before Faro starts: the request goes out exactly as `invoke` alone would send it.
    traced: (_name: string, send: () => Promise<unknown>) => send(),
}));
vi.mock("@/ipc/os", () => ({ isMacOs: vi.fn(() => false), isWindows: vi.fn(() => false) }));
// Closing an image releases its enhanced result in Rust, and there is no Rust behind jsdom.
vi.mock("@/ipc/enhance", async (importOriginal) => ({
    ...(await importOriginal<typeof import("@/ipc/enhance")>()),
    releaseEnhanced: vi.fn(() => Promise.resolve()),
    releaseAllEnhanced: vi.fn(() => Promise.resolve()),
}));

const onMacOs = isMacOs as unknown as Mock;
const checked = isOutdated as unknown as Mock;
const opened = openLink as unknown as Mock;

describe("Navbar", () => {
    beforeEach(() => {
        onMacOs.mockReturnValue(false);
        resetFileStore();
    });

    it("names the application", () => {
        render(<Navbar />);

        // Against the constant rather than a second copy of the string: a test spelling out
        // "Open Photo AI" would keep passing if the constant were renamed out from under it.
        expect(screen.getByText(APP_NAME)).toBeInTheDocument();
    });

    it("renders the version the core library reports", async () => {
        render(<Navbar />);

        expect(await screen.findByText("v26.9.0")).toBeInTheDocument();
    });

    it("leaves room for the traffic lights on macOS", () => {
        onMacOs.mockReturnValue(true);

        const { container } = render(<Navbar />);

        expect(container.querySelector("header")).toHaveClass("pl-[86px]");
    });

    it("reserves no space anywhere else", () => {
        const { container } = render(<Navbar />);

        expect(container.querySelector("header")).not.toHaveClass("pl-[86px]");
    });

    it("is the window's drag handle", () => {
        const { container } = render(<Navbar />);

        expect(container.querySelector("header")).toHaveAttribute("data-tauri-drag-region");
    });

    it("reports the current image's name and its pixel size", () => {
        openFiles(HOLIDAY);

        render(<Navbar />);

        expect(screen.getByText("holiday.png")).toBeInTheDocument();
        expect(screen.getByText("Dimensions")).toBeInTheDocument();
        expect(screen.getByText("3000 x 2000")).toBeInTheDocument();
    });

    it("divides the application's name from the file's, and the dimensions from Settings", () => {
        openFiles(HOLIDAY);

        const { container } = render(<Navbar />);

        // Both rules the design draws, and the width that makes them visible. `orientation="vertical"`
        // sets no width of its own, so a separator carrying only a height is an element 0px wide - a
        // divider nobody can see. Asserting the class rather than the box because jsdom lays nothing
        // out; the variant-qualified height is what the caller has to write to out-specify the
        // component's own `h-full`.
        const dividers = container.querySelectorAll("[data-slot='separator'][data-orientation='vertical']");

        expect(dividers).toHaveLength(2);

        for (const divider of dividers) {
            expect(divider).toHaveClass("data-[orientation=vertical]:w-px");
            expect(divider).toHaveClass("data-[orientation=vertical]:h-5");
        }
    });

    it("reports neither while no image is open", () => {
        const { container } = render(<Navbar />);

        expect(screen.queryByText("Dimensions")).not.toBeInTheDocument();
        expect(container.querySelector("[data-slot='navbar-dimensions']")).toBeNull();
    });

    it("names the file rather than the folders above it", () => {
        // Several folders deep, and with spaces in the name: the navbar answers which photograph, and
        // a path that long would push Settings off the window.
        openFiles(SUNSET);

        render(<Navbar />);

        expect(screen.getByText("sunset over the harbour.nef")).toBeInTheDocument();
        expect(screen.queryByText(SUNSET.path)).not.toBeInTheDocument();
    });

    it("says nothing about the size of a photograph it could not measure", () => {
        // A file whose header could not be read is still reported - the user named it - but there is
        // no size to state, and "undefined x undefined" would be worse than the block's absence.
        const { width, height, ...unmeasured } = HOLIDAY;
        openFiles(unmeasured);

        render(<Navbar />);

        expect(screen.getByText("holiday.png")).toBeInTheDocument();
        expect(screen.queryByText("Dimensions")).not.toBeInTheDocument();
    });
});

describe("the navbar's file options control", () => {
    beforeEach(resetFileStore);

    it("appears beside the name only while an image is open", async () => {
        render(<Navbar />);

        // Nothing open: the navbar names no file, so there is nothing to offer options for.
        expect(screen.queryByRole("button", { name: /^File options/ })).not.toBeInTheDocument();

        act(() => openFiles(HOLIDAY));

        expect(await screen.findByRole("button", { name: "File options for holiday.png" })).toBeInTheDocument();
    });

    it("acts on the current image", async () => {
        // The navbar names the current image, so its menu acts on that one - which is the half of the
        // contract a thumbnail's menu inverts.
        openFiles(HOLIDAY, SUNSET);
        act(() => useFileStore.getState().setCurrentIndex(1));
        render(<Navbar />);

        fireEvent.keyDown(screen.getByRole("button", { name: "File options for sunset over the harbour.nef" }), {
            key: "Enter",
        });
        fireEvent.click(await screen.findByRole("menuitem", { name: "Close image" }));

        await waitFor(() => expect(useFileStore.getState().files.map((f) => f.path)).toEqual([HOLIDAY.path]));
    });

    it("offers the same three actions the drawer's does", async () => {
        openFiles(HOLIDAY);
        render(<Navbar />);

        fireEvent.keyDown(screen.getByRole("button", { name: "File options for holiday.png" }), { key: "Enter" });

        const items = await screen.findAllByRole("menuitem");
        expect(items.map((item) => item.textContent)).toEqual([
            "Close image",
            "Close all images",
            "Show in File Manager",
        ]);
    });

    it.each([
        ["with nothing open", []],
        ["with an image open", [HOLIDAY]],
    ])("opens About %s", async (_, files) => {
        openFiles(...files);
        render(<Navbar />);

        expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

        fireEvent.click(screen.getByRole("button", { name: "About" }));

        expect(await screen.findByRole("dialog", { name: "About" })).toBeInTheDocument();
    });
});

describe("the navbar's update button", () => {
    const UPDATE = "Update Available";

    beforeEach(() => {
        checked.mockReset();
        opened.mockReset();
        opened.mockResolvedValue(undefined);
        resetFileStore();
    });

    it("is absent until the check has answered", async () => {
        let answer: (outdated: boolean) => void = () => {};
        checked.mockReturnValue(new Promise<boolean>((resolve) => (answer = resolve)));
        render(<Navbar />);

        // The version arriving first shows the rest of the navbar has rendered without waiting on it.
        await screen.findByText("v26.9.0");
        expect(screen.queryByRole("button", { name: UPDATE })).not.toBeInTheDocument();

        await act(async () => answer(true));

        expect(screen.getByRole("button", { name: UPDATE })).toBeInTheDocument();
    });

    it("is absent when no newer release is published", async () => {
        checked.mockResolvedValue(false);
        render(<Navbar />);

        await screen.findByText("v26.9.0");
        await waitFor(() => expect(checked).toHaveBeenCalled());

        expect(screen.queryByRole("button", { name: UPDATE })).not.toBeInTheDocument();
    });

    it("is absent, and logs why, when the check could not be asked", async () => {
        const failure = new Error("the IPC bridge is gone");
        checked.mockRejectedValue(failure);
        const logged = vi.spyOn(console, "error").mockImplementation(() => {});
        render(<Navbar />);

        await waitFor(() => expect(logged).toHaveBeenCalledWith(expect.any(String), failure));

        expect(screen.queryByRole("button", { name: UPDATE })).not.toBeInTheDocument();
        logged.mockRestore();
    });

    it("pulses after the version when a newer release is published", async () => {
        checked.mockResolvedValue(true);
        render(<Navbar />);

        const button = await screen.findByRole("button", { name: UPDATE });
        const version = await screen.findByText("v26.9.0");

        expect(button).toHaveClass("animate-pulse");
        expect(version.compareDocumentPosition(button) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    });

    it("opens the releases page by name", async () => {
        checked.mockResolvedValue(true);
        render(<Navbar />);

        fireEvent.click(await screen.findByRole("button", { name: UPDATE }));

        expect(opened).toHaveBeenCalledWith("releases");
        expect(track).toHaveBeenCalledExactlyOnceWith("update_opened", {});
    });

    it("stays, and logs why, when the page could not be opened", async () => {
        const failure = { kind: "openLink", message: "no application is registered for https" };
        checked.mockResolvedValue(true);
        opened.mockRejectedValueOnce(failure);
        const logged = vi.spyOn(console, "error").mockImplementation(() => {});
        render(<Navbar />);

        fireEvent.click(await screen.findByRole("button", { name: UPDATE }));

        await waitFor(() => expect(logged).toHaveBeenCalledWith(expect.any(String), failure));
        expect(screen.getByRole("button", { name: UPDATE })).toBeInTheDocument();
        logged.mockRestore();
    });
});
