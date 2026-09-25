import { listen } from "@tauri-apps/api/event";
import type { Family, Precision } from "./catalogue";
import type { CropInfo } from "./crop";
import type { Face } from "./faces";
import { call } from "./invoke";
import type { SupportedProviders } from "./setup";

/**
 * The names of the Rust commands, written out once on this side of the boundary.
 *
 * Each is written a second time in `crates/gui/src/enhance/mod.rs`, as the name of a function carrying
 * `#[tauri::command]`, and nothing in either toolchain notices when one of the two is renamed alone:
 * a rename here is not a type error, it is an `invoke` that rejects at runtime. `enhance.test.ts`
 * pins this half; the Rust half is pinned by `generate_handler![]` refusing to compile against a
 * function that does not exist.
 */
const ENHANCE_COMMAND = "enhance";
const CANCEL_ENHANCE_COMMAND = "cancel_enhance";
const RELEASE_ENHANCED_COMMAND = "release_enhanced";
const RELEASE_ALL_ENHANCED_COMMAND = "release_all_enhanced";

/**
 * The event a run's progress reports arrive under, written a second time in `crates/gui/src/enhance/progress.rs`
 * as `PROGRESS_EVENT`.
 *
 * A global event rather than a channel, unlike `initialize`'s. The two differ in what a late listener
 * costs: setup's plan fires within milliseconds of the command starting and a listener registered in
 * parallel with the invoke can miss it, which is unrecoverable. Here every report names its own run,
 * so a window that subscribes once can tell a report about the run it is drawing from one about a run
 * it has abandoned - which is also what a channel per call could not do, since a displaced run's
 * channel goes on delivering.
 */
const PROGRESS_EVENT = "enhance:progress";

/**
 * The processor to run on, spelled as the settings store spells it.
 *
 * Derived from `SupportedProviders` rather than written out, for the reason `stores/settings.ts`
 * gives about its own identical derivation: a provider the library adds and `ipc/setup.ts` names
 * arrives in this type without this file being edited, and one renamed on the wire is a type error
 * rather than an option that quietly stops matching.
 *
 * These are the wire's field names (`tensorrt`), not the library's own enum spellings (`TensorRT`).
 * Rust translates once, in `enhance/run.rs`, because two spellings of one provider is the drift both
 * sides are spent avoiding.
 */
export type Processor = "auto" | keyof SupportedProviders;

/**
 * One operation to run, as the window names it.
 *
 * Discriminated by `family`, spelled as Rust's `Family` spells it, carrying the model's codename and
 * the precision to run it at - both of which come straight from the `catalogue` command, so a chooser
 * built from the catalogue sends back exactly what it was given.
 *
 * **Not a composed string.** The Wails application builds `up_kyoto_2x_fp32` here and parses it back
 * in Go, and its own comment records the cost: the string is a contract in three places at once. None
 * of those forces apply here - the run cache key is derived inside the library from the operation
 * itself, and the pixels travel over the `opai://` scheme rather than through a cache of this side's
 * own.
 *
 * **A union, one member per enhancement the window presents**, rather than one shape with optional
 * properties: each family's own parameters sit inside its member, which is why the scale is part of
 * `upscale` and the faces part of `face_recovery` rather than both hanging off `codename`. Detection has
 * no member: the window asks for faces through its own command, never as an operation in a chain.
 */
