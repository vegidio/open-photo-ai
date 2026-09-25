import { invoke } from "@tauri-apps/api/core";
import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import "@/i18n";
import i18n from "@/i18n";
import { forgetCatalogue } from "@/ipc/catalogue";
import { type SettingsData, useSettingsStore } from "@/stores/settings";
import { CATALOGUE, render } from "@/test/support";
import { DraftHarness } from "./draft.tsx";
import { EnhancementsPage } from "./EnhancementsPage.tsx";

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
            <EnhancementsPage />
        </DraftHarness>,
    );

    drafted = () => latest;

    return result;
};

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invoked = invoke as unknown as Mock;

/** Mounts the table with the catalogue already answered. */
const mount = async () => {
    invoked.mockResolvedValue(CATALOGUE);

    await act(async () => {
        renderPage();
    });
};

beforeEach(() => {
    localStorage.clear();
    useSettingsStore.setState(useSettingsStore.getInitialState(), true);
    forgetCatalogue();
});

describe("the enhancements table", () => {
    it("draws one row per enhancement, titled by its own name key", async () => {
        await mount();

        for (const name of [
            "Denoise",
            "Face Recovery",
            "Colorization",
            "Light Adjustment",
            "Color Balance",
            "Sharpen",
            "Upscale",
        ]) {
            expect(screen.getByRole("combobox", { name })).toBeInTheDocument();
        }
    });

    it("gives detection no row", async () => {
        await mount();

        // Published by the library and drawn by nothing: it is not an enhancement a user adds.
        expect(screen.queryByRole("combobox", { name: "Detection" })).not.toBeInTheDocument();
        expect(screen.getAllByRole("combobox")).toHaveLength(7);
    });

    it("fetches the catalogue once for all seven", async () => {
        await mount();

        expect(invoked).toHaveBeenCalledTimes(1);
        expect(invoked).toHaveBeenCalledWith("catalogue");
    });

    it("offers the catalogue's models, at the tiers its precisions name", async () => {
        await mount();

        fireEvent.keyDown(screen.getByRole("combobox", { name: "Denoise" }), { key: "Enter" });

        expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual([
            "Stockholm HD",
            "Stockholm SD",
            "Gothenburg HD",
            "Gothenburg SD",
            "Malmö HD",
            "Malmö SD",
        ]);
    });

    it("rules off each model from the next, and not its own tiers", async () => {
        await mount();

        fireEvent.keyDown(screen.getByRole("combobox", { name: "Denoise" }), { key: "Enter" });

        // Three models, so two rules - between them, never inside one and never at either end. The
        // list reads as pairs, and without the break the eye has to re-read each label to find where
        // one model ends.
        const listbox = screen.getByRole("listbox");
        expect(listbox.querySelectorAll("[data-slot='select-separator']")).toHaveLength(2);

        const kinds = [...listbox.querySelectorAll("[role='option'], [data-slot='select-separator']")].map((node) =>
            node.getAttribute("role") === "option" ? node.textContent : "---",
        );
        expect(kinds).toEqual([
            "Stockholm HD",
            "Stockholm SD",
            "---",
            "Gothenburg HD",
            "Gothenburg SD",
            "---",
            "Malmö HD",
            "Malmö SD",
        ]);
    });

    it("rules off a model that publishes a different number of precisions", async () => {
        await mount();

        fireEvent.keyDown(screen.getByRole("combobox", { name: "Upscale" }), { key: "Enter" });

        // Four models - Osaka's pair is fp16/int8 rather than the float convention - so three rules,
        // which is what driving the break off the codename rather than a count buys.
        expect(screen.getByRole("listbox").querySelectorAll("[data-slot='select-separator']")).toHaveLength(3);
    });

    it("reads a family never chosen for as the catalogue's first model", async () => {
        await mount();

        // Tokyo, not Kyoto, which is a deliberate parity gap with the reference: it is the order the
        // library publishes rather than a codename written into this frontend.
        expect(screen.getByRole("combobox", { name: "Upscale" })).toHaveTextContent("Tokyo HD");
    });

    it("writes only the family whose model was chosen", async () => {
        useSettingsStore.setState({ models: { sharpen: "novgorod_fp16" } });
        await mount();

        fireEvent.keyDown(screen.getByRole("combobox", { name: "Denoise" }), { key: "Enter" });
        fireEvent.keyDown(screen.getByRole("option", { name: "Gothenburg SD" }), { key: "Enter" });

        expect(drafted().models).toEqual({
            sharpen: "novgorod_fp16",
            denoise: "gothenburg_fp16",
        });
    });

    it("draws a stored model the library no longer publishes as the family's first", async () => {
        useSettingsStore.setState({ models: { denoise: "atlantis_fp32" } });
        await mount();

        expect(screen.getByRole("combobox", { name: "Denoise" })).toHaveTextContent("Stockholm HD");
    });

    it("translates the tier but never the model's name", async () => {
        await act(async () => {
            await i18n.changeLanguage("sv");
        });
        await mount();

        fireEvent.keyDown(screen.getByRole("combobox", { name: i18n.t("enhancements.denoise.name") }), {
            key: "Enter",
        });
        const options = screen.getAllByRole("option").map((option) => option.textContent);

        // Model names are proper nouns and stay out of the catalogues; the tier is the only part of
        // the label a catalogue supplies.
        expect(options[0]).toContain("Stockholm");
        expect(options[1]).toBe(i18n.t("models.label", { model: "Stockholm", quality: i18n.t("models.quality.md") }));

        await act(async () => {
            await i18n.changeLanguage("en");
        });
    });
});

