import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { toast } from "sonner";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import "@/i18n";
import { revealExport } from "@/ipc/export";
import { useEnhancementStore } from "@/stores/enhancements";
import { type Row, useExportBatchStore } from "@/stores/exportBatch";
import { useExportSettingsStore } from "@/stores/exportSettings";
import { HOLIDAY, openFiles, render, resetFileStore, SUNSET } from "@/test/support";
import { ExportQueue } from "./ExportQueue";
import { ExportRow } from "./ExportRow";

vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));
vi.mock("@/ipc/export", () => ({ revealExport: vi.fn(() => Promise.resolve()), cancelExport: vi.fn() }));
vi.mock("@/ipc/os", () => ({ isMacOs: () => true, isWindows: () => false }));
vi.mock("@/ipc/enhance", async (importOriginal) => ({
    ...(await importOriginal<typeof import("@/ipc/enhance")>()),
    releaseEnhanced: vi.fn(() => Promise.resolve()),
    releaseAllEnhanced: vi.fn(() => Promise.resolve()),
}));

const revealed = revealExport as unknown as Mock;

const at = (row: Row) => act(() => useExportBatchStore.getState().setStage(HOLIDAY.path, row));
const pill = () => document.querySelector("[data-slot='export-stage']");

const renderRow = () => render(<ExportRow path={HOLIDAY.path} />);

beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    resetFileStore();
    openFiles(HOLIDAY, SUNSET);
    useEnhancementStore.setState({ autopilot: false, enhancements: new Map() });
    useExportSettingsStore.setState(useExportSettingsStore.getInitialState(), true);
    useExportBatchStore.setState(useExportBatchStore.getInitialState(), true);
    useExportBatchStore.getState().open([HOLIDAY.path, SUNSET.path]);
});

describe("a row", () => {
    it("names the file it will write, with its dimensions before and after and its types", () => {
        useExportSettingsStore.setState({ prefix: "new-", suffix: "-opai", format: "webp" });
        useEnhancementStore
            .getState()
            .addEnhancement(HOLIDAY.path, { family: "upscale", codename: "kyoto", precision: "fp32", scale: 2 });

        renderRow();

        expect(screen.getByText("new-holiday-opai.webp")).toBeInTheDocument();
        expect(screen.getByText(/3000 x 2000/)).toBeInTheDocument();
        expect(screen.getByText("6000 x 4000")).toBeInTheDocument();
        expect(screen.getByText(/PNG → WEBP/)).toBeInTheDocument();
        expect(screen.getByText("8.03 MB")).toBeInTheDocument();
    });

    it("follows the settings while idle", () => {
        renderRow();
        expect(screen.getByText("holiday.png")).toBeInTheDocument();

        act(() => useExportSettingsStore.getState().setFormat("jpeg"));

        expect(screen.getByText("holiday.jpg")).toBeInTheDocument();
    });

    it.each([
        [{ stage: "queued" }, "Queued"],
        [{ stage: "analysing" }, "Analysing"],
        [{ stage: "enhancing", fraction: 0.5 }, "Enhancing"],
        [{ stage: "writing" }, "Writing"],
        [{ stage: "done", path: "/h.png", bytes: 2048 }, "Done"],
        [{ stage: "failed", reason: "no" }, "Failed"],
    ] as [Row, string][])("reads its stage: %j", async (row, label) => {
        renderRow();
        await at(row);

        expect(pill()).toHaveTextContent(label);
    });

    it("shows a bar of its own only while it is the row being worked on", async () => {
        renderRow();
        expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();

        act(() => useExportBatchStore.getState().setCurrent({ path: HOLIDAY.path, run: "export-1" }));
        await at({ stage: "enhancing", fraction: 0.4 });

        expect(screen.getByRole("progressbar")).toBeInTheDocument();

        act(() => useExportBatchStore.getState().setCurrent({ path: SUNSET.path }));
        expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    });

    it("scrolls itself into view when it becomes the row being worked on", () => {
        const scrolled = vi.spyOn(Element.prototype, "scrollIntoView");
        render(<ExportRow path={HOLIDAY.path} follow />);
        expect(scrolled).not.toHaveBeenCalled();

        act(() => useExportBatchStore.getState().setCurrent({ path: HOLIDAY.path, run: "export-1" }));

        expect(scrolled).toHaveBeenCalledOnce();
        expect(scrolled.mock.contexts[0]).toHaveAttribute("data-slot", "export-row");
        scrolled.mockRestore();
    });
});