export type Operation =
    | {
          family: "upscale";
          /** Which model, as `VariantEntry.codename` publishes it. */
          codename: string;
          /** Which build of it, as `VariantEntry.precisions` publishes them. */
          precision: Precision;
          /**
           * How much larger the result is.
           *
           * Brought inside the range the catalogue publishes rather than refused: the control that
           * drives it is bounded to the same range, so a value outside it can only arrive through a
           * fault, and a refused enhancement is not what a user should see for one. The fault stays
           * in the Rust log.
           */
          scale: number;
      }
    | {
          family: "denoise";
          /** Which model, as `VariantEntry.codename` publishes it. */
          codename: string;
          /** Which build of it, as `VariantEntry.precisions` publishes them. */
          precision: Precision;
          /**
           * How strongly the model's output is applied: the library's unit value, 0..3, where 0 leaves the
           * photograph unchanged, 1 is the model's own output and above 1 amplifies it - not the percentage
           * the window shows. Converted and clamped as a bias is.
           */
          strength: number;
      }
    | {
          family: "face_recovery";
          /** Which model, as `VariantEntry.codename` publishes it. */
          codename: string;
          /** Which build of it, as `VariantEntry.precisions` publishes them. */
          precision: Precision;
          /**
           * The faces to restore, as the detector found them in the photograph **as it is framed**.
           *
           * **Empty in the stack, filled at the boundary.** `stores/enhancements.ts` holds what the
           * user chose, and a user chooses a model rather than a set of faces: the faces are a
           * property of the pixels and change when the framing does, so putting them in the stack
           * would mean every framing change rewrote every photograph's stack. `useEnhancementRun`
           * puts them in on the way to {@link enhance}, which is the one place they are resolved.
           *
           * **The order matters.** Rust's `Faces` keeps the order it is given and folds an
           * order-sensitive signature of the bounding boxes into the run cache tag, so a reordered
           * selection asks for a different result rather than the same one.
           *
           * **There is no fidelity beside this.** The window runs every face recovery at maximum
           * fidelity and offers no control for it, so a field here would describe a choice the user
           * cannot make. It is fixed in `crates/gui/src/enhance/operation.rs`.
           */
          faces: Face[];
      }
    | {
          family: "light_adjustment";
          /** Which model, as `VariantEntry.codename` publishes it. */
          codename: string;
          /** Which build of it, as `VariantEntry.precisions` publishes them. */
          precision: Precision;
          /**
           * Which way, and how far, the photograph is shifted: the library's unit value, -1..1 about a
           * neutral 0, not the percentage the window shows.
           *
           * Unit rather than percent so the catalogue's published range and the range the wire accepts are
           * the same numbers; the percentage is the options panel's presentation, converted there. Brought
           * inside the range rather than refused, for the reason the scale is.
           */
          bias: number;
      }
    | {
          family: "color_balance";
          /** Which model, as `VariantEntry.codename` publishes it. */
          codename: string;
          /** Which build of it, as `VariantEntry.precisions` publishes them. */
          precision: Precision;
          /**
           * Which way, and how far, the photograph's colours are shifted: the library's unit value, -1..1
           * about a neutral 0, not the percentage the window shows - the same `opai::Bias` a light
           * adjustment carries, converted and clamped the same way.
           */
          bias: number;
      }
    | {
          family: "sharpen";
          /** Which model, as `VariantEntry.codename` publishes it. */
          codename: string;
          /** Which build of it, as `VariantEntry.precisions` publishes them. */
          precision: Precision;
          /**
           * How strongly the model's output is applied: the library's unit value, 0..3, where 0 leaves the
           * photograph unchanged, 1 is the model's own output and above 1 amplifies it - the same
           * `opai::Strength` a denoise carries, converted and clamped the same way.
           */
          strength: number;
      }
    | {
          /**
           * Carries no value: colorization takes no parameter, so a field here would describe a choice the
           * user cannot make.
           */
          family: "colorization";
          /** Which model, as `VariantEntry.codename` publishes it. */
          codename: string;
          /** Which build of it, as `VariantEntry.precisions` publishes them. */
          precision: Precision;
      };

/** What a report says is happening, as Rust's `Stage` spells it. */
export type Stage = "installing" | "running";

/**
 * One report about a run in flight.
 *
 * `stage` is **absent** for an operation whose result the application already had. It was neither
 * fetched nor run, and reporting it as running at completion would jump the bar a whole operation's
 * width with no explanation - so a chain made entirely of such operations finishes without ever
 * saying it did any work, and the interface draws no indicator at all.
 *
 * `installFraction` is the second number and is absent unless a model is actually being fetched. Both
 * are needed while one is: a fetch occupies only the head of one operation's share of the chain, so a
 * bar tracking `chainFraction` barely moves during one, and a multi-gigabyte download without its own
 * figure is indistinguishable from a stall.
 *
 * Reports are thinned in Rust, to about a hundred per run - a tiled upscale produces thousands and an
 * indicator has about a hundred positions. Nothing here needs to throttle again.
 */
export type RunProgress = {
    /** Which run this is about. A report naming a run the window has abandoned is discardable. */
    run: string;
    /**
     * The operation being carried out, as a user would see it named: `Kyoto 4x (FP16)`.
     *
     * A diagnostic, composed in English by the library. Right for a log and untranslatable by a
     * front end, which is why the family travels beside it.
     */
    operation: string;
    /**
     * Which enhancement the operation belongs to, spelled as the catalogue spells it.
     *
     * What a chip over the preview names the operation from, in the language the rest of the window
     * is speaking. {@link RunProgress.operation} cannot answer that - it is a sentence the library
     * is free to reword - and reconstructing it from the chain the window sent would be right only
     * while a chain held one operation of each family.
     *
     * Reported for every stage alike, including an operation whose result was already known: it did
     * no work, but it still belongs to an enhancement.
     */
    family: Family;
    stage?: Stage;
    /** `0..1`. Never decreases over a run, and reaches 1 exactly once. */
    chainFraction: number;
    /** `0..1` through the fetch itself, while one is happening. */
    installFraction?: number;
};

