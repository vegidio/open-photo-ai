import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import { MAX_QUALITY, MIN_QUALITY, type SettingsData, useSettingsStore } from "@/stores/settings";
import { DraftHarness, PUBLISHED_QUALITY, render } from "@/test/support";
import { ExportPage, parseQuality } from "./ExportPage.tsx";

// Rust's format table, answered at once: the hook's own fetch is pinned by `ipc/export.test.ts`.
vi.mock("@/hooks/useExportFormats", async () => {
    const { EXPORT_FORMATS } = await import("@/test/support");

    return { useExportFormats: () => EXPORT_FORMATS };
});

/**
 * The page, on a draft of its own - see `GeneralPage.test.tsx`. `drafted()` is what the page has
 * written since it mounted.
 */
let drafted: () => SettingsData;

const renderPage = () => {
    let latest: SettingsData;

    const result = render(
        <DraftHarness
            onDraft={(values) => {
                latest = values;
            }}
        >
            <ExportPage />
        </DraftHarness>,
    );

    drafted = () => latest;

    return result;
};

const slider = (format: string) => screen.getByRole("slider", { name: format });

const box = (format: string) => screen.getByRole<HTMLInputElement>("textbox", { name: `${format} quality` });

/** Focuses a format's box, types into it, and leaves it the way the user would: by Enter or by blur. */
const type = (format: string, text: string, leave: "Enter" | "blur") => {
    const input = box(format);

    act(() => input.focus());
    fireEvent.change(input, { target: { value: text } });

    if (leave === "Enter") fireEvent.keyDown(input, { key: "Enter" });
    else act(() => input.blur());
};

beforeEach(() => {
    localStorage.clear();
    useSettingsStore.setState(useSettingsStore.getInitialState(), true);
});

describe("the export qualities", () => {
    it("start each format at its own value, in the slider and the box", () => {
        renderPage();

        // Not one shared number: 60 in libheif is a very different picture to 60 in libjpeg.
        for (const [format, value] of [
            ["AVIF", "60"],
            ["HEIC", "60"],
            ["JPEG", "90"],
            ["WEBP", "75"],
        ] as const) {
            expect(slider(format)).toHaveAttribute("aria-valuenow", value);
            expect(box(format)).toHaveValue(value);
        }
    });

    it("draw no starting-value mark", () => {
        renderPage();

        expect(document.querySelector("[data-slot='slider-mark']")).not.toBeInTheDocument();
    });

    it("move one format without touching another", () => {
        renderPage();

        fireEvent.keyDown(slider("AVIF"), { key: "ArrowRight" });

        // Only the format moved is recorded; the rest stay at the default Rust publishes for them.
        expect(drafted().quality).toEqual({ avif: PUBLISHED_QUALITY.avif + 1 });
    });

    it("update the box as the slider moves", () => {
        renderPage();

        fireEvent.keyDown(slider("WEBP"), { key: "ArrowLeft" });

        expect(box("WEBP")).toHaveValue(String(PUBLISHED_QUALITY.webp - 1));
    });

    it("cannot be driven outside what the encoders accept", () => {
        useSettingsStore.setState({ quality: { ...PUBLISHED_QUALITY, jpeg: MAX_QUALITY, avif: MIN_QUALITY } });
        renderPage();

        fireEvent.keyDown(slider("JPEG"), { key: "ArrowRight" });
        fireEvent.keyDown(slider("AVIF"), { key: "ArrowLeft" });

        // A zero in particular is not a harmless bad setting: the native encoders take it as a real
        // request and write out a garbage image.
        expect(drafted().quality.jpeg).toBe(MAX_QUALITY);
        expect(drafted().quality.avif).toBe(MIN_QUALITY);
    });
});

describe("typing a quality", () => {
    it("moves the slider once confirmed with Enter", () => {
        renderPage();

        type("JPEG", "42", "Enter");

        expect(drafted().quality.jpeg).toBe(42);
        expect(slider("JPEG")).toHaveAttribute("aria-valuenow", "42");
    });

    it("takes no effect while it is being typed", () => {
        renderPage();
        const input = box("JPEG");

        act(() => input.focus());
        fireEvent.change(input, { target: { value: "4" } });

        // "4" on the way to "42" is not a request for 4: nothing is recorded, and JPEG stays at its default.
        expect(drafted().quality.jpeg).toBeUndefined();
        expect(input).toHaveValue("4");
    });

    it("brings a value above the bounds to the bound when the box is left", () => {
        renderPage();

        type("HEIC", "250", "blur");

        expect(drafted().quality.heic).toBe(MAX_QUALITY);
        expect(box("HEIC")).toHaveValue(String(MAX_QUALITY));
    });

    it("goes back to the value it held when cleared and left", () => {
        useSettingsStore.setState({ quality: { ...PUBLISHED_QUALITY, avif: 33 } });
        renderPage();

        type("AVIF", "", "blur");

        expect(drafted().quality.avif).toBe(33);
        expect(box("AVIF")).toHaveValue("33");
    });

    it("goes back to the value it held when left holding something that is not a number", () => {
        renderPage();

        type("WEBP", "high", "Enter");

        expect(drafted().quality.webp).toBeUndefined();
        expect(box("WEBP")).toHaveValue(String(PUBLISHED_QUALITY.webp));
    });
});

describe("parseQuality", () => {
    it.each([
        ["42", 42],
        [" 42 ", 42],
        ["42.6", 43],
        ["0", MIN_QUALITY],
        ["-5", MIN_QUALITY],
        ["250", MAX_QUALITY],
        ["", undefined],
        ["   ", undefined],
        ["high", undefined],
        ["Infinity", undefined],
    ])("reads %j as %s", (text, expected) => {
        expect(parseQuality(text)).toBe(expected);
    });
});
