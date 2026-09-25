import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import "@/i18n";
import { pickDirectory } from "@/ipc/export";
import type { ImageRecord } from "@/ipc/images";
import { useEnhancementStore } from "@/stores/enhancements";
import { useExportBatchStore } from "@/stores/exportBatch";
import { useExportSettingsStore } from "@/stores/exportSettings";
import { useFileStore } from "@/stores/files";
import { DEFAULT_QUALITY, useSettingsStore } from "@/stores/settings";
import { HOLIDAY, openFiles, render, resetFileStore, resetSettingsStore, SUNSET } from "@/test/support";
import { runBatch } from "./batch";
import { ExportDialog } from "./ExportDialog";

vi.mock("@tauri-apps/api/core", () => ({
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));
vi.mock("@/ipc/export", () => ({
    pickDirectory: vi.fn(),
    revealExport: vi.fn(() => Promise.resolve()),
    cancelExport: vi.fn(() => Promise.resolve()),
    onExportProgress: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock("@/ipc/os", () => ({ isMacOs: () => false, isWindows: () => false }));
vi.mock("@/ipc/enhance", async (importOriginal) => ({
    ...(await importOriginal<typeof import("@/ipc/enhance")>()),
    releaseEnhanced: vi.fn(() => Promise.resolve()),
    releaseAllEnhanced: vi.fn(() => Promise.resolve()),
}));
// The loop has its own tests; here it is what Save starts.
vi.mock("./batch", async (importOriginal) => ({
    ...(await importOriginal<typeof import("./batch")>()),
    runBatch: vi.fn(() => Promise.resolve()),
}));

const picked = pickDirectory as unknown as Mock;
const ran = runBatch as unknown as Mock;

const PORTRAIT: ImageRecord = { ...HOLIDAY, path: "/Users/someone/Pictures/portrait.jpg", extension: "jpg" };
const LANDSCAPE: ImageRecord = { ...HOLIDAY, path: "/Users/someone/Pictures/landscape.jpeg", extension: "jpeg" };

const onClose = vi.fn();
const mount = () => render(<ExportDialog onClose={onClose} />);
const batch = () => useExportBatchStore.getState();
const button = (name: string) => screen.getByRole("button", { name });
const choose = async (combobox: string, option: string) => {
    fireEvent.keyDown(screen.getByRole("combobox", { name: combobox }), { key: "Enter" });
    fireEvent.click(await screen.findByRole("option", { name: option }));
};
const quality = () => screen.queryByRole("slider", { name: "Quality" });
const note = () => document.querySelector("[data-slot='overwrite-note']");

/** Picks exactly these files. */
const pick = (...files: ImageRecord[]) =>
    useFileStore.setState({ selectedPaths: new Set(files.map((file) => file.path)) });

beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    resetSettingsStore();
    resetFileStore();
    openFiles(HOLIDAY, SUNSET, PORTRAIT, LANDSCAPE);
    useEnhancementStore.setState({ autopilot: false, enhancements: new Map() });
    useExportSettingsStore.setState(useExportSettingsStore.getInitialState(), true);
    useExportBatchStore.setState(useExportBatchStore.getInitialState(), true);
});

describe("the queue it opens with", () => {
    it("is the picked photographs, in drawer order grouped by chain, and nothing else", () => {
        const upscale = { family: "upscale", codename: "kyoto", precision: "fp32", scale: 2 } as const;
        useEnhancementStore.getState().addEnhancement(SUNSET.path, upscale);
        useEnhancementStore.getState().addEnhancement(LANDSCAPE.path, upscale);
        pick(HOLIDAY, SUNSET, PORTRAIT, LANDSCAPE);

        mount();

        expect(batch().queue).toEqual([HOLIDAY.path, PORTRAIT.path, SUNSET.path, LANDSCAPE.path]);
    });

    it("does not change when the picked set does behind it", () => {
        pick(HOLIDAY, SUNSET);
        mount();

        act(() => useFileStore.getState().selectAll());

        expect(batch().queue).toEqual([HOLIDAY.path, SUNSET.path]);
    });

    it("is summarised as ready with nothing written", () => {
        pick(HOLIDAY, SUNSET);
        mount();

        expect(screen.getByText("2 files ready · nothing written yet")).toBeInTheDocument();
    });
});

describe("the summary", () => {
    it("reads what is written and what remains while running, and the failures once ended", () => {
        pick(HOLIDAY, SUNSET, PORTRAIT, LANDSCAPE);
        mount();

        act(() => {
            batch().start();
            batch().setStage(HOLIDAY.path, { stage: "done", path: "/h.png", bytes: 1 });
            batch().setCurrent({ path: SUNSET.path, run: "export-2" });
            batch().setStage(SUNSET.path, { stage: "enhancing", fraction: 0.5 });
        });
        expect(screen.getByText("1 of 4 written · 2 remaining")).toBeInTheDocument();

        act(() => {
            batch().setStage(SUNSET.path, { stage: "failed", reason: "no" });
            batch().setStage(PORTRAIT.path, { stage: "done", path: "/p.png", bytes: 1 });
            batch().setStage(LANDSCAPE.path, { stage: "done", path: "/l.png", bytes: 1 });
            batch().finish();
        });
        expect(screen.getByText("3 of 4 written, 1 failed")).toBeInTheDocument();
    });
});

describe("closing", () => {
    it("works by Escape and by the close box while idle", () => {
        pick(HOLIDAY);
        mount();

        fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
        fireEvent.click(button("Close"));

        expect(onClose).toHaveBeenCalledTimes(2);
    });

    it("is refused while running: no close box, and Escape does nothing", () => {
        pick(HOLIDAY);
        mount();
        act(() => batch().start());

        expect(screen.queryByRole("button", { name: "Close" })).not.toBeInTheDocument();
        fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });

        expect(onClose).not.toHaveBeenCalled();
    });

    it("works again once the batch has ended", () => {
        pick(HOLIDAY);
        mount();
        act(() => {
            batch().start();
            batch().finish();
        });

        fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });

        expect(onClose).toHaveBeenCalledOnce();
    });

    it("never happens on a click outside", () => {
        pick(HOLIDAY);
        mount();

        fireEvent.pointerDown(document.body);

        expect(onClose).not.toHaveBeenCalled();
    });

    it("aborts a batch still running when the dialog goes away", () => {
        pick(HOLIDAY);
        const { unmount } = mount();
        act(() => batch().start());

        unmount();

        expect(batch().aborted).toBe(true);
    });
});

