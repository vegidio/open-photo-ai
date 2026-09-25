import type { ParseKeys, TFunction } from "i18next";
import { type LucideIcon, Maximize2, Paintbrush, Palette, ScanFace, Sun, Triangle, Waves } from "lucide-react";
import { type Suggestion, suggestedScale } from "@/ipc/autopilot";
import type { Family, FamilyEntry, ParameterEntry, Precision, VariantEntry } from "@/ipc/catalogue";
import { type CropInfo, framedDimensions } from "@/ipc/crop";
import type { Operation } from "@/ipc/enhance";
import type { ImageRecord } from "@/ipc/images";
import { report } from "@/lib/report";

/**
 * One enhancement this application presents, and what it is called.
 *
 * Two name keys rather than one, because they are different sentences in different places: the
 * name is the enhancement's own, shared with the add menu, the sidebar and the settings dialog; the
 * short name belongs to the chip drawn over a running preview, which is 148px wide.
 *
 * **The short name is a key of its own rather than a truncation of the full one.** Several
 * languages need a different word rather than fewer characters of the same one - the reference
 * records Russian shortening "Восстановление лиц" to "Лица" - and the catalogues have carried both
 * since `add-gui-1-foundation` ported them whole.
 */
export type Enhancement = {
    family: Family;
    nameKey: ParseKeys;
    /** The name the progress chip draws, in the language the rest of the window is speaking. */
    shortNameKey: ParseKeys;
    /**
     * The glyph a menu entry and a sidebar row are drawn with.
     *
     * A component rather than a name resolved through a switch, which is what the reference's
     * `Icon` atom is: every consumer here renders `<entry.icon />` directly, so an enhancement added
     * to this list cannot be added without one, and nothing has to keep a second table in step.
     */
    icon: LucideIcon;
};

/**
 * The seven enhancements, in the order the add menu offers them.
 *
 * **This is the frontend's only list of domain names, and it is not model vocabulary.** It is which
 * enhancements this application presents and in what order - a presentation decision, which is why
 * it lives in a front end.
 *
 * **Not the order a chain runs in.** That one is the library's: `Opai::process` puts every chain into
 * `Family::APPLY_ORDER`, and the catalogue publishes each family's place in it, which is what a stack is
 * kept in - see {@link applyOrder}. The two lists happen to agree today, as the reference's one list did:
 * its comment is that *"the add menu offering them in one order while the pipeline ran them in another
 * would be a silent inconsistency."* A test holds them to each other.
 *
 * **`detection` is named nowhere here.** The catalogue publishes it - deliberately, so that "which
 * model detects faces" is not something a front end restates - but it is not an enhancement a user
 * adds, so it gets no row. That is how the eighth family stays out of a dialog about enhancements
 * without the catalogue being filtered on the wire.
 */
export const ENHANCEMENTS = [
    {
        family: "denoise",
        nameKey: "enhancements.denoise.name",
        shortNameKey: "preview.progress.denoise",
        icon: Waves,
    },
    {
        family: "face_recovery",
        nameKey: "enhancements.faceRecovery.name",
        shortNameKey: "preview.progress.faceRecovery",
        icon: ScanFace,
    },
    {
        family: "colorization",
        nameKey: "enhancements.colorization.name",
        shortNameKey: "preview.progress.colorization",
        icon: Paintbrush,
    },
    {
        family: "light_adjustment",
        nameKey: "enhancements.lightAdjustment.name",
        shortNameKey: "preview.progress.lightAdjustment",
        icon: Sun,
    },
    {
        family: "color_balance",
        nameKey: "enhancements.colorBalance.name",
        shortNameKey: "preview.progress.colorBalance",
        icon: Palette,
    },
    {
        family: "sharpen",
        nameKey: "enhancements.sharpen.name",
        shortNameKey: "preview.progress.sharpen",
        icon: Triangle,
    },
    {
        family: "upscale",
        nameKey: "enhancements.upscale.name",
        shortNameKey: "preview.progress.upscale",
        icon: Maximize2,
    },
] as const satisfies readonly Enhancement[];

