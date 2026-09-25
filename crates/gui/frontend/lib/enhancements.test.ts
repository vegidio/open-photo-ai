import { describe, expect, it } from "vitest";
import type { Family, FamilyEntry } from "@/ipc/catalogue";
import type { Operation } from "@/ipc/enhance";
import type { ImageRecord } from "@/ipc/images";
import { CATALOGUE, HOLIDAY } from "@/test/support";
import {
    defaultScale,
    ENHANCEMENTS,
    type Enhancement,
    modelChoice,
    newOperation,
    progressLabelKey,
    suggestedOperation,
    upscaleFactor,
} from "./enhancements.ts";

const entry = (family: string) => CATALOGUE.find((published) => published.family === family);

/** The values on offer, which is what a chooser's items are and what a stored selection is one of. */
const valuesOf = (choice: { options: { value: string }[] }) => choice.options.map((option) => option.value);

describe("the enhancements this application presents", () => {
    it("names seven", () => {
        expect(ENHANCEMENTS).toHaveLength(7);
    });

    it("names only families the catalogue publishes", () => {
        // The compile-time half is the `Family` type on each entry; this is the half that catches a
        // family renamed on the wire, which typechecks here and then matches nothing at runtime.
        const published = CATALOGUE.map(({ family }) => family);

        expect(ENHANCEMENTS.every(({ family }) => published.includes(family))).toBe(true);
    });

    it("leaves detection out", () => {
        // Published by the library on purpose, drawn by nothing: it is not an enhancement a user
        // adds, and this is how it stays out without the catalogue being filtered on the wire.
        // Read through the wider `Enhancement` type: the list's own literal types already rule
        // `detection` out, and this is the runtime half of the same statement.
        const presented: readonly Enhancement[] = ENHANCEMENTS;

        expect(presented.some(({ family }) => family === "detection")).toBe(false);
    });

    it("lists them in pipeline order", () => {
        expect(ENHANCEMENTS.map(({ family }) => family)).toEqual([
            "denoise",
            "face_recovery",
            "colorization",
            "light_adjustment",
            "color_balance",
            "sharpen",
            "upscale",
        ]);
    });
});

describe("what a family offers", () => {
    it("offers every published model at every precision it publishes", () => {
        expect(valuesOf(modelChoice(entry("denoise"), undefined))).toEqual([
            "stockholm_fp32",
            "stockholm_fp16",
            "gothenburg_fp32",
            "gothenburg_fp16",
            "malmo_fp32",
            "malmo_fp16",
        ]);
    });

    it("names the tiers positionally, off each variant's own precisions", () => {
        const { options } = modelChoice(entry("upscale"), undefined);

        // Tokyo's pair is the float convention; Osaka's is its own, and reading the position gives
        // each of them the right tier without a map of per-model overrides.
        expect(options.find((option) => option.value === "tokyo_fp32")?.quality).toEqual({ kind: "tier", tier: "hd" });
        expect(options.find((option) => option.value === "tokyo_fp16")?.quality).toEqual({ kind: "tier", tier: "md" });
        expect(options.find((option) => option.value === "osaka_fp16")?.quality).toEqual({ kind: "tier", tier: "hd" });
        expect(options.find((option) => option.value === "osaka_int8")?.quality).toEqual({ kind: "tier", tier: "md" });
    });

    it("gives a single-precision variant no tier at all", () => {
        // A tier label on a choice of one says nothing: there is no sibling to tell it apart from.
        const single: FamilyEntry = {
            family: "denoise",
            variants: [{ codename: "stockholm", label: "Stockholm", precisions: ["fp32"], parameters: [] }],
        };

        const { options } = modelChoice(single, undefined);

        expect(options).toEqual([
            { value: "stockholm_fp32", codename: "stockholm", precision: "fp32", first: true, label: "Stockholm" },
        ]);
    });

    it("shows a third precision's own spelling, which no tier has a word for", () => {
        const three: FamilyEntry = {
            family: "upscale",
            variants: [{ codename: "osaka", label: "Osaka", precisions: ["fp32", "fp16", "int8"], parameters: [] }],
        };

        expect(modelChoice(three, undefined).options.at(-1)?.quality).toEqual({ kind: "precision", precision: "int8" });
    });

    it("says which model each option is a precision of", () => {
        // What a chooser draws its group separators on: every model contributes one option per
        // precision, so this is the only thing that says where one model's tiers end.
        const { options } = modelChoice(entry("denoise"), undefined);

        expect(options.map((option) => option.codename)).toEqual([
            "stockholm",
            "stockholm",
            "gothenburg",
            "gothenburg",
            "malmo",
            "malmo",
        ]);
    });

    it("carries the catalogue's own label rather than composing one", () => {
        expect(modelChoice(entry("color_balance"), undefined).options.at(-1)?.label).toBe("São Paulo");
    });
});