describe("a written row", () => {
    it("shows the size written and reveals the path actually written", async () => {
        renderRow();
        expect(screen.queryByRole("button", { name: "Show in Finder" })).not.toBeInTheDocument();

        await at({ stage: "done", path: "/Users/someone/Pictures/holiday_1.png", bytes: 2048 });
        expect(screen.getByText("2.00 KB")).toBeInTheDocument();

        fireEvent.click(screen.getByRole("button", { name: "Show in Finder" }));

        expect(revealed).toHaveBeenCalledExactlyOnceWith("/Users/someone/Pictures/holiday_1.png");
    });

    it("says the file is still saved where a reveal is refused", async () => {
        const notice = vi.spyOn(toast, "error");
        vi.spyOn(console, "error").mockImplementation(() => {});
        revealed.mockRejectedValueOnce({ kind: "revealExport", message: "no" });

        renderRow();
        await at({ stage: "done", path: "/h.png", bytes: 1 });
        fireEvent.click(screen.getByRole("button", { name: "Show in Finder" }));

        await waitFor(() =>
            expect(notice).toHaveBeenCalledExactlyOnceWith(
                "Couldn't open the file manager. The file is still saved at its export location.",
            ),
        );
    });
});

describe("a failed row", () => {
    const reason = "Couldn't write holiday.png to /Users/someone/Pictures. Permission denied (os error 13)";

    it("shows why on hover, copies it on click, and confirms the copy until the pointer leaves", async () => {
        const writeText = vi.fn(() => Promise.resolve());
        Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });

        renderRow();
        await at({ stage: "failed", reason });
        const failed = screen.getByRole("button", { name: /Failed/ });

        fireEvent.pointerEnter(failed);
        expect(within(await screen.findByRole("tooltip")).getByText(reason)).toBeInTheDocument();

        fireEvent.click(failed);
        expect(writeText).toHaveBeenCalledExactlyOnceWith(reason);
        expect(within(await screen.findByRole("tooltip")).getByText("Copied to the clipboard")).toBeInTheDocument();

        fireEvent.pointerLeave(failed);
        fireEvent.pointerEnter(failed);
        expect(within(await screen.findByRole("tooltip")).getByText(reason)).toBeInTheDocument();
    });

    it("keeps showing the reason where the clipboard refuses", async () => {
        const writeText = vi.fn(() => Promise.reject(new Error("denied")));
        Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
        const logged = vi.spyOn(console, "error").mockImplementation(() => {});

        renderRow();
        await at({ stage: "failed", reason });
        const failed = screen.getByRole("button", { name: /Failed/ });

        fireEvent.pointerEnter(failed);
        fireEvent.click(failed);

        await waitFor(() => expect(logged).toHaveBeenCalled());
        expect(within(screen.getByRole("tooltip")).getByText(reason)).toBeInTheDocument();
    });
});

describe("the queue", () => {
    it("draws one row per queued path, in queue order, and says the order is the pipeline's", () => {
        useExportBatchStore.getState().open([SUNSET.path, HOLIDAY.path]);

        render(<ExportQueue />);

        expect(screen.getByText("Queue (2)")).toBeInTheDocument();
        expect(screen.getByText("in pipeline order")).toBeInTheDocument();
        expect(screen.getAllByRole("img").map((image) => image.getAttribute("alt"))).toEqual([
            "sunset over the harbour.png",
            "holiday.png",
        ]);
    });

    it("follows the row being worked on until it is scrolled by hand, and again once the next run starts", () => {
        const scrolled = vi.spyOn(Element.prototype, "scrollIntoView");
        const batch = useExportBatchStore.getState();
        render(<ExportQueue />);
        const list = document.querySelector("[data-slot='export-row']")?.parentElement as HTMLElement;

        act(() => batch.start());
        act(() => batch.setCurrent({ path: HOLIDAY.path }));
        expect(scrolled).toHaveBeenCalledOnce();

        fireEvent.wheel(list);
        act(() => batch.setCurrent({ path: SUNSET.path }));
        expect(scrolled).toHaveBeenCalledOnce();

        act(() => batch.finish());
        act(() => batch.reset());
        act(() => batch.start());
        act(() => batch.setCurrent({ path: HOLIDAY.path }));
        expect(scrolled).toHaveBeenCalledTimes(2);
        scrolled.mockRestore();
    });
});