/**
 * The order a chain runs its families in, read off the catalogue: every family that publishes a place
 * in one, by that place. `detection` publishes none and is left out.
 *
 * **The library's order, not this side's**: `Opai::process` sorts every chain into `Family::APPLY_ORDER`
 * whatever order it is sent in, so a stack kept in this order is drawn in the order it will actually run.
 *
 * Empty while the catalogue has not arrived, which sorts nothing - and nothing is added before it has,
 * because `newOperation` builds nothing without a model to name.
 */
export const applyOrder = (families: readonly FamilyEntry[]): Family[] =>
    families
        .flatMap(({ family, order }) => (order === undefined ? [] : [{ family, order }]))
        .sort((left, right) => left.order - right.order)
        .map(({ family }) => family);

/**
 * The families `keep` holds, in {@link ENHANCEMENTS}' order - so a list built from them does not
 * depend on the order they were chosen or stored in.
 */
export const familiesWhere = (keep: (family: Family) => boolean): Family[] =>
    ENHANCEMENTS.map(({ family }) => family).filter(keep);

/**
 * Nothing, or the compile error that the wire carries a family {@link ENHANCEMENTS} forgot.
 *
 * The same shape `stores/settings.ts` uses for its snapshot keys: `Exclude` is `never` exactly while
 * every member of `Operation`'s union is named in the list, so a family added to the wire and not to
 * the list stops the typecheck, naming itself in the error. `as const` is what makes this readable:
 * it keeps each entry's `family` as its literal, so the list's own type says which families it holds,
 * while `satisfies` still checks every entry against {@link Enhancement}.
 *
 * The reverse needs no check here: an entry whose family the wire cannot carry is a type error where
 * the add menu hands it to {@link newOperation}.
 */
type Unoffered<T extends never = Exclude<Operation["family"], (typeof ENHANCEMENTS)[number]["family"]>> = T;
export type _EveryOperationIsOffered = Unoffered;

/**
 * The quality tiers a chooser has a name for, in the order the catalogue publishes precisions.
 *
 * `md` is the catalogue key behind the label "SD"; renaming it would touch all thirteen catalogues,
 * so the key and the string differ on purpose.
 */
const TIERS = ["hd", "md"] as const;

export type QualityTier = (typeof TIERS)[number];

/**
 * How an option's quality is named: by the tier its position has a name for, or by the precision's
 * own spelling where it has none.
 *
 * The second case is not defensive. A variant publishing a third precision would land on a tier the
 * catalogues have no word for, and inventing one - or reusing "SD" - would mislabel it; showing
 * `int8` says exactly what it is. Absent altogether is the third case, handled by
 * {@link ModelOption.quality} being optional: a tier label on a model published at one precision
 * says nothing, because there is nothing to tell it apart from.
 */
export type ModelQuality = { kind: "tier"; tier: QualityTier } | { kind: "precision"; precision: Precision };

/** One model at one precision, as the chooser offers it. */
export type ModelOption = {
    /** `<codename>_<precision>` - the value stored, in the reference's own spelling. */
    value: string;
    /**
     * The model this option is a precision of.
     *
     * Carried so a chooser can tell where one model's tiers end and the next model's begin - which
     * is what the settings rows draw a separator on. Splitting the value string at the call site
     * would work today and stop working the first time a codename contains an underscore; the
     * reference keeps the same field, for the same reason.
     */
    codename: string;
    /**
     * The precision half of {@link value}, carried rather than recovered.
     *
     * `modelChoice` maps over `variant.precisions`, so it has this exactly; a call site that reads
     * it back off `value` has to slice at the codename's length and then assert the result is a
     * `Precision`, which is an unchecked cast over a string the catalogue already typed. Carrying
     * it keeps the composition of `value` private to this module - see {@link modelValue} and
     * {@link optionFor}, which are the only two things that need to know the format at all.
     */
    precision: Precision;
    /**
     * Whether this is the first option of its model, which is where a chooser marks a boundary.
     *
     * The producer knows this exactly - it is `index === 0` over the variant's own precisions -
     * where a consumer can only approximate it by comparing against the previous option's codename,
     * and that approximation holds only while the options stay grouped in source order. Both
     * choosers draw off this field, so the tray's marker and the settings rows' separator cannot
     * fall out of step.
     */
    first: boolean;
    /** The model's display name, straight from the catalogue. Never composed from. */
    label: string;
    /** Absent where this model publishes one precision and a tier would say nothing. */
    quality?: ModelQuality;
};