/**
 * How a run ended.
 *
 * Two outcomes rather than one and a rejection, because **a stop is not a failure**: a user who has
 * changed their mind has not been told their enhancement broke. Both resolve; what a caller branches
 * on is `outcome`.
 *
 * The enhanced arm carries a description and not the pixels. A fourfold enlargement is tens of
 * megabytes, and putting it here would put it on the message channel and make this side responsible
 * for holding it. `identity` is the address `renditionUrl` draws it from, exactly as an opened file's is.
 */
export type Enhancement =
    | {
          outcome: "enhanced";
          /**
           * What the result's pixels are addressed by, composed from the source's identity and every
           * operation applied - so a second chain over the same photograph is never mistaken for this
           * one.
           */
          identity: string;
          width: number;
          height: number;
      }
    /**
     * The run was stopped - by {@link cancelEnhance}, or by a later run displacing it.
     *
     * **A superseded run resolves this way too**, even one that finished its work before noticing it
     * had been displaced: the backend dropped its pixels when it took the slot for the successor, so
     * there is no address to draw. A caller branches on `outcome` and never has to ask whether the
     * run it is holding is still the current one.
     */
    | { outcome: "stopped" };

/** Why an operation could not be run, as Rust's `UnknownOperation` tags it. */
export type UnknownOperation =
    | { kind: "model"; family: string; codename: string }
    | { kind: "precision"; codename: string; precision: Precision };

/**
 * Why the `enhance` command rejected.
 *
 * `kind` is the `#[serde(tag = "kind")]` on Rust's `EnhanceError`. A stop is deliberately **not** one
 * of these - see {@link Enhancement}.
 *
 * `message`, where a member carries one, is the core library's own sentence, composed in English and
 * shown untranslated: it is a diagnostic to be copied into a bug report, and a translated one is a
 * diagnostic nobody reading the report can search for. That is the same rule `ipc/setup.ts` records.
 */
export type EnhanceError =
    | { kind: "notReady" }
    | { kind: "unknownSource"; identity: string }
    | { kind: "unknownOperation"; index: number; reason: UnknownOperation }
    | { kind: "unreadableSource"; identity: string; message: string }
    | { kind: "enhance"; message: string };

/**
 * A name for one run, unique within this application.
 *
 * The prefix is drawn once per load of this module and the counter runs within it, so two runs of one
 * session differ and a run from before a reload cannot collide with one after it. The second half
 * matters: the Rust slot outlives the webview, and a name reused across a reload could land a stop
 * that was meant for the previous page's run on this page's.
 *
 * Not `crypto.randomUUID()`, which would be the obvious reach: it needs a secure context, and what is
 * wanted here is a name the backend compares for equality and nothing more. Nothing about a run is
 * guessable-sensitive.
 */
const SESSION = Math.random().toString(36).slice(2, 10);
let minted = 0;

/**
 * A name for one run reported on {@link onEnhanceProgress}, unique within this application.
 *
 * Exported because a detection is the second thing reported on that event, and `ipc/faces.ts` mints its
 * name from here rather than keeping a counter of its own. **One counter is what makes a detection and
 * an enhancement unable to share a name**, which matters: the window filters progress reports by the
 * run it is drawing, and two independent counters could hand the same string to both.
 */
export const mintRun = () => `${SESSION}-${++minted}`;

