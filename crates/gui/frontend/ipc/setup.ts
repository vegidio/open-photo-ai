import { Channel } from "@tauri-apps/api/core";
import { call } from "./invoke";

// Each written a second time in `crates/gui/src/setup.rs`, as a `#[tauri::command]` function's name. A
// rename on one side alone is not a type error but an `invoke` that rejects at runtime: `setup.test.ts`
// pins this half, and `generate_handler![]` the Rust one by refusing to compile against a function
// that does not exist.
const INITIALIZE_COMMAND = "initialize";
const QUIT_COMMAND = "quit";

/** What a component's row is doing, as Rust's `RowState` spells it. */
export type RowState = "installed" | "downloading" | "extracting";

/** One component this machine needs, with the published size of the files behind it. */
export type PlanRow = {
    name: string;
    size: number;
};

/** One report about one component: what it is doing and how far through its own install it is. */
export type ProgressRow = {
    name: string;
    state: RowState;
    /** `0..1`. Never decreases for one component, and reaches exactly 1 only when that component is finished. */
    fraction: number;
};

/**
 * What the Rust side sends while the application starts itself.
 *
 * Discriminated by `kind`, which is the `#[serde(tag = "kind")]` on `SetupEvent`. The plan arrives
 * once, before anything moves; a progress report arrives for each phase of each component that is
 * actually installed, plus one terminal report for each that turned out to need no work.
 */
export type SetupEvent = { kind: "plan"; rows: PlanRow[] } | ({ kind: "progress" } & ProgressRow);

/**
 * Which of the failure dialog's three shapes a failed initialization gets, as Rust's `Failure` spells
 * it.
 *
 * Three rather than one per library error, because there are exactly three renderings: `transfer` is
 * the only one that gets the throttling advice, `unrecoverable` is the only one that does not get Try
 * again, and `other` is what an error case added to the library later arrives as.
 */
export type Failure = "transfer" | "unrecoverable" | "other";

/**
 * Why the `initialize` command rejected.
 *
 * `kind` is the `#[serde(tag = "kind")]` on Rust's `SetupError` and `failure` is the classification
 * beside it.
 *
 * `message` is the core library's own sentence, composed in English and shown untranslated: it is a
 * diagnostic to be copied into a bug report.
 */
export type SetupError = {
    kind: "initialize";
    // Two different words for two different discriminants, which is why this is not called `kind` as well.
    failure: Failure;
    // Untranslated because a translated diagnostic is one nobody reading the report can search for.
    message: string;
};

/**
 * Which execution providers this machine turned out to offer, as Rust's `SupportedProviders` spells
 * it. `cpu` is among them rather than an implied truth.
 *
 * The Rust struct is `#[non_exhaustive]`, so a provider added later arrives as a fifth key this type
 * says nothing about. A provider the frontend does not know is one it does not offer.
 */
export type SupportedProviders = {
    // The four the library publishes today, named so the settings pane's processor list is built from
    // them rather than from a list of provider names written here - and with `cpu` among them, that list
    // is drawn without a special case for the one that is always there.
    //
    // **Named fields and nothing more**, deliberately: an unknown provider going unoffered is correct
    // until someone teaches the frontend its name, and is the opposite of a type that would break a
    // launch over a field nobody asked for.
    cpu: boolean;
    coreml: boolean;
    cuda: boolean;
    tensorrt: boolean;
};

/**
 * Start the application: install what this machine needs and start the inference environment.
 *
 * `onEvent` is attached to the channel **before** the invoke that hands it over, so it sees every
 * event - the plan included - in the order Rust produced them.
 *
 * Resolves with {@link SupportedProviders}: what this machine can be asked to run on, decided inside
 * this call. The Rust `initialize` command says why it rides here rather than on a command of its own.
 *
 * Safe to call more than once. Rust serializes the calls and remembers only success, so a second
 * call while the first is running waits for it, a call after success returns without installing
 * anything, and a call after a failure tries again. A repeat call resolves with the same report.
 *
 * Rejects with the serialized {@link SetupError}, typed as `unknown` rather than `SetupError`
 * because that is what it is: an `invoke` rejects with whatever crossed the boundary, and a promise
 * carries no type for it. The one place it is read narrows it - see `setupFailure` in
 * `stores/setup.ts`.
 */
export const initialize = (onEvent: (event: SetupEvent) => void) => {
    // A channel rather than two global events, because the plan fires within milliseconds of the
    // command starting and a global listener registered in parallel with the invoke can miss it.
    const channel = new Channel<SetupEvent>();
    channel.onmessage = onEvent;

    return call<SupportedProviders>(INITIALIZE_COMMAND, { channel });
};

/**
 * End the application.
 *
 * The way out of a first launch that cannot finish. It resolves only in the sense that the process
 * is on its way down; nothing should be sequenced after it.
 */
export const quit = () => call<void>(QUIT_COMMAND);
