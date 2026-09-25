import { call } from "./invoke";
import { once } from "./once";

// Written a second time in `crates/gui/src/catalogue.rs`, as the `#[tauri::command]` function's name.
// A rename on one side alone is not a type error but an `invoke` that rejects at runtime:
// `catalogue.test.ts` pins this half, and `generate_handler![]` the Rust one by refusing to compile
// against a function that does not exist.
const CATALOGUE_COMMAND = "catalogue";

// Every family rather than the seven a settings row draws: which enhancements exist is model
// vocabulary, and a filter written here would be this application having an opinion about it.
/**
 * One enhancement family, as Rust's `Family` serializes it. Every family the library names is
 * published, `detection` and `colorization` included.
 */
export type Family =
    | "detection"
    | "denoise"
    | "face_recovery"
    | "light_adjustment"
    | "color_balance"
    | "colorization"
    | "sharpen"
    | "upscale";

/** The numeric precision one model artifact is published at, as Rust's `Precision` spells it. */
export type Precision = "fp32" | "fp16" | "int8";

/**
 * What kind of value a published parameter is, as Rust's `ParameterKind` tags it.
 *
 * Two cases a front end has to tell apart that an empty list cannot: `range` is something a user
 * adjusts through a control, bounded by the same constants the library's own constructor enforces,
 * so a slider built from this cannot offer a value that would be refused. `faces` is a set supplied
 * by a detection run rather than by a control - there is no slider to draw, and what it says is that
 * this model needs another operation's output before it can be built at all.
 *
 * The bounds are flattened onto the tag, which is `#[serde(flatten)]` on the Rust side. `default` is what a
 * newly added enhancement starts at, published by the library so no front end restates a number of its own.
 */
export type ParameterKind = { kind: "range"; min: number; max: number; default: number } | { kind: "faces" };

/** One parameter a model takes, named as its own error message names it, and what kind of value it is. */
export type ParameterEntry = { name: string } & ParameterKind;

/** One model a user can choose, and what building it takes. */
export type VariantEntry = {
    /** The developer-facing identifier, which is also what a chosen model is stored as. */
    codename: string;
    /** The display text to show. Never composed from - that is what `codename` is for. */
    label: string;
    /** Every precision this model is published in, in the order a chooser should offer them. */
    precisions: Precision[];
    /**
     * The parameters **this model** takes.
     *
     * Per model rather than per family, because two models of one family may disagree: Athens carries
     * a fidelity and Santorini does not, and a control built from a family-wide list would offer one
     * of them a value its sibling refuses.
     */
    parameters: ParameterEntry[];
};

/** What one family offers: its models, and what each of them takes. */
export type FamilyEntry = {
    family: Family;
    /**
     * Where this family's operation sits in a chain, as `opai`'s `Family::APPLY_ORDER` places it - or absent
     * for `detection`, which is never in one.
     *
     * **The order a chain runs in is the library's.** `Opai::process` puts every chain into it whatever order
     * it is sent in, so a stack drawn in any other order would show one sequence while another ran. Not the
     * order the entries are listed in, which is a presentation decision of its own. Read through
     * `applyOrder` in `lib/enhancements.ts`.
     */
    order?: number;
    variants: VariantEntry[];
};

// Kept because the Rust side answers from a value built once per process and static by construction,
// so a refetch would serialize the same bytes again - and this is the largest payload this
// application puts on the wire.
const fetched = once(() => call<FamilyEntry[]>(CATALOGUE_COMMAND));

/**
 * Everything the library can be asked to run: every family, its models, their labels, the precisions
 * they are published in and the parameters they take with the bounds a control may offer.
 *
 * **Fetched once and kept**, as {@link once} keeps it. Everything that reads it calls this; nothing
 * passes it around.
 *
 * Unfiltered, including the families no settings row draws: the subset a screen wants is that
 * screen's business.
 */
export const catalogue = () => fetched.get();

/** Forgets the fetched catalogue. For tests alone - see {@link once}. */
export const forgetCatalogue = () => fetched.forget();