/**
 * What an option's quality is called, or `undefined` where it has none to name.
 *
 * Two callers draw it differently and only this much is shared: the settings row composes it into
 * `models.label` beside the model's name, and a sidebar row puts it at the end of an info line. A
 * model published at one precision answers `undefined` - a tier label says nothing where there is
 * nothing to tell it apart from - and each caller decides what to do with that.
 *
 * A tier is translated and a precision is not: `int8` is the library's own spelling and naming it
 * anything else would be this application inventing a word for a build it has none for.
 */
export const qualityLabel = (t: TFunction, quality: ModelQuality | undefined): string | undefined => {
    if (!quality) return undefined;

    return quality.kind === "tier" ? t(`models.quality.${quality.tier}`) : quality.precision;
};

/**
 * What one option is called: the model's own name, and the quality it is published at where that
 * says anything.
 *
 * The model's half is never translated - the names are proper nouns and the catalogue publishes
 * them - and the composition is, because the order of a name and its qualifier is not universal.
 * Both choosers in this application draw exactly this, which is what stops them naming the same
 * model two different ways.
 */
export const modelLabel = (t: TFunction, option: ModelOption): string => {
    const quality = qualityLabel(t, option.quality);

    return quality ? t("models.label", { model: option.label, quality }) : option.label;
};

/** What a chooser draws for one family: everything on offer, and the one that is selected. */
export type ModelChoice = {
    options: ModelOption[];
    /** Always one of `options`' values, or `""` where the family offers nothing at all. */
    selected: string;
};

/** The quality naming for one position in one variant's published precisions. */
const qualityAt = (variant: VariantEntry, index: number): ModelQuality | undefined => {
    // One precision, so there is no second option to distinguish this from.
    if (variant.precisions.length < 2) return undefined;

    const tier = TIERS[index];

    return tier ? { kind: "tier", tier } : { kind: "precision", precision: variant.precisions[index] as Precision };
};

/**
 * Everything one family offers and which of it is selected, given what the user stored.
 *
 * **The tiers are read positionally**, off each variant's own published precisions: `precisions[0]`
 * is HD, `precisions[1]` is SD. The reference keeps a map for this - `{ hd: 'fp32', md: 'fp16' }`
 * with an override for Osaka, whose comment explains that *"no fp32 build of it was ever published,
 * and its diffusion transformer is now also published quantised to int8"* - because the Go bindings
 * publish no order. The Rust catalogue does, and pins it: `FloatPrecision::PRECISIONS` is
 * `[Fp32, Fp16]` and `OsakaPrecision::PRECISIONS` is `[Fp16, Int8]`, each documented as *"in the
 * order a chooser should offer them"*. Reading the position therefore reproduces that map exactly,
 * including its one override, from data the library already guarantees - and generalises where the
 * map does not, which `OsakaPrecision`'s own doc asks for: a catalogue that decided which pair a
 * model gets would be *"silently wrong the moment a second diffusion model published a different
 * pair"*.
 *
 * **Three things can be wrong with the stored selection, and one rule covers all three.** It can be
 * absent, because the user never chose for this family; it can name a model the library no longer
 * publishes; or it can name a precision a published model no longer offers. All three fall back to
 * the first variant at its first precision - which is also the default for a family never chosen
 * for, so the default requirement and the repair requirement are satisfied by the same line.
 *
 * **That default is the catalogue's own order** rather than a table of codenames here. The
 * catalogue's order is documented as *"the order a chooser should offer them"*, so its first entry
 * is already the library's answer to what a user should see first. It agrees with the reference for
 * six of the seven families and differs for upscale, which defaults to Tokyo here and Kyoto there -
 * the slice's one declared parity gap, taken because both alternatives cost more: seven codenames in
 * a front end is exactly the duplication `opai::catalogue()` exists to remove, and a `default` field
 * on the library's catalogue is a preference `autopilot.rs` has already refused to hold.
 *
 * A family the catalogue does not publish at all answers with nothing on offer, rather than
 * throwing: it is the same class of drift as a retired model, and a row drawing an empty chooser is
 * a better answer than a dialog that does not open.
 */