describe("which model is selected", () => {
    it("is the stored one, where the library still publishes it", () => {
        expect(modelChoice(entry("denoise"), "malmo_fp16").selected).toBe("malmo_fp16");
    });

    it("is the first variant at its first precision, for a family never chosen for", () => {
        expect(modelChoice(entry("denoise"), undefined).selected).toBe("stockholm_fp32");
    });

    it("takes upscale's default from the catalogue's own order", () => {
        // This slice's one declared parity gap: the reference states Kyoto, and this takes the first
        // variant the catalogue publishes - which it documents as "the order a chooser should offer
        // them" - rather than writing seven codenames into a front end.
        expect(modelChoice(entry("upscale"), undefined).selected).toBe("tokyo_fp32");
    });

    it("replaces a model the library no longer publishes", () => {
        expect(modelChoice(entry("denoise"), "atlantis_fp32").selected).toBe("stockholm_fp32");
    });

    it("replaces a precision a published model no longer offers", () => {
        // There is no fp32 Osaka and never was; a store written against one must not leave the
        // chooser on a value none of its items carries.
        expect(modelChoice(entry("upscale"), "osaka_fp32").selected).toBe("tokyo_fp32");
    });

    it("offers nothing for a family the catalogue does not publish", () => {
        expect(modelChoice(undefined, "stockholm_fp32")).toEqual({ options: [], selected: "" });
    });

    it("is always one of the values on offer", () => {
        for (const { family } of ENHANCEMENTS) {
            const choice = modelChoice(entry(family), "nothing_real");

            expect(valuesOf(choice)).toContain(choice.selected);
        }
    });
});

describe("what a row, a menu entry and a chip are drawn from", () => {
    it("gives every enhancement a short name and a glyph", () => {
        // Both are consumed without a fallback - `<entry.icon />` and `t(entry.shortNameKey)` - so an
        // enhancement added to the list without one of them is a blank in the menu or on the chip.
        for (const { family, shortNameKey, icon } of ENHANCEMENTS) {
            expect(shortNameKey, family).toBeTruthy();
            expect(icon, family).toBeTruthy();
        }
    });

    it("gives each of them its own glyph", () => {
        // Seven rows drawn with one glyph would be a list nothing distinguishes at a glance, which is
        // what the icon is there for.
        const icons = ENHANCEMENTS.map(({ icon }) => icon);

        expect(new Set(icons).size).toBe(icons.length);
    });

    it("gives each of them its own short name", () => {
        // The chip names one enhancement at a time, so two families sharing a key would draw the
        // same word for two different operations.
        const keys = ENHANCEMENTS.map(({ shortNameKey }) => shortNameKey);

        expect(new Set(keys).size).toBe(keys.length);
    });
});

describe("what the progress chip calls the family a report carries", () => {
    /*
     * Every family a report can carry, as a total record: a family added to the union and forgotten
     * here is a type error rather than a case this walk silently skips.
     */
    const EVERY_FAMILY: Record<Family, true> = {
        detection: true,
        denoise: true,
        face_recovery: true,
        light_adjustment: true,
        color_balance: true,
        colorization: true,
        sharpen: true,
        upscale: true,
    };

    it("answers a name for every family a report can carry", () => {
        for (const family of Object.keys(EVERY_FAMILY) as Family[]) {
            expect(progressLabelKey(family), family).toBeTruthy();
        }
    });

    it("names a detection after the enhancement that asked for it", () => {
        // The detector is offered by no menu and named in no catalogue row here, so naming it would
        // name a model nobody chose. What the user asked for is the face recovery. See design.md D8.
        expect(progressLabelKey("detection")).toBe("preview.progress.faceRecovery");
        expect(progressLabelKey("face_recovery")).toBe(progressLabelKey("detection"));
    });

    it("names every other family as itself", () => {
        expect(progressLabelKey("upscale")).toBe("preview.progress.upscale");
        expect(progressLabelKey("denoise")).toBe("preview.progress.denoise");
    });

    it("answers nothing where there is no report yet", () => {
        // The chip draws its generic label for this, which is the honest thing to say about a run
        // that has started and not reported: something is happening and it has not said what.
        expect(progressLabelKey(undefined)).toBeUndefined();
    });
});