describe("the Autopilot switches", () => {
    const NAMES = [
        "Denoise",
        "Face Recovery",
        "Colorization",
        "Light Adjustment",
        "Color Balance",
        "Sharpen",
        "Upscale",
    ];

    it("are addressable by their family's name", async () => {
        await mount();

        for (const name of NAMES) {
            expect(screen.getByRole("switch", { name: `Autopilot: ${name}` })).toBeInTheDocument();
        }
        expect(screen.getAllByRole("switch")).toHaveLength(7);
    });

    it("are all on for a user who has never chosen", async () => {
        await mount();

        for (const name of NAMES) {
            expect(screen.getByRole("switch", { name: `Autopilot: ${name}` })).toBeChecked();
        }
    });

    it("toggle only their own family", async () => {
        useSettingsStore.setState({ autopilotExcluded: ["sharpen"] });
        await mount();

        fireEvent.click(screen.getByRole("switch", { name: "Autopilot: Colorization" }));

        // Kept in `ENHANCEMENTS`' order rather than the order clicked in.
        expect(drafted().autopilotExcluded).toEqual(["colorization", "sharpen"]);
        expect(drafted().models).toEqual({});

        fireEvent.click(screen.getByRole("switch", { name: "Autopilot: Sharpen" }));

        expect(drafted().autopilotExcluded).toEqual(["colorization"]);
        expect(screen.getByRole("switch", { name: "Autopilot: Sharpen" })).toBeChecked();
        expect(screen.getByRole("switch", { name: "Autopilot: Colorization" })).not.toBeChecked();
    });

    it("leave the model choices alone, and are left alone by them", async () => {
        await mount();

        fireEvent.keyDown(screen.getByRole("combobox", { name: "Denoise" }), { key: "Enter" });
        fireEvent.keyDown(screen.getByRole("option", { name: "Gothenburg SD" }), { key: "Enter" });

        expect(drafted().autopilotExcluded).toEqual([]);
    });
});

describe("the table's header", () => {
    it("names its three columns", async () => {
        await mount();

        for (const heading of ["Enhancement", "Default model", "Autopilot"]) {
            expect(screen.getByText(heading)).toBeInTheDocument();
        }
    });
});