export const modelChoice = (entry: FamilyEntry | undefined, stored: string | undefined): ModelChoice => {
    const options = (entry?.variants ?? []).flatMap((variant) =>
        variant.precisions.map((precision, index) => {
            const quality = qualityAt(variant, index);

            return {
                value: `${variant.codename}_${precision}`,
                codename: variant.codename,
                precision,
                first: index === 0,
                label: variant.label,
                ...(quality && { quality }),
            } satisfies ModelOption;
        }),
    );

    const selected = options.some((option) => option.value === stored) ? stored : options[0]?.value;

    return { options, selected: selected ?? "" };
};

/**
 * The stored value naming the model an operation is running, in the reference's own spelling.
 *
 * One of two places that knows `value` is `<codename>_<precision>` - {@link modelChoice}, which
 * builds it, is the other - so a caller wanting the option an operation corresponds to composes
 * nothing and asks {@link optionFor} instead.
 */
export const modelValue = (operation: Pick<Operation, "codename" | "precision">): string =>
    `${operation.codename}_${operation.precision}`;

/**
 * The option a stored value names, or `undefined` where the catalogue no longer publishes it.
 *
 * Asks {@link modelChoice} for the whole list rather than for a selection, because what a caller
 * wants here is the entry matching the value it actually carries - not the repaired one a chooser
 * would fall back to. A model the catalogue has stopped publishing therefore answers `undefined`,
 * which is the caller's cue to report the operation by its own codename rather than to throw.
 */
export const optionFor = (entry: FamilyEntry | undefined, value: string): ModelOption | undefined =>
    modelChoice(entry, value).options.find((candidate) => candidate.value === value);

/**
 * The range `entry` publishes as `name` for the model `codename`, or `undefined` where that model
 * publishes none - or the catalogue has not arrived.
 *
 * **The selected model's**, not the family's, because `VariantEntry.parameters` is per model: two models
 * of one family may disagree, and a control built from a family-wide list would offer one of them a value
 * its sibling refuses. **Named rather than found by kind**, because a model may publish more than one
 * range - Athens publishes a fidelity beside its faces - and "the only range" would then be the wrong one.
 * The predicate narrows as well as finds: the bounds only exist on the `range` arm.
 */
export const publishedRange = (
    entry: FamilyEntry | undefined,
    codename: string,
    name: string,
): (ParameterEntry & { kind: "range" }) | undefined =>
    entry?.variants
        .find((published) => published.codename === codename)
        ?.parameters.find(
            (parameter): parameter is ParameterEntry & { kind: "range" } =>
                parameter.name === name && parameter.kind === "range",
        );

/** Every range `variant` publishes, at the value a newly added enhancement starts it at. */
const publishedDefaults = (variant: VariantEntry | undefined): Record<string, number> =>
    Object.fromEntries(
        (variant?.parameters ?? []).flatMap((parameter) =>
            parameter.kind === "range" ? [[parameter.name, parameter.default]] : [],
        ),
    );

/**
 * The value `operation` carries for the parameter published as `name`, or the catalogue's default for
 * it where it carries none - `undefined` only where neither knows one.
 *
 * An operation built by {@link newOperation} carries every range its model publishes, so the fallback is
 * for one whose model was switched to a sibling that publishes a parameter the first did not.
 */
export const parameterOf = (operation: Operation, entry: FamilyEntry | undefined, name: string): number | undefined =>
    operation.parameters[name] ?? publishedRange(entry, operation.codename, name)?.default;

/** `operation` with the parameter published as `name` set to `value`, and everything else as it was. */
export const withParameter = (operation: Operation, name: string, value: number): Operation => ({
    ...operation,
    parameters: { ...operation.parameters, [name]: value },
});

/**
 * `operation` switched to the model a chooser's `value` names, or `undefined` where the catalogue no
 * longer publishes it.
 *
 * A chosen model is resolved to the option it names rather than parsed: the option carries both halves
 * as fields, so nothing here knows how the value is composed. One the catalogue no longer publishes
 * resolves to nothing, which a caller ignores - leaving the enhancement on the model it is already
 * running.
 *
 * **The values already set are kept**, and a parameter the new model publishes that the operation does
 * not carry yet arrives at its published default - so switching Santorini to Athens starts Athens'
 * fidelity where a new Athens would. A value the new model does not read is kept too: switching back
 * finds it where it was, and Rust ignores it meanwhile.
 */