/** A photograph of a given size, with everything a scale is not chosen from left as the fixture has it. */
const sized = (width: number, height: number): ImageRecord => ({ ...HOLIDAY, width, height });

describe("the scale a new upscale arrives at", () => {
    it("enlarges a small photograph fourfold", () => {
        expect(defaultScale(sized(1024, 1024))).toBe(4);
    });

    it("enlarges a medium one twofold", () => {
        expect(defaultScale(sized(2048, 2048))).toBe(2);
    });

    it("leaves a large one alone", () => {
        expect(defaultScale(sized(6000, 4000))).toBe(1);
    });

    it("leaves a photograph it could not measure alone", () => {
        // Absent dimensions mean the header could not be parsed, so nothing is known about how large
        // the photograph is. The reference's own arithmetic lands this case in the *smallest* bucket
        // and enlarges it fourfold; the conservative end is the honest answer.
        const unmeasured: ImageRecord = { path: HOLIDAY.path, extension: "png" };

        expect(defaultScale(unmeasured)).toBe(1);
        expect(defaultScale(undefined)).toBe(1);
    });
});

describe("the operation a new enhancement carries", () => {
    const upscale = CATALOGUE.find((published) => published.family === "upscale");

    it("arrives at the user's stored default, at a scale suited to the photograph", () => {
        expect(newOperation("upscale", upscale, "kyoto_fp16", sized(1024, 1024))).toEqual({
            family: "upscale",
            codename: "kyoto",
            precision: "fp16",
            scale: 4,
        });
    });

    it("repairs a stored selection naming a model the catalogue no longer publishes", () => {
        // Through `modelChoice`, so the repair is the one the settings rows already ship rather than
        // a second implementation of it: the first variant the catalogue publishes, at its first
        // precision.
        expect(newOperation("upscale", upscale, "berlin_fp32", sized(6000, 4000))).toEqual({
            family: "upscale",
            codename: "tokyo",
            precision: "fp32",
            scale: 1,
        });
    });

    it("gives a new face recovery the default model and no faces", () => {
        // **No faces**, and not because none have been found: the stack is the user's choice, and the
        // faces are a property of the pixels that `useEnhancementRun` resolves on the way to a run.
        // A stack entry carrying them would be rewritten by every framing change. See design.md D5.
        expect(newOperation("face_recovery", entry("face_recovery"), undefined, sized(1024, 1024))).toEqual({
            family: "face_recovery",
            codename: "athens",
            precision: "fp32",
            faces: [],
        });
    });

    it("gives a face recovery the model the user stored for it", () => {
        expect(newOperation("face_recovery", entry("face_recovery"), "santorini_fp16", sized(1024, 1024))).toEqual({
            family: "face_recovery",
            codename: "santorini",
            precision: "fp16",
            faces: [],
        });
    });

    it("gives a new light adjustment the default model at a bias of 0.5", () => {
        // The reference's `defaultAmount`, and Paris because the catalogue lists it first.
        expect(newOperation("light_adjustment", entry("light_adjustment"), undefined, sized(1024, 1024))).toEqual({
            family: "light_adjustment",
            codename: "paris",
            precision: "fp32",
            bias: 0.5,
        });
    });

    it("gives a new colour balance the default model at a bias of 0.5", () => {
        // The reference's `defaultAmount` for this family too, and Rio because the catalogue lists it first.
        expect(newOperation("color_balance", entry("color_balance"), undefined, sized(1024, 1024))).toEqual({
            family: "color_balance",
            codename: "rio",
            precision: "fp32",
            bias: 0.5,
        });
    });

    it("gives a new denoise the default model at a strength of 1", () => {
        // The reference's `defaultAmount` - the model's own output - and Stockholm because the catalogue
        // lists it first.
        expect(newOperation("denoise", entry("denoise"), undefined, sized(1024, 1024))).toEqual({
            family: "denoise",
            codename: "stockholm",
            precision: "fp32",
            strength: 1,
        });
    });

    it("gives a new colorization the default model and nothing else", () => {
        // Colorization takes no parameter, so the model is all it carries - and Delhi because the
        // catalogue lists it first, which is also the reference's `defaultModel`.
        expect(newOperation("colorization", entry("colorization"), undefined, sized(1024, 1024))).toEqual({
            family: "colorization",
            codename: "delhi",
            precision: "fp32",
        });
    });

    it("gives a colorization the model the user stored for it", () => {
        expect(newOperation("colorization", entry("colorization"), "jaipur_fp16", sized(1024, 1024))).toEqual({
            family: "colorization",
            codename: "jaipur",
            precision: "fp16",
        });
    });

    it("gives a new sharpen the default model at a strength of 1", () => {
        // The reference's `defaultAmount` and `defaultModel` for sharpen - and Moscow because the
        // catalogue lists it first, not because anything here names it.
        expect(newOperation("sharpen", entry("sharpen"), undefined, sized(1024, 1024))).toEqual({
            family: "sharpen",
            codename: "moscow",
            precision: "fp32",
            strength: 1,
        });
    });

    it("splits the stored value through the option's own codename", () => {
        // A codename containing an underscore is what a split on the separator gets wrong, and it
        // gets it wrong silently: `sao_paulo_fp32` would send codename `sao` at precision `paulo`.
        const underscored: FamilyEntry = {
            family: "upscale",
            variants: [{ codename: "sao_paulo", label: "São Paulo", precisions: ["fp32", "fp16"], parameters: [] }],
        };

        expect(newOperation("upscale", underscored, "sao_paulo_fp16", sized(2048, 2048))).toEqual({
            family: "upscale",
            codename: "sao_paulo",
            precision: "fp16",
            scale: 2,
        });
    });

    it("builds nothing for a family the catalogue publishes no model for", () => {
        expect(newOperation("upscale", undefined, "kyoto_fp16", sized(1024, 1024))).toBeUndefined();
    });
});

