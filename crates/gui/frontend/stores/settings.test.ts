import { beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_LANGUAGE } from "@/i18n/languages";
import {
    BACKGROUNDS,
    DEFAULT_QUALITY,
    MAX_QUALITY,
    MIN_QUALITY,
    settingsData,
    settingsDefaults,
    useSettingsStore,
} from "./settings.ts";

/** Seeds `localStorage` with a stored settings state and rehydrates the store from it. */
const stored = async (state: Record<string, unknown>) => {
    // `persist` reads storage once, at import, so every test after the first would otherwise be
    // asserting against whatever the first one left behind. `rehydrate()` is the middleware's own way
    // back in, and going through it is what makes these tests about the repairs the application
    // actually performs rather than about a hand-rolled copy of them.
    localStorage.setItem("settings-storage", JSON.stringify({ state, version: 0 }));

    await useSettingsStore.persist.rehydrate();
};

/** A complete, sound stored state, for the tests that break exactly one field of it. */
const SOUND = {
    language: "sv",
    background: "dotted",
    analytics: false,
    processor: "coreml",
    models: { denoise: "gothenburg_fp16" },
    autopilotExcluded: ["colorization"],
    quality: { avif: 42, heic: 43, jpeg: 44, webp: 45 },
} as const;

beforeEach(() => {
    localStorage.clear();
    useSettingsStore.setState(useSettingsStore.getInitialState(), true);
});

describe("apply", () => {
    it("puts every field it is given in force at once", () => {
        useSettingsStore.getState().apply({
            language: "ja",
            background: "dotted",
            analytics: false,
            processor: "coreml",
            models: { upscale: "kyoto_fp16" },
            quality: { ...DEFAULT_QUALITY, jpeg: 42 },
        });

        expect(useSettingsStore.getState()).toMatchObject({
            language: "ja",
            background: "dotted",
            analytics: false,
            processor: "coreml",
            models: { upscale: "kyoto_fp16" },
            quality: { ...DEFAULT_QUALITY, jpeg: 42 },
        });
    });

    it("leaves a field it is not given alone", () => {
        const before = useSettingsStore.getState().processor;

        useSettingsStore.getState().apply({ language: "ja" });

        expect(useSettingsStore.getState()).toMatchObject({ language: "ja", processor: before });
    });

    it("bounds a quality a caller hands it", () => {
        useSettingsStore.getState().apply({ quality: { ...DEFAULT_QUALITY, jpeg: 0, webp: 4000 } });

        expect(useSettingsStore.getState().quality).toMatchObject({ jpeg: MIN_QUALITY, webp: MAX_QUALITY });
    });

    it("writes what it applied to storage", () => {
        // What keeps a *draft* off the disk is that a draft never reaches this store at all - see
        // `features/settings/draft.tsx`.
        useSettingsStore.getState().apply({ language: "sv" });

        expect(JSON.parse(localStorage.getItem("settings-storage") ?? "{}").state?.language).toBe("sv");
    });
});

describe("what a restart reads back", () => {
    it("returns every saved preference", async () => {
        await stored(SOUND);

        expect(useSettingsStore.getState()).toMatchObject(SOUND);
    });

    it("replaces a language it no longer ships, and keeps the rest", async () => {
        await stored({ ...SOUND, language: "ko" });

        expect(useSettingsStore.getState().language).toBe(DEFAULT_LANGUAGE);
        expect(useSettingsStore.getState().processor).toBe("coreml");
    });

    it("replaces a processor it has no name for, and keeps the rest", async () => {
        // The Wails application's own spelling, which is the case this guard was written for.
        await stored({ ...SOUND, processor: "TensorRT" });

        expect(useSettingsStore.getState().processor).toBe("auto");
        expect(useSettingsStore.getState().language).toBe("sv");
    });

    it("starts a settings file written by the Wails application on the particle field", async () => {
        // That application drew one surface and offered no choice, so its stored object simply has
        // no background in it. This is the ordinary upgrade path rather than a corrupt file, and the
        // default is the answer to it.
        const { background: _absent, ...wails } = SOUND;
        await stored(wails);

        expect(useSettingsStore.getState().background).toBe("particles");
        // The rest of what that application saved is still theirs.
        expect(useSettingsStore.getState().language).toBe("sv");
    });

    it("replaces a remembered background naming neither surface, and keeps the rest", async () => {
        await stored({ ...SOUND, background: "aurora" });

        expect(useSettingsStore.getState().background).toBe("particles");
        expect(useSettingsStore.getState().language).toBe("sv");
        expect(useSettingsStore.getState().processor).toBe("coreml");
    });

    it.each(BACKGROUNDS)("reads back a remembered %s surface as it was written", async (background) => {
        await stored({ ...SOUND, background });

        expect(useSettingsStore.getState().background).toBe(background);
    });

    it.each([
        ["absent", {}],
        ["zero", { jpeg: 0 }],
        ["above the bounds", { jpeg: MAX_QUALITY + 1 }],
        ["below the bounds", { jpeg: MIN_QUALITY - 1 }],
        ["not a number", { jpeg: "high" }],
    ])("replaces a %s quality with that format's starting value", async (_case, quality) => {
        await stored({ ...SOUND, quality: { ...quality, avif: 42 } });

        expect(useSettingsStore.getState().quality.jpeg).toBe(DEFAULT_QUALITY.jpeg);
        // The sound value beside it is untouched - the repair is per format, not per record.
        expect(useSettingsStore.getState().quality.avif).toBe(42);
    });

    it("keeps a stored model as written, for the row to resolve", async () => {
        // Including one that names nothing the library publishes: this store does not know the
        // catalogue and must not pretend to - see `lib/enhancements.ts`.
        await stored({ ...SOUND, models: { upscale: "atlantis_fp64" } });

        expect(useSettingsStore.getState().models).toEqual({ upscale: "atlantis_fp64" });
    });

    it("leaves a first launch's detected language alone", async () => {
        // `persist` runs `merge` whether or not storage held anything, so this is the boot where a
        // repair applied unconditionally would overwrite what the environment asked for - and then
        // persist the overwrite, making it permanent for every user who is not English.
        useSettingsStore.setState({ language: "de" });

        await useSettingsStore.persist.rehydrate();

        expect(useSettingsStore.getState().language).toBe("de");
    });

    it("survives a stored shape that is not one at all", async () => {
        await stored({
            language: 7,
            background: [],
            analytics: "yes",
            processor: null,
            models: "none",
            autopilotExcluded: "all",
            quality: 12,
        });

        expect(useSettingsStore.getState()).toMatchObject({
            language: DEFAULT_LANGUAGE,
            background: "particles",
            analytics: true,
            processor: "auto",
            models: {},
            autopilotExcluded: [],
            quality: DEFAULT_QUALITY,
        });
    });

    it("allows every enhancement for a settings file saved before the choice existed", async () => {
        const { autopilotExcluded: _absent, ...older } = SOUND;
        await stored(older);

        expect(useSettingsStore.getState().autopilotExcluded).toEqual([]);
        expect(useSettingsStore.getState().language).toBe("sv");
    });

    it.each([
        ["not a list", "colorization", []],
        ["naming a family it does not present", ["colorization", "detection", "aurora", 7], ["colorization"]],
        ["listing a family twice", ["sharpen", "colorization", "sharpen"], ["colorization", "sharpen"]],
    ])("repairs an exclusion list %s, and keeps the rest", async (_case, autopilotExcluded, repaired) => {
        await stored({ ...SOUND, autopilotExcluded });

        expect(useSettingsStore.getState().autopilotExcluded).toEqual(repaired);
        expect(useSettingsStore.getState()).toMatchObject({
            language: "sv",
            processor: "coreml",
            background: "dotted",
        });
    });
});

