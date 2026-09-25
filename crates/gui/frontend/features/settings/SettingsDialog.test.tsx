import { invoke } from "@tauri-apps/api/core";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { Navbar } from "@/features/navbar/Navbar";
import "@/i18n";
import i18n from "@/i18n";
import { forgetCatalogue } from "@/ipc/catalogue";
import { track } from "@/lib/faro";
import { type SettingsData, settingsData, useSettingsStore } from "@/stores/settings";
import { useSetupStore } from "@/stores/setup";
import { CATALOGUE, PROVIDERS, PUBLISHED_QUALITY, render, resetSetupStore } from "@/test/support";
import { useDraftState } from "./draft.tsx";
import { SettingsDialog } from "./SettingsDialog.tsx";
import { changedSettings, useSettingsDialog } from "./useSettingsDialog.ts";

// Mocked at `lib/faro.ts`'s own boundary: what `track` does with an event is pinned by `faro.test.ts`.
// Rust's format table, answered at once: the hook's own fetch is pinned by `ipc/export.test.ts`.
vi.mock("@/hooks/useExportFormats", async () => {
    const { EXPORT_FORMATS } = await import("@/test/support");

    return { useExportFormats: () => EXPORT_FORMATS };
});
vi.mock("@/lib/faro", () => ({
    track: vi.fn(),
    sendError: vi.fn(),
    pauseFaro: vi.fn(),
    // Untraced, as before Faro starts: the request goes out exactly as `invoke` alone would send it.
    traced: (_name: string, send: () => Promise<unknown>) => send(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@/ipc/os", () => ({ isMacOs: () => false, isWindows: () => false }));

const invoked = invoke as unknown as Mock;

/**
 * The dialog, open, on a draft of its own, for the tests about what the surface *draws*: the rows need
 * a draft to render at all - see `useSettingsDraft`.
 */
const Standalone = ({ close }: { close: (save: boolean) => void }) => {
    const { draft } = useDraftState();

    return <SettingsDialog open close={close} draft={draft} />;
};

const open = async (close: (save: boolean) => void = vi.fn()) => {
    await act(async () => {
        render(<Standalone close={close} />);
    });
};

/**
 * The write a row makes, reachable from a test.
 *
 * The rows write the dialog's draft, so this is the level the tests below change a preference at.
 * What the real controls do to the real draft is `background.test.tsx`'s subject, end to end through
 * the DOM.
 */
let editDraft: (patch: Partial<SettingsData>) => void;

/** The chrome that owns the dialog, exactly as the application mounts it. */
const Surface = () => {
    const { open: isOpen, draft, openSettings, close } = useSettingsDialog();

    editDraft = draft.update;

    return (
        <>
            <button type="button" onClick={openSettings}>
                Settings
            </button>
            <SettingsDialog open={isOpen} close={close} draft={draft} />
        </>
    );
};

/** The navbar, as the application mounts it, with its Settings button pressed. */
const openFromNavbar = async () => {
    await act(async () => {
        render(<Navbar />);
    });

    await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    });
};

/** The surface above, opened - which is the navbar's arrangement with the draft reachable. */
const openSurface = async () => {
    await act(async () => {
        render(<Surface />);
    });

    await act(async () => {
        fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    });
};

beforeEach(() => {
    localStorage.clear();
    useSettingsStore.setState(useSettingsStore.getInitialState(), true);
    resetSetupStore();
    forgetCatalogue();

    // The catalogue for the enhancement rows; everything else this dialog invokes resolves the same
    // way, which is enough for a frame that asserts on structure rather than on any one command.
    invoked.mockResolvedValue(CATALOGUE);
    useSetupStore.getState().succeeded(PROVIDERS);
});

afterEach(async () => {
    await act(async () => {
        await i18n.changeLanguage("en");
    });
});

describe("the settings surface", () => {
    it("opens from the window's own chrome", async () => {
        await openFromNavbar();

        expect(screen.getByRole("dialog")).toHaveAccessibleName("Settings");
    });

    it("opens on General, with its name, its description and the nav marking it", async () => {
        await open();

        const nav = screen.getByRole("navigation", { name: "Settings" });

        expect(screen.getByRole("heading", { name: "General" })).toBeInTheDocument();
        expect(screen.getByText(i18n.t("settings.pages.general.description"))).toBeInTheDocument();
        expect(within(nav).getByRole("button", { name: "General" })).toHaveAttribute("aria-current", "page");
        // Exactly one, so the mark moves rather than accumulating.
        expect(nav.querySelectorAll("[aria-current]")).toHaveLength(1);
    });

    it("lists the four pages", async () => {
        await open();

        const nav = screen.getByRole("navigation", { name: "Settings" });

        expect(
            within(nav)
                .getAllByRole("button")
                .map((button) => button.textContent),
        ).toEqual(["General", "Performance", "Enhancements", "Export"]);
    });

    it("shows a chosen page in place of the one before it, and marks it current", async () => {
        await open();

        const nav = screen.getByRole("navigation", { name: "Settings" });

        await act(async () => {
            fireEvent.click(within(nav).getByRole("button", { name: "Export" }));
        });

        expect(screen.getByRole("heading", { name: "Export" })).toBeInTheDocument();
        expect(screen.getByRole("slider", { name: "JPEG" })).toBeInTheDocument();
        // General's controls are gone rather than scrolled away.
        expect(screen.queryByRole("combobox", { name: "Language" })).not.toBeInTheDocument();
        expect(within(nav).getByRole("button", { name: "Export" })).toHaveAttribute("aria-current", "page");
        expect(within(nav).getByRole("button", { name: "General" })).not.toHaveAttribute("aria-current");
    });

    it("keeps a change made on one page after moving to another and back", async () => {
        await openSurface();

        fireEvent.click(screen.getByRole("switch", { name: "Analytics" }));

        const nav = () => screen.getByRole("navigation", { name: "Settings" });
        await act(async () => {
            fireEvent.click(within(nav()).getByRole("button", { name: "Export" }));
        });
        await act(async () => {
            fireEvent.click(within(nav()).getByRole("button", { name: "General" }));
        });

        expect(screen.getByRole("switch", { name: "Analytics" })).not.toBeChecked();
        // Nothing applied by the trip.
        expect(useSettingsStore.getState().analytics).toBe(true);
    });

    it("opens on General again, whichever page it was left on", async () => {
        await openSurface();

        await act(async () => {
            fireEvent.click(
                within(screen.getByRole("navigation", { name: "Settings" })).getByRole("button", { name: "Export" }),
            );
        });
        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
        });
        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Settings" }));
        });

        expect(screen.getByRole("heading", { name: "General" })).toBeInTheDocument();
    });

    it("is not dismissed by a click outside it", async () => {
        const close = vi.fn();
        await openSurface();

        await act(async () => {
            editDraft({ analytics: false });
        });

        fireEvent.pointerDown(document.body);

        // Neither closed nor discarded: everything on the surface is a draft, and a stray click is
        // not an answer to "keep this or not".
        expect(close).not.toHaveBeenCalled();
        expect(screen.getByRole("dialog")).toBeInTheDocument();
        expect(screen.getByRole("switch", { name: /analytics/i })).not.toBeChecked();
    });

    it("changes nothing by being opened and closed", async () => {
        const before = { ...useSettingsStore.getState() };
        await openFromNavbar();

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
        });

        expect(useSettingsStore.getState()).toMatchObject({
            language: before.language,
            analytics: before.analytics,
            processor: before.processor,
            models: before.models,
            autopilotExcluded: before.autopilotExcluded,
            quality: before.quality,
        });
    });
});