describe("the buttons", () => {
    it("read Cancel and Save while idle, Abort and a disabled Save while running, Close and Export again once ended", () => {
        pick(HOLIDAY);
        mount();

        expect(button("Cancel")).toBeEnabled();
        expect(button("Save")).toBeEnabled();

        act(() => batch().start());
        expect(button("Abort")).toBeEnabled();
        expect(button("Save")).toBeDisabled();

        act(() => batch().finish());
        // The close box is back too, so the footer's is the second of two.
        expect(screen.getAllByRole("button", { name: "Close" })).toHaveLength(2);
        expect(button("Export again")).toBeEnabled();
    });

    it("Abort marks the batch aborted", () => {
        pick(HOLIDAY);
        mount();
        act(() => batch().start());

        fireEvent.click(button("Abort"));

        expect(batch().aborted).toBe(true);
    });

    it("Export again puts every row back to Queued and starts nothing", () => {
        pick(HOLIDAY, SUNSET);
        mount();
        act(() => {
            batch().start();
            batch().setStage(HOLIDAY.path, { stage: "done", path: "/h.png", bytes: 1 });
            batch().finish();
        });

        fireEvent.click(button("Export again"));

        expect(batch().phase).toBe("idle");
        expect(batch().rows.get(HOLIDAY.path)).toEqual({ stage: "queued" });
        expect(ran).not.toHaveBeenCalled();
        expect(button("Save")).toBeEnabled();
    });

    it("Cancel closes", () => {
        pick(HOLIDAY);
        mount();

        fireEvent.click(button("Cancel"));

        expect(onClose).toHaveBeenCalledOnce();
    });
});

describe("the settings", () => {
    it("are unavailable while running", () => {
        pick(HOLIDAY);
        useExportSettingsStore.getState().setFormat("webp");
        mount();
        act(() => batch().start());

        expect(screen.getByRole("textbox", { name: "Prefix" })).toBeDisabled();
        expect(screen.getByRole("textbox", { name: "Suffix" })).toBeDisabled();
        expect(screen.getByRole("combobox", { name: "Save to" })).toBeDisabled();
        expect(screen.getByRole("combobox", { name: "Format" })).toBeDisabled();
        expect(screen.getByRole("switch")).toBeDisabled();
        expect(quality()).toHaveAttribute("data-disabled");
    });

    it("write the affixes as they are typed", () => {
        pick(HOLIDAY);
        mount();

        fireEvent.change(screen.getByRole("textbox", { name: "Suffix" }), { target: { value: "-opai" } });

        expect(useExportSettingsStore.getState().suffix).toBe("-opai");
        expect(screen.getByText("holiday-opai.png")).toBeInTheDocument();
    });

    it("swap the footnote for the warning when overwriting is switched on", () => {
        pick(HOLIDAY);
        mount();
        expect(note()).toHaveTextContent("new files get a number");

        fireEvent.click(screen.getByRole("switch"));

        expect(note()).toHaveTextContent("will be overwritten");
        expect(note()).toHaveClass("text-warning");
        expect(useExportSettingsStore.getState().overwrite).toBe(true);
    });
});