export const withModel = (
    operation: Operation,
    entry: FamilyEntry | undefined,
    value: string,
): Operation | undefined => {
    const option = optionFor(entry, value);
    if (!option) return undefined;

    const variant = entry?.variants.find((published) => published.codename === option.codename);

    return {
        ...operation,
        codename: option.codename,
        precision: option.precision,
        parameters: { ...publishedDefaults(variant), ...operation.parameters },
    };
};

/**
 * The families a progress report can carry that are not enhancements a user adds, and the enhancement
 * each is reported **as**.
 *
 * One entry: a detection is run for a face recovery and nothing else, so what the user should read on
 * the chip is the enhancement they asked for. The detector is offered by no menu in this application
 * and named in no catalogue row, so naming it there would name a model nobody chose.
 *
 * **Honest on the wire, named here.** Rust sends `detection`, because the operation *is* a detection
 * and saying otherwise would make the report's own diagnostic (`New York (FP32)`) disagree with its
 * family. Which enhancements this application presents and what it calls them is already this file's
 * business - the same decision that keeps `detection` out of {@link ENHANCEMENTS} - so the mapping is
 * one entry beside that list rather than a lie on the wire. See design.md D8.
 */
const REPORTED_AS: Partial<Record<Family, Operation["family"]>> = { detection: "face_recovery" };

/**
 * What the progress chip calls the family a report carries, or `undefined` where this application has
 * no name for it.
 *
 * Derived from {@link ENHANCEMENTS} rather than a second table of short names: the seven that are
 * enhancements answer their own, and {@link REPORTED_AS} redirects the one that is not.
 *
 * `undefined` is the honest answer for a family the catalogue publishes and `ENHANCEMENTS` does not
 * name - the same class of drift a retired model is, and the caller draws its generic label for it
 * rather than throwing.
 */
export const progressLabelKey = (family: Family | undefined): ParseKeys | undefined => {
    if (!family) return undefined;

    const reported = REPORTED_AS[family] ?? family;

    return ENHANCEMENTS.find((entry) => entry.family === reported)?.shortNameKey;
};

/**
 * What the library calls an upscale's multiplier: its own `Scale::NAME`, the name the catalogue publishes
 * the range under and the operation carries the value under.
 */
const SCALE = "scale";

/**
 * The scale a freshly added upscale starts at, for `file` as it is framed by `crop`: 4x for a small
 * photograph, 2x for a medium one, 1x for a large one.
 *
 * **The ladder is the library's**, asked through {@link suggestedScale} rather than restated here: it
 * is the one Autopilot's suggestion is read off, so the two cannot disagree about one photograph. A
 * small photograph can afford more enlargement than a large one, and a fixed multiplier would turn a
 * 6000x4000 source into a 24000x16000 result on the strength of nothing.
 *
 * **Measured on the framed size**, which is what will actually be upscaled - the same size the
 * analysis measures. A tight crop of a large photograph is a small photograph.
 *
 * **A photograph with no dimensions gets 1x** without asking, which is the conservative end rather
 * than the arithmetic one. The dimensions are absent exactly when this application could not parse the
 * file's header and it carries no framing, so nothing is known about how large it is - and the
 * reference's own expression answers 4x there, because an unknown size multiplies out to zero pixels
 * and lands in the smallest bucket. Enlarging an unmeasured photograph fourfold is the worst of the
 * three guesses. A question the bridge fails to answer lands on the same 1x, for the same reason.
 */
export const startingScale = async (file: ImageRecord | undefined, crop: CropInfo | undefined): Promise<number> => {
    const { width, height } = framedDimensions(file, crop);
    if (width === undefined || height === undefined) return 1;

    try {
        return await suggestedScale(width, height);
    } catch (error) {
        report("asking for the scale a new upscale starts at failed", error);
        return 1;
    }
};