describe("drafts, Cancel and Save", () => {
    /** Changes four rows of four different shapes, through the draft the rows write to. */
    const changeFourRows = async () => {
        await act(async () => {
            editDraft({
                language: "sv",
                processor: "coreml",
                models: { upscale: "kyoto_fp16" },
                quality: { ...useSettingsStore.getState().quality, jpeg: 42 },
            });
        });
    };

    it("reverts every changed row on Cancel, with the language never applied", async () => {
        await openSurface();
        await changeFourRows();

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
        });

        expect(useSettingsStore.getState()).toMatchObject({
            language: "en",
            processor: "auto",
            models: {},
        });
        // No format was ever moved, so none has a quality of its own: each is at the default Rust publishes.
        expect(useSettingsStore.getState().quality).toEqual({});
        // Unchanged throughout: the draft was never applied, so there was nothing to take back.
        expect(i18n.resolvedLanguage).toBe("en");
    });

    it("sends which preferences a Save changed, sorted, and nothing for a Cancel", async () => {
        await openSurface();
        await changeFourRows();
        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
        });
        expect(track).not.toHaveBeenCalled();

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Settings" }));
        });
        await changeFourRows();
        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Save" }));
        });

        expect(track).toHaveBeenCalledExactlyOnceWith("settings_saved", {
            changed: ["language", "models", "processor", "quality"],
        });
    });

    it("sends nothing for a Save that changes nothing, or changes only the Analytics choice", async () => {
        await openSurface();
        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Save" }));
        });

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Settings" }));
        });
        await act(async () => editDraft({ analytics: false }));
        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Save" }));
        });

        expect(useSettingsStore.getState().analytics).toBe(false);
        expect(track).not.toHaveBeenCalled();
    });

    it("applies the chosen language on Save", async () => {
        await openSurface();
        await changeFourRows();

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Save" }));
        });

        expect(i18n.resolvedLanguage).toBe("sv");
        expect(useSettingsStore.getState()).toMatchObject({ language: "sv", processor: "coreml" });
    });

    /** The language as the next launch would read it: off the disk, not out of the store. */
    const savedLanguage = () => JSON.parse(localStorage.getItem("settings-storage") ?? "{}").state?.language;

    it("writes nothing to storage until Save", async () => {
        await openSurface();
        await changeFourRows();

        // Still what it was before the surface opened. A draft that reached the disk would be one
        // Cancel could not take back, because quitting with the dialog open is not a path Cancel is on.
        expect(savedLanguage()).toBe("en");

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Save" }));
        });

        expect(savedLanguage()).toBe("sv");
    });

    it("leaves storage as it was when the surface is cancelled", async () => {
        await openSurface();
        await changeFourRows();

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
        });

        expect(savedLanguage()).toBe("en");
    });

    it("reverts on dismissal as well as on Cancel", async () => {
        await openSurface();
        await changeFourRows();

        // Escape, which is the other way out the surface accepts.
        await act(async () => {
            fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
        });

        expect(useSettingsStore.getState().language).toBe("en");
        expect(i18n.resolvedLanguage).toBe("en");
    });
});

