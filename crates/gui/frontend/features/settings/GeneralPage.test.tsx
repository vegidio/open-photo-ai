import { invoke } from "@tauri-apps/api/core";
import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import "@/i18n";
import i18n from "@/i18n";
import { LANGUAGE_NAMES, LANGUAGE_TAGS } from "@/i18n/languages";
import { type SettingsData, useSettingsStore } from "@/stores/settings";
import { render } from "@/test/support";
import { DraftHarness } from "./draft.tsx";
import { GeneralPage } from "./GeneralPage.tsx";

/**
 * The page, on a draft of its own.
 *
 * The page reads and writes the dialog's draft rather than the settings store, so it needs one to
 * render at all. `renderPage` mounts it in a `DraftHarness`, whose draft is copied from what the store
 * holds when it mounts, exactly as opening the dialog does - and `drafted()` is what the page has
 * written since.
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
            <GeneralPage />
        </DraftHarness>,
    );

    drafted = () => latest;

    return result;
};

// Mocked at the `invoke` boundary, so what the Logs row asks for is what would actually go on the
// wire - the same seam `ipc/logs.test.ts` pins from the other side.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invoked = invoke as unknown as Mock;

beforeEach(() => {
    localStorage.clear();
    useSettingsStore.setState(useSettingsStore.getInitialState(), true);
});

describe("the language row", () => {
    /**
     * The options as the picker lists them, in order.
     *
     * Mounted and unmounted per call rather than opened twice over one render: an open Radix select
     * marks everything outside its portal `aria-hidden`, so a trigger that has been opened once is
     * no longer addressable until the select is torn down. Unmounting is what actually runs that
     * teardown, and it leaves the DOM as the next call finds it.
     */
    const listed = () => {
        const { unmount } = renderPage();

        fireEvent.keyDown(screen.getByRole("combobox", { name: i18n.t("settings.app.language.title") }), {
            key: "Enter",
        });
        const options = screen.getAllByRole("option").map((option) => option.textContent);

        unmount();

        return options;
    };

    it("names each language in itself, whatever language the application is showing", async () => {
        const inEnglish = listed();

        await act(async () => {
            await i18n.changeLanguage("ja");
        });

        // The same thirteen strings, unchanged: someone who cannot read the interface must still be
        // able to find their own language, which a translated list would prevent.
        expect(listed()).toEqual(inEnglish);
        expect(inEnglish).toContain("\u65e5\u672c\u8a9e");

        await act(async () => {
            await i18n.changeLanguage("en");
        });
    });

    it("offers every language the application ships, and no others", () => {
        expect([...listed()].sort()).toEqual(LANGUAGE_TAGS.map((tag) => LANGUAGE_NAMES[tag]).sort());
    });

    it("lists them in the order of their own names", () => {
        // Bahasa Indonesia first, not `de`: the tag is never shown, so ordering by it would order
        // the list by something the user cannot see.
        expect(listed()[0]).toBe("Bahasa Indonesia");
    });

    it("writes the choice to the draft without applying it", () => {
        renderPage();

        fireEvent.keyDown(screen.getByRole("combobox", { name: "Language" }), { key: "Enter" });
        fireEvent.keyDown(screen.getByRole("option", { name: "Svenska" }), { key: "Enter" });

        expect(drafted().language).toBe("sv");
        // Not applied: that happens on Save, which is what lets Cancel take it back.
        expect(i18n.resolvedLanguage).toBe("en");
    });
});

describe("the logs row", () => {
    it("reveals the file", async () => {
        invoked.mockResolvedValue(undefined);
        renderPage();

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Show logs" }));
        });

        // One command: `reveal_log` resolves the path in Rust, so the row has nothing to ask first.
        expect(invoked).toHaveBeenCalledWith("reveal_log");
        expect(invoked).toHaveBeenCalledTimes(1);
    });

    it("tells the user when it could not be shown", async () => {
        vi.spyOn(console, "error").mockImplementation(() => {});
        invoked.mockRejectedValue({ kind: "revealLog", message: "no file manager" });
        renderPage();

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Show logs" }));
        });

        // The catalogue's own sentence, in the language the application is running in - a diagnostic
        // nobody is shown is worse than the failure it describes.
        expect(await screen.findByText(i18n.t("errors.showLogsFailed"))).toBeInTheDocument();
    });

    it("says so in the language the application is running in", async () => {
        vi.spyOn(console, "error").mockImplementation(() => {});
        invoked.mockRejectedValue("nope");
        await act(async () => {
            await i18n.changeLanguage("sv");
        });
        renderPage();

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: i18n.t("settings.app.logs.button") }));
        });

        expect(await screen.findByText(i18n.t("errors.showLogsFailed"))).toBeInTheDocument();

        await act(async () => {
            await i18n.changeLanguage("en");
        });
    });
});

describe("the analytics row", () => {
    it("remembers the choice", () => {
        renderPage();

        fireEvent.click(screen.getByRole("switch", { name: "Analytics" }));

        expect(drafted().analytics).toBe(false);
    });

    it("sends nothing when it is turned off", () => {
        renderPage();

        fireEvent.click(screen.getByRole("switch", { name: "Analytics" }));

        // An opt-out that is transmitted is not an opt-out. Nothing in this application reports an
        // event yet, and this row is the reason that stays true rather than happening to be true.
        expect(invoked).not.toHaveBeenCalled();
    });
});

describe("the background tiles", () => {
    const tile = (background: string) =>
        document.querySelector<HTMLElement>(`[data-slot='background-tile'][for='settings_background_${background}']`);

    it("draws both surfaces as live samples", () => {
        renderPage();

        // The real particle field in one, the canvas's own dot in the other - each beside its name.
        expect(tile("particles")?.querySelector("[data-slot='particles']")).toBeInTheDocument();
        expect(tile("dotted")?.firstElementChild?.className).toContain("var(--color-surface-dot)");
        expect(screen.getByRole("radio", { name: "Particles" })).toBeChecked();
        expect(screen.getByRole("radio", { name: "Dotted" })).not.toBeChecked();
    });

    it("chooses a surface from a click anywhere on its tile", () => {
        renderPage();

        fireEvent.click(tile("dotted") as HTMLElement);

        expect(drafted().background).toBe("dotted");
        expect(screen.getByRole("radio", { name: "Dotted" })).toBeChecked();
        // Outlined, so which tile is chosen reads without finding its radio.
        expect(tile("dotted")).toHaveClass("outline-primary");
        expect(tile("particles")).toHaveClass("outline-border");
    });

    it("is one group, named by the card's own title", () => {
        renderPage();

        expect(screen.getByRole("radiogroup", { name: "Preview background" })).toBeInTheDocument();
    });

    it("does not apply the chosen surface before Save", () => {
        renderPage();

        fireEvent.click(tile("dotted") as HTMLElement);

        // The canvas reads the store; only Save writes it. End to end in `background.test.tsx`.
        expect(useSettingsStore.getState().background).toBe("particles");
    });
});