describe("how much larger a chain makes a photograph", () => {
    const upscale = (scale: number): Operation => ({
        family: "upscale",
        codename: "kyoto",
        precision: "fp32",
        scale,
    });

    it("is the upscale's own scale", () => {
        expect(upscaleFactor([upscale(4)])).toBe(4);
    });

    it("is one for a stack that enlarges nothing", () => {
        // Which is every stack without an upscale in it, and the empty one - the navbar then reports
        // the framing's size, or the file's, rather than a product.
        expect(upscaleFactor([])).toBe(1);
    });

    it("is one for a scale that could only have arrived through a fault", () => {
        // The control is bounded to the catalogue's range, so none of these is reachable by a user.
        // A navbar reading "0 x 0" is a worse answer to a fault than the photograph's own size.
        expect(upscaleFactor([upscale(0)])).toBe(1);
        expect(upscaleFactor([upscale(-2)])).toBe(1);
        expect(upscaleFactor([upscale(Number.NaN)])).toBe(1);
        expect(upscaleFactor([upscale(Number.POSITIVE_INFINITY)])).toBe(1);
    });
});

describe("the operation an Autopilot suggestion becomes", () => {
    // A photograph whose header says 2x, so a suggestion carrying anything else is visibly the analysis's own.
    const photograph = sized(2048, 2048);

    // Each family's stored default, so "the user's model" is a choice rather than the catalogue's first entry.
    const stored: Record<Exclude<Family, "detection">, string> = {
        denoise: "malmo_fp16",
        sharpen: "novgorod_fp32",
        light_adjustment: "lyon_fp16",
        color_balance: "saopaulo_fp32",
        colorization: "mumbai_fp16",
        face_recovery: "santorini_fp32",
        upscale: "saitama_fp16",
    };

    it.each(ENHANCEMENTS.map(({ family }) => family).filter((family) => family !== "upscale"))(
        "is exactly what the add menu builds for %s, with the user's stored model",
        (family) => {
            const built = suggestedOperation({ family }, entry(family), stored[family], photograph);

            expect(built).toEqual(newOperation(family, entry(family), stored[family], photograph));
            expect(built?.codename).toBe(stored[family].split("_")[0]);
        },
    );

    it("carries the scale the analysis chose for an upscale, not the one the header implies", () => {
        const built = suggestedOperation({ family: "upscale", scale: 4 }, entry("upscale"), stored.upscale, photograph);

        expect(defaultScale(photograph)).toBe(2);
        expect(built).toEqual({ family: "upscale", codename: "saitama", precision: "fp16", scale: 4 });
    });

    it("builds nothing for a family the catalogue publishes no model for", () => {
        expect(suggestedOperation({ family: "denoise" }, undefined, undefined, photograph)).toBeUndefined();
        expect(suggestedOperation({ family: "upscale", scale: 2 }, undefined, undefined, photograph)).toBeUndefined();
    });
});