describe("Reset to defaults", () => {
    const goTo = async (page: string) => {
        await act(async () => {
            fireEvent.click(
                within(screen.getByRole("navigation", { name: "Settings" })).getByRole("button", { name: page }),
            );
        });
    };

    const reset = async () => {
        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Reset to defaults" }));
        });
    };

    it("puts back only the page being viewed", async () => {
        useSettingsStore.setState({ quality: { avif: 20, heic: 30, jpeg: 40, webp: 50 } });
        await openSurface();

        await act(async () => {
            editDraft({ language: "sv" });
        });
        await goTo(i18n.t("settings.pages.export.title"));
        await reset();

        for (const [format, value] of Object.entries({ AVIF: 60, HEIC: 60, JPEG: 90, WEBP: 75 })) {
            expect(screen.getByRole("slider", { name: format })).toHaveAttribute("aria-valuenow", String(value));
        }

        // The language is on another page, so the reset leaves the change to it alone.
        await goTo(i18n.t("settings.pages.general.title"));
        expect(screen.getByRole("combobox", { name: i18n.t("settings.app.language.title") })).toHaveTextContent(
            "Svenska",
        );
    });

    it("is a draft: Cancel after a reset leaves everything as it was", async () => {
        useSettingsStore.setState({
            quality: { avif: 20, heic: 30, jpeg: 40, webp: 50 },
            models: { upscale: "kyoto_fp16" },
            autopilotExcluded: ["colorization"],
        });
        const before = { ...useSettingsStore.getState() };
        await openSurface();

        await goTo("Export");
        await reset();
        await goTo("Enhancements");
        await reset();

        // Nothing applied by the resets themselves.
        expect(useSettingsStore.getState().quality).toEqual(before.quality);

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
        });

        expect(useSettingsStore.getState()).toMatchObject({
            quality: before.quality,
            models: before.models,
            autopilotExcluded: before.autopilotExcluded,
        });
    });

    it("clears the model choices and the exclusions when a reset Enhancements page is saved", async () => {
        useSettingsStore.setState({
            models: { upscale: "kyoto_fp16", denoise: "gothenburg_fp16" },
            autopilotExcluded: ["colorization", "sharpen"],
            quality: { ...PUBLISHED_QUALITY, jpeg: 42 },
        });
        await openSurface();

        await goTo("Enhancements");
        await reset();

        expect(screen.getByRole("switch", { name: "Autopilot: Colorization" })).toBeChecked();

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Save" }));
        });

        expect(useSettingsStore.getState()).toMatchObject({ models: {}, autopilotExcluded: [] });
        // Export is another page, so its change survives.
        expect(useSettingsStore.getState().quality.jpeg).toBe(42);
    });
});

describe("changedSettings", () => {
    const inForce = () => settingsData(useSettingsStore.getState());

    it("reads a record and a set by what they hold, not by the order they hold it in", () => {
        const a = { ...inForce(), models: { upscale: "kyoto_fp16", denoise: "stockholm_fp32" } } as SettingsData;
        const b = { ...inForce(), models: { denoise: "stockholm_fp32", upscale: "kyoto_fp16" } } as SettingsData;

        expect(changedSettings(a, b)).toEqual([]);
        expect(
            changedSettings(
                { ...a, autopilotExcluded: ["denoise", "upscale"] },
                { ...a, autopilotExcluded: ["upscale", "denoise"] },
            ),
        ).toEqual([]);
    });

    it("never names the Analytics choice", () => {
        expect(changedSettings(inForce(), { ...inForce(), analytics: !inForce().analytics })).toEqual([]);
    });
});