/**
 * Run a chain of enhancements over an open image.
 *
 * Answers **the run's name straight away**, beside the promise of its outcome. That is the whole
 * shape of this function and the reason it is not simply `async`: a stop and its run cross the
 * boundary independently and either may arrive first, so an effect that starts a run needs the name
 * of the thing its cleanup will stop before the promise settles.
 *
 * ```ts
 * useEffect(() => {
 *     const { run, done } = enhance(identity, operations, processor);
 *     done.then(draw);
 *
 *     return () => cancelEnhance(run);
 * }, [identity, operations, processor]);
 * ```
 *
 * **Asking for a run stops whatever was running.** The backend enforces that rather than trusting
 * this side to ask, so an unmount or a fault here cannot leave a run holding models in memory and
 * occupying the processor it was given. The explicit stop above is still needed for the case a new
 * run does not cover: removing the only enhancement, or moving to a photograph with none.
 *
 * `source` is the identity of an image the user opened - the same one `renditionUrl` draws it from.
 * An identity the application never admitted is rejected; a location is not an identity at all.
 *
 * `crop` is how the user has framed that image, or absent to run over the whole photograph. **The
 * chain runs over the framed pixels**, so the result is the enhancement of what the window is
 * drawing rather than of the file, and two framings of one photograph answer two different results -
 * neither of which can be served where the other was asked for. A framing that changes nothing costs
 * nothing: Rust answers the source picture unchanged, so opening the framing controls and dismissing
 * them does not discard what has already been computed.
 *
 * The same value goes on a rendition URL through `cropQuery`, in a different spelling. That is two
 * encodings of one {@link CropInfo} because there are two doors, and each is pinned by a test on
 * both sides of the boundary.
 *
 * **An empty chain is a request, not a mistake.** A user who has toggled every enhancement off is
 * asking a meaningful question, and it resolves with the source's own identity.
 *
 * Rejects with the serialized {@link EnhanceError}, typed as `unknown` because that is what an
 * `invoke` rejection is - a promise carries no type for it.
 */
export const enhance = (source: string, operations: Operation[], processor: Processor, crop?: CropInfo) => {
    // Before the invoke, which is what `enhance.test.ts` pins: the caller has the name of the run in
    // the same turn it asked for it, so a cleanup that runs before the command has even been
    // dispatched still has something to name.
    const run = mintRun();

    // `crop` is sent as `undefined` rather than omitted from the object, which serde reads as the
    // `Option<Crop>` being `None` - the same thing an absent key would mean, spelled where a reader
    // can see that no framing was sent.
    return { run, done: call<Enhancement>(ENHANCE_COMMAND, { run, source, operations, processor, crop }) };
};

/**
 * Stop a run this window asked for, by the name {@link enhance} handed back.
 *
 * **Does nothing unless that run is still the one in flight.** A stop for a run the backend has
 * already displaced arrives in the ordinary course of changing an enhancement - this side's cleanup
 * and its next request race - and acting on it would stop the run the user is waiting for.
 *
 * A stop that arrives *before* its own run still lands on it, which is the other half of why the name
 * is minted here rather than returned by the command.
 *
 * Resolves whatever happened; there is no outcome to act on, and a run that had already finished is
 * not an error.
 */
export const cancelEnhance = (run: string) => call<void>(CANCEL_ENHANCE_COMMAND, { run });

/**
 * Subscribe to every run's progress reports.
 *
 * One subscription for the application rather than one per run: each report names its own run, so a
 * listener compares {@link RunProgress.run} against the run it is drawing and discards the rest. A
 * displaced run goes on reporting until it notices it has been stopped, and this is what makes those
 * reports discardable rather than confusing.
 *
 * Resolves with the function that removes the listener, as `listen` does.
 */
export const onEnhanceProgress = (handler: (report: RunProgress) => void) =>
    listen<RunProgress>(PROGRESS_EVENT, (event) => handler(event.payload));

/**
 * Release the enhanced result made from an image that has been closed.
 *
 * **Names the image, not the result.** The identity is the one on the `ImageRecord` the file store is
 * removing, so nothing has to be looked up to make the call - and naming it is what makes the race
 * safe: closing one photograph while a run over another is in flight is ordinary, and a release
 * meaning "drop whatever you hold" would throw away that run's result.
 *
 * **This is what stops a large result staying resident for the session.** The backend drops the pixels
 * it holds when a later run displaces them, and until this existed that was the only thing that did -
 * so a user who enhanced a large scan, looked at it and closed it held gigabytes until they enhanced
 * something else.
 *
 * A release naming a photograph the backend holds no result for does nothing, and a run still in
 * flight is left for {@link cancelEnhance} to stop.
 *
 * Resolves whatever happened; there is no outcome to act on. A rejection loses nothing the user can
 * see - the pixels simply stay resident until the next run displaces them, which is what used to
 * happen always - so the caller logs it rather than surfacing it.
 */
export const releaseEnhanced = (identity: string) => call<void>(RELEASE_ENHANCED_COMMAND, { identity });

/**
 * Release the enhanced result, because every image has been closed.
 *
 * The counterpart to {@link releaseEnhanced} for emptying the window at once, on the same terms: a run
 * in flight is left to its own stop, and nothing here is worth surfacing to the user.
 */
export const releaseAllEnhanced = () => call<void>(RELEASE_ALL_ENHANCED_COMMAND);