/**
 * How much larger a chain makes the photograph it runs over: the upscale's own scale, or 1 where the
 * stack holds none.
 *
 * Here rather than inline in the navbar, beside {@link startingScale} and the other derivations of
 * this shape, because it is the same question those answer - what the stack implies about the
 * photograph's size - and the navbar is the second region to depend on the stack's shape rather than
 * the owner of that dependency. The reference keeps the same function, in the same place.
 *
 * The **first** upscale rather than the product of every one of them: the sidebar offers one
 * enhancement per family, so a stack holds at most one. A second would be a chain this application
 * cannot build, and multiplying for it would be answering a question nobody can ask.
 *
 * A scale at or below zero is read as 1, which is the reference's own guard. The control is bounded
 * to the catalogue's range, so such a value can only arrive through a fault - and a navbar reading
 * "0 x 0" is a worse answer to a fault than the photograph's own size.
 */
export const upscaleFactor = (operations: readonly Operation[]): number => {
    const scale = operations.find((operation) => operation.family === "upscale")?.parameters[SCALE];

    return scale !== undefined && Number.isFinite(scale) && scale > 0 ? scale : 1;
};

/**
 * The operation a newly added enhancement carries: the user's default model for that family, with every
 * parameter that model publishes at the catalogue's published default.
 *
 * **The defaults are the library's**, published beside the bounds on each range - a strength of 1, the
 * model's own output; a bias of 0.5; Athens' maximum fidelity - so no number here restates one. The one
 * exception is an upscale's `scale`, the factor it starts at: {@link startingScale}'s answer for the
 * photograph, asked by the caller because it is a question for the backend, and read by no other family.
 *
 * **The model is resolved through {@link modelChoice}**, which is the same call the settings rows
 * make - so the default for a family never chosen for, the repair of a selection naming a model the
 * catalogue no longer publishes, and the positional HD/SD tier rule are the ones already shipped
 * rather than a second implementation of all three.
 *
 * **Nothing here parses the `<codename>_<precision>` value.** `ModelOption` carries the codename and
 * the precision as fields, so the format stays private to {@link modelValue} and {@link optionFor} -
 * which is what stops a call site splitting on `_`, a thing that works today and breaks silently the
 * first time a codename contains one.
 *
 * `undefined` for a family the catalogue publishes nothing for, which is the same drift
 * `modelChoice` already answers with an empty list: there is no model to name, so there is no
 * operation to build, and the menu entry that asked simply adds nothing.
 */
export const newOperation = (
    family: Operation["family"],
    entry: FamilyEntry | undefined,
    stored: string | undefined,
    scale = 1,
): Operation | undefined => {
    const { options, selected } = modelChoice(entry, stored);
    const option = options.find((candidate) => candidate.value === selected);

    if (!option) return undefined;

    const variant = entry?.variants.find((published) => published.codename === option.codename);
    const operation: Operation = {
        family,
        codename: option.codename,
        precision: option.precision,
        parameters: publishedDefaults(variant),
    };

    /*
     * **No faces and no choice among them** for a face recovery: the stack is the user's choice about the
     * enhancement, and the faces are a property of the pixels, which change when the framing does. The run
     * finds them itself, and the choice is put in by `withChoice` on the way to `enhance`, so a framing
     * change rewrites no photograph's stack. See design.md D5.
     *
     * The one value that is the photograph's rather than the catalogue's, an upscale's scale: see
     * `startingScale`.
     */
    return family === "upscale" ? withParameter(operation, SCALE, scale) : operation;
};

/**
 * The operation an Autopilot suggestion becomes: exactly what the add menu would add for that family, except
 * that an upscale carries the scale the analysis chose.
 *
 * **Built through {@link newOperation}** rather than beside it, so a change to the menu's defaults moves both.
 * The upscale carries the scale the analysis chose, which is the library's ladder read off the framed pixels -
 * the same answer {@link startingScale} would have asked for.
 *
 * `undefined` for a family the catalogue publishes no model for, as {@link newOperation} answers it.
 *
 * A family the wire's `Suggestion` gains that `Operation` does not carry is a type error at the call below.
 */
export const suggestedOperation = (
    suggestion: Suggestion,
    entry: FamilyEntry | undefined,
    stored: string | undefined,
): Operation | undefined => {
    const operation = newOperation(suggestion.family, entry, stored);

    return operation?.family === "upscale" && suggestion.family === "upscale"
        ? withParameter(operation, SCALE, suggestion.scale)
        : operation;
};