describe("Save to", () => {
    it("shows a browsed folder, and keeps it when the picker is then dismissed", async () => {
        pick(HOLIDAY);
        mount();

        picked.mockResolvedValueOnce("/exports");
        await choose("Save to", "Browse...");
        await waitFor(() => expect(useExportSettingsStore.getState().location).toBe("/exports"));
        expect(screen.getByRole("combobox", { name: "Save to" })).toHaveTextContent("/exports");
        expect(picked).toHaveBeenCalledWith("Select Directory");

        picked.mockResolvedValueOnce(undefined);
        await choose("Save to", "Browse...");
        await waitFor(() => expect(picked).toHaveBeenCalledTimes(2));

        expect(useExportSettingsStore.getState().location).toBe("/exports");
    });

    it("goes back to the original directory", async () => {
        useExportSettingsStore.getState().setLocation("/exports");
        pick(HOLIDAY);
        mount();

        await choose("Save to", "Original directory");

        expect("location" in useExportSettingsStore.getState()).toBe(false);
    });
});

describe("the quality", () => {
    it("is shown for one lossy format, at that format's quality from Settings", () => {
        useSettingsStore.getState().apply({ quality: { ...DEFAULT_QUALITY, webp: 64 } });
        useExportSettingsStore.getState().setFormat("webp");
        pick(HOLIDAY, SUNSET);
        mount();

        expect(quality()).toHaveAttribute("aria-valuenow", "64");
    });

    it("is shown under Preserve where every file is the same lossy format", () => {
        useExportSettingsStore.getState().setFormat("preserve");
        pick(PORTRAIT, LANDSCAPE);
        mount();

        expect(quality()).toHaveAttribute("aria-valuenow", String(DEFAULT_QUALITY.jpeg));
    });

    it("is hidden under a mixed Preserve and for a lossless format", async () => {
        useExportSettingsStore.getState().setFormat("preserve");
        pick(PORTRAIT, HOLIDAY);
        mount();
        expect(quality()).not.toBeInTheDocument();

        act(() => useExportSettingsStore.getState().setFormat("png"));
        expect(quality()).not.toBeInTheDocument();
    });

    it("is a draft that closing discards", () => {
        useExportSettingsStore.getState().setFormat("webp");
        pick(HOLIDAY);
        mount();

        fireEvent.keyDown(quality() as HTMLElement, { key: "ArrowRight" });
        expect(quality()).toHaveAttribute("aria-valuenow", String(DEFAULT_QUALITY.webp + 1));

        fireEvent.click(button("Cancel"));

        expect(useSettingsStore.getState().quality.webp).toBe(DEFAULT_QUALITY.webp);
        expect(ran).not.toHaveBeenCalled();
    });

    it("is committed to Settings by Save, which runs the batch at it with the settings in force", () => {
        useExportSettingsStore.getState().setFormat("webp");
        useExportSettingsStore.getState().setSuffix("-opai");
        pick(HOLIDAY);
        mount();

        fireEvent.keyDown(quality() as HTMLElement, { key: "ArrowLeft" });
        fireEvent.click(button("Save"));

        const committed = { ...DEFAULT_QUALITY, webp: DEFAULT_QUALITY.webp - 1 };
        expect(useSettingsStore.getState().quality).toEqual(committed);
        expect(ran).toHaveBeenCalledExactlyOnceWith(
            { prefix: "", suffix: "-opai", overwrite: false, format: "webp" },
            committed,
        );
    });

    it("commits only the format the slider stands for at Save, not a draft left on another", () => {
        useExportSettingsStore.getState().setFormat("webp");
        pick(HOLIDAY);
        mount();

        fireEvent.keyDown(quality() as HTMLElement, { key: "ArrowLeft" });
        act(() => useExportSettingsStore.getState().setFormat("jpeg"));
        fireEvent.keyDown(quality() as HTMLElement, { key: "ArrowRight" });
        fireEvent.click(button("Save"));

        const committed = { ...DEFAULT_QUALITY, jpeg: DEFAULT_QUALITY.jpeg + 1 };
        expect(useSettingsStore.getState().quality).toEqual(committed);
        expect(ran).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ format: "jpeg" }), committed);
    });
});
