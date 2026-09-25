import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import "@/i18n";
import i18n from "@/i18n";
import type { SupportedProviders } from "@/ipc/setup";
import { type SettingsData, useSettingsStore } from "@/stores/settings";
import { useSetupStore } from "@/stores/setup";
import { DraftHarness, PROVIDERS, render, resetSetupStore } from "@/test/support";
import { offeredProcessors, PerformancePage } from "./PerformancePage.tsx";

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
            <PerformancePage />
        </DraftHarness>,
    );

    drafted = () => latest;

    return result;
};

/** A machine with an NVIDIA card, which the Mac the shared fixture describes is not. */
const RTX: SupportedProviders = { cpu: true, coreml: false, cuda: true, tensorrt: true };

/** A machine offering nothing beyond its processor. */
const CPU_ONLY: SupportedProviders = { cpu: true, coreml: false, cuda: false, tensorrt: false };

/** Mounts the page against a report, and returns the cards it offers, by name. */
const listed = (providers: SupportedProviders) => {
    useSetupStore.getState().succeeded(providers);
    const { unmount } = renderPage();

    const options = screen.getAllByRole("radio").map((radio) => radio.getAttribute("value"));

    unmount();

    return options;
};

const card = (name: string) => screen.getByRole("radio", { name }).closest("[data-slot='processor-card']");

beforeEach(() => {
    localStorage.clear();
    useSettingsStore.setState(useSettingsStore.getInitialState(), true);
    resetSetupStore();
});

describe("the processor cards", () => {
    it("offer the automatic choice, the reported providers and the CPU", () => {
        expect(listed(RTX)).toEqual(["auto", "tensorrt", "cuda", "cpu"]);
    });

    it("offer two choices on a machine that offers nothing but its processor", () => {
        // The list is built from the report, so a machine with no supported adapter is simply
        // offered less - rather than being offered a provider it would then fail to build.
        expect(listed(CPU_ONLY)).toEqual(["auto", "cpu"]);
    });

    it("offer CoreML where the machine reported it", () => {
        expect(listed(PROVIDERS)).toEqual(["auto", "coreml", "cpu"]);
    });

    it("do not offer a provider the frontend has no name for", () => {
        // The Rust struct is `#[non_exhaustive]`, so a provider added to the library arrives as a
        // fifth key this application says nothing about. One it cannot name is one it cannot draw.
        const future = { ...CPU_ONLY, vulkan: true } as unknown as SupportedProviders;

        expect(listed(future)).toEqual(["auto", "cpu"]);
        expect(offeredProcessors(future)).toEqual(["auto", "cpu"]);
    });

    it("are each named by the processor alone", () => {
        useSetupStore.getState().succeeded(RTX);
        renderPage();

        for (const name of ["Auto", "TensorRT", "CUDA", "CPU"]) {
            expect(screen.getByRole("radio", { name })).toBeInTheDocument();
        }
    });

    it("explain every choice, whichever one is chosen", () => {
        useSetupStore.getState().succeeded(RTX);
        useSettingsStore.setState({ processor: "cuda" });
        renderPage();

        // All four at once: the trade-off is read before choosing, not only after.
        for (const [name, processor] of [
            ["Auto", "auto"],
            ["TensorRT", "tensorrt"],
            ["CUDA", "cuda"],
            ["CPU", "cpu"],
        ] as const) {
            expect(screen.getByRole("radio", { name })).toHaveAccessibleDescription(
                i18n.t(`settings.performance.processor.${processor}.description`),
            );
        }
    });

    it("mark the automatic choice, and only it, as recommended", () => {
        useSetupStore.getState().succeeded(RTX);
        renderPage();

        expect(card("Auto")).toHaveTextContent("Recommended");
        expect(screen.getAllByText("Recommended")).toHaveLength(1);
    });

    it("choose a processor from a click anywhere on its card, and outline it", () => {
        useSetupStore.getState().succeeded(RTX);
        renderPage();

        fireEvent.click(screen.getByText(i18n.t("settings.performance.processor.cuda.description")));

        expect(drafted().processor).toBe("cuda");
        expect(screen.getByRole("radio", { name: "CUDA" })).toBeChecked();
        expect(card("CUDA")).toHaveClass("border-primary");
        expect(card("Auto")).toHaveClass("border-border");
    });

    it("read as the automatic choice where the machine no longer offers what was remembered", () => {
        // A driver uninstalled, or a settings file carried to another machine. The stored value is
        // still there; what the page shows is the only choice that is always honourable.
        useSettingsStore.setState({ processor: "tensorrt" });
        useSetupStore.getState().succeeded(PROVIDERS);

        renderPage();

        expect(screen.getByRole("radio", { name: "Auto" })).toBeChecked();
    });

    it("offer the automatic choice alone before a report has arrived", () => {
        // Nothing has decided the answer yet, which is the honest reading of an absent report - and
        // it cannot be seen in the application, because the dialog is unreachable until setup ends.
        renderPage();

        expect(screen.getAllByRole("radio")).toHaveLength(1);
        expect(screen.getByRole("radio", { name: "Auto" })).toBeChecked();
    });

    it.each([
        ["Yes", true, "Auto"],
        ["No", false, "CUDA"],
    ])("show the choice the TensorRT question's %s made", (_case, enable, name) => {
        // The question is not the settings surface, so nothing is drafted: the answer is in force,
        // and the page opened afterwards seeds its draft from it like any saved preference.
        useSetupStore.getState().succeeded(RTX);
        useSettingsStore.setState({ processor: "cpu" });
        useSettingsStore.getState().answerTensorRT(enable);

        renderPage();

        expect(screen.getByRole("radio", { name })).toBeChecked();
    });
});