describe("the defaults", () => {
    it("are what a first launch holds", () => {
        // One definition read by both a first launch and Reset to defaults, so the two cannot disagree.
        expect(settingsData(useSettingsStore.getInitialState())).toEqual(settingsDefaults());
    });

    it("hand out records no two callers share", () => {
        const first = settingsDefaults();
        const second = settingsDefaults();

        // Two drafts seeded from one shared record would each be editing the other's.
        expect(first.quality).not.toBe(second.quality);
        expect(first.models).not.toBe(second.models);
        expect(first.autopilotExcluded).not.toBe(second.autopilotExcluded);
        expect(first).toEqual(second);
    });
});

describe("the TensorRT question", () => {
    /** What was last written to disk, as `persist` wrote it. */
    const persisted = () => JSON.parse(localStorage.getItem("settings-storage") ?? "{}").state;

    it("is unanswered on a first launch", async () => {
        await useSettingsStore.persist.rehydrate();

        expect(useSettingsStore.getState().tensorrtAsked).toBe(false);
    });

    it("stays answered across a restart", async () => {
        await stored({ ...SOUND, tensorrtAsked: true });

        expect(useSettingsStore.getState().tensorrtAsked).toBe(true);
    });

    it.each([
        ["answered", false, true],
        ["not yet answered", true, false],
    ])("reads a Wails settings file that %s it", async (_case, isFirstTensorRT, asked) => {
        // That application stored the same fact the other way up, under the same key.
        await stored({ ...SOUND, isFirstTensorRT });

        expect(useSettingsStore.getState().tensorrtAsked).toBe(asked);
    });

    it.each([
        ["a string", "true"],
        ["a number", 1],
    ])("reads %s as not yet answered", async (_case, tensorrtAsked) => {
        await stored({ ...SOUND, tensorrtAsked });

        expect(useSettingsStore.getState().tensorrtAsked).toBe(false);
    });

    it("is not a preference the settings surface holds", () => {
        useSettingsStore.getState().answerTensorRT(true);

        expect(settingsData(useSettingsStore.getState())).not.toHaveProperty("tensorrtAsked");
        expect(settingsDefaults()).not.toHaveProperty("tensorrtAsked");
    });

    it.each([
        ["Yes", true, "auto"],
        ["No", false, "cuda"],
    ])("answered %s, sets the processor and records the answer", (_case, enable, processor) => {
        useSettingsStore.setState({ processor: "coreml" });

        useSettingsStore.getState().answerTensorRT(enable);

        expect(useSettingsStore.getState()).toMatchObject({ processor, tensorrtAsked: true });
        // Both halves reached the disk, in the one write.
        expect(persisted()).toMatchObject({ processor, tensorrtAsked: true });
    });

    it("stays answered through a later Save", () => {
        useSettingsStore.getState().answerTensorRT(false);

        // What the settings dialog's Save hands back: the draft, which never carries the flag.
        useSettingsStore.getState().apply({ ...settingsData(useSettingsStore.getState()), language: "ja" });

        expect(useSettingsStore.getState().tensorrtAsked).toBe(true);
        expect(persisted().tensorrtAsked).toBe(true);
    });

    it("stays answered through a reset of the Performance page", () => {
        useSettingsStore.getState().answerTensorRT(false);

        useSettingsStore.getState().apply({ processor: settingsDefaults().processor });

        expect(useSettingsStore.getState()).toMatchObject({ processor: "auto", tensorrtAsked: true });
        expect(persisted()).toMatchObject({ processor: "auto", tensorrtAsked: true });
    });
});
