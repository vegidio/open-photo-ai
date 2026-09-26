import type { ReactElement, ReactNode } from "react";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { act, type RenderOptions, type RenderResult, render as renderBare } from "@testing-library/react";
import { vi } from "vitest";
import { SettingsDraftProvider, useDraftState } from "@/features/settings/draft";
import type { Family, FamilyEntry, Precision, VariantEntry } from "@/ipc/catalogue";
import type { CropInfo } from "@/ipc/crop";
import type { ExportFormats } from "@/ipc/export";
import type { ImageRecord } from "@/ipc/images";
import { applyOrder } from "@/lib/enhancements";
import type { SetupError, SetupEvent, SupportedProviders } from "@/ipc/setup";
import { AppProviders } from "@/providers";
import { useCropStore } from "@/stores/crop";
import { useFileStore } from "@/stores/files";
import { type Background, type SettingsData, useSettingsStore } from "@/stores/settings";
import { useSetupStore } from "@/stores/setup";

/**
 * What every test in this crate shares: the providers production mounts, and the setup fixtures.
 *
 * The Rust side made this call first - see `inference/test_support.rs`, and the commit that moved
 * `crates/perf`'s helpers into one module. The argument is the same here: a fixture copied into four
 * files is four things that can quietly stop being the same, and the ones below are load-bearing.
 * `SetupFailure.test.tsx` compares what the two states of the dialog say about one plan; that
 * comparison means nothing if the two files' plans have drifted apart.
 */

/**
 * `render`, with the tree wrapped in what `main.tsx` wraps it in.
 *
 * Tests mount three different roots - `App`, `Drawer`, `SetupDialog` - and none of them is the root
 * that ships. Rendering them bare meant they ran without `LucideProvider`'s stroke weight and,
 * once the tooltip provider moved out of the features that were mounting their own, without a
 * tooltip context at all. Going through here is what keeps a tree under test the tree that ships.
 */
export const render = (ui: ReactElement, options?: Omit<RenderOptions, "wrapper">): RenderResult =>
    renderBare(ui, { wrapper: AppProviders, ...options });

/**
 * The canvas's rectangle in a 800 x 600 window: a 40px navbar above it, a 48px folded drawer below.
 *
 * jsdom lays nothing out, so every rectangle it would report is zero-sized and every drag would fall
 * outside it. Stating one is what makes "over the canvas" and "outside it" mean anything at all.
 *
 * Shared because the two files that need it test the two halves of **one** listener - what a drag in
 * progress draws (`features/preview/Preview.test.tsx`) and what a drop then does
 * (`hooks/useDroppedImages.test.tsx`) - against what is supposed to be one region. Two copies of
 * these numbers can drift apart, and the drag tests would go on passing against a rectangle the drop
 * tests no longer use: the hit test would simply stop being covered, silently.
 */
export const CANVAS = new DOMRect(0, 40, 800, 512);

/** The middle of {@link CANVAS}: where a drag that is over the canvas is. */
export const ON_CANVAS = new PhysicalPosition(400, 300);

/** Above {@link CANVAS}, on the navbar: the miss both files check the hit test against. */
export const ON_NAVBAR = new PhysicalPosition(400, 20);

/**
 * The four-row plan macOS cannot produce.
 *
 * A machine with an RTX card needs all four, and the sizes are the real published ones - they differ
 * by more than an order of magnitude, which is the whole reason the overall figure is weighted. macOS
 * plans one row, so the multi-row dialog, the queued state and the weighting are covered here or
 * nowhere.
 *
 * Typed as the `plan` member rather than as `SetupEvent`, which is what each file's own copy was:
 * a `const` annotated with a union is narrowed by its initializer, but that narrowing is local to
 * the module, so an imported `SetupEvent` would be the whole union and `PLAN.rows` would not exist.
 * It is still assignable anywhere a `SetupEvent` is taken.
 */
export const PLAN: Extract<SetupEvent, { kind: "plan" }> = {
    kind: "plan",
    rows: [
        { name: "ONNX Runtime", size: 184_000_000 },
        { name: "NVIDIA CUDA", size: 612_000_000 },
        { name: "NVIDIA cuDNN", size: 412_000_000 },
        { name: "NVIDIA TensorRT", size: 1_000_000 },
    ],
};

/**
 * The one-row plan every macOS launch produces.
 *
 * Beside [`PLAN`] rather than written out at the two sites that need it, because what those tests are
 * about is the single row - the header that has no "of" to state, and a count pluralised against one -
 * so a second copy drifting into two rows would quietly stop testing the thing it names.
 */
export const SINGLE_PLAN: Extract<SetupEvent, { kind: "plan" }> = {
    kind: "plan",
    rows: [{ name: "ONNX Runtime", size: 184_000_000 }],
};

/** One component's report, as the `initialize` channel delivers it. */
export const progress = (
    name: string,
    state: "installed" | "downloading" | "extracting",
    fraction: number,
): SetupEvent => ({ kind: "progress", name, state, fraction });

/**
 * A component that had nothing to do: on disk, current and complete before this launch started.
 *
 * `Phase::AlreadyInstalled` in the core library, which is the **only** phase that maps to `installed`
 * on the wire - exactly one report, terminal, with neither of the two working phases before it.
 */
export const alreadyInstalled = (name: string) => progress(name, "installed", 1);

/**
 * A component this launch actually installed, as its last report leaves it.
 *
 * **Not `installed`**, which is the whole point of having both of these. `Reporter::finish` lands the
 * bar on exactly 1 and re-emits the phase it was already in, so a component that downloaded and
 * expanded comes to rest on `extracting` at a fraction of 1 and never reports `installed` at all.
 * Fixtures that used `installed` for this were the reason the failure dialog blamed the first
 * component that succeeded rather than the one that stopped.
 */
export const finished = (name: string) => progress(name, "extracting", 1);

/** Drives the store through a sequence of reports, in order. */
export const apply = (...events: SetupEvent[]) => {
    for (const event of events) useSetupStore.getState().apply(event);
};

/** The reason a failed launch carries, on its own where a test asserts on the text. */
export const REASON = "downloading https://example.invalid/cudnn.7z failed: connection reset by peer";

/** What `initialize` rejects with, spelled the way `serde` emits Rust's `SetupError`. */
export const REJECTION: SetupError = { kind: "initialize", failure: "transfer", message: REASON };

/**
 * What `initialize` resolves with on the machine the fixtures describe: a Mac, where CoreML is
 * supported and neither NVIDIA provider is.
 *
 * One fixture rather than an object per test, for the same reason the plan is one: a report copied
 * into several files is several things that can quietly stop agreeing about which providers a
 * machine offers.
 */
export const PROVIDERS: SupportedProviders = { cpu: true, coreml: true, cuda: false, tensorrt: false, webgpu: false };

/**
 * The state column of each row, in order - of whichever of the two dialogs is rendered.
 *
 * One slot for both states, because one row component draws them, which is what makes a comparison
 * across the two a comparison of what they say rather than of two implementations.
 */
export const STATE_SLOTS = "[data-slot='setup-row-state']";

export const states = () => [...document.querySelectorAll(STATE_SLOTS)].map((element) => element.textContent);

/**
 * Back to a launch that has not started, with `failure` absent rather than `undefined`.
 *
 * Through zustand's own `getInitialState` rather than by naming the initial values here: the
 * hand-rolled form had to spell out every field, so a field added to the store would not have been
 * reset by it and nothing would have said so. The replacing form is what makes `failure` genuinely
 * absent - `exactOptionalPropertyTypes` is on, so an optional property means the key is not there,
 * and a merging `set` can only add keys.
 */
export const resetSetupStore = () => useSetupStore.setState(useSetupStore.getInitialState(), true);

/** One variant row, at the two float precisions every model but Osaka publishes. */
const variant = (
    codename: string,
    label: string,
    precisions: Precision[] = ["fp32", "fp16"],
    parameters: VariantEntry["parameters"] = [],
): VariantEntry => ({
    codename,
    label,
    precisions,
    parameters,
});

/**
 * The multiplier every upscale model publishes, with the bounds `Scale::MIN` and `Scale::MAX`
 * enforce.
 *
 * The one published parameter this fixture states, because it is the one a control is built from:
 * the options panel reads its range from the selected model's own entry rather than from a constant
 * of its own, so a fixture that elided it would be testing a control with no bounds at all.
 */
const SCALE: VariantEntry["parameters"] = [{ name: "scale", kind: "range", min: 1, max: 8, default: 1 }];

/**
 * The amount every light-adjustment and colour-balance model publishes, with the bounds `Bias::MIN` and
 * `Bias::MAX` enforce.
 */
const BIAS: VariantEntry["parameters"] = [{ name: "bias", kind: "range", min: -1, max: 1, default: 0.5 }];

/**
 * The amount every denoise and sharpen model publishes, with the bounds `Strength::MIN` and
 * `Strength::MAX` enforce.
 */
const STRENGTH: VariantEntry["parameters"] = [{ name: "strength", kind: "range", min: 0, max: 3, default: 1 }];

/**
 * What Athens publishes: the faces a detection supplies, and the fidelity with the bounds `Fidelity::MIN` and
 * `Fidelity::MAX` enforce, starting at the maximum. Santorini publishes the faces alone.
 */
const FACES_AND_FIDELITY: VariantEntry["parameters"] = [
    { name: "faces", kind: "faces" },
    { name: "fidelity", kind: "range", min: 0, max: 1, default: 1 },
];
const FACES: VariantEntry["parameters"] = [{ name: "faces", kind: "faces" }];

/** The place `opai`'s `Family::APPLY_ORDER` gives each family in a chain; detection has none. */
const ORDER: Partial<Record<Family, number>> = {
    denoise: 0,
    face_recovery: 1,
    colorization: 2,
    light_adjustment: 3,
    color_balance: 4,
    sharpen: 5,
    upscale: 6,
};

const family = (name: Family, variants: VariantEntry[]): FamilyEntry => {
    const order = ORDER[name];

    return { family: name, ...(order !== undefined && { order }), variants };
};

/**
 * The catalogue, as `crates/opai/src/models/catalogue.rs` builds it.
 *
 * Codenames, labels, published precisions and the order of both families and variants are that
 * file's, read off the variant rows it is built from - so a test asserting that upscale defaults to
 * Tokyo is asserting about the order the library actually publishes rather than about a convenient
 * fixture. The parameters are the library's, defaults included, for every family that publishes any - see
 * {@link SCALE}, {@link BIAS}, {@link STRENGTH} and {@link FACES_AND_FIDELITY} - because a new operation
 * starts each of them at its published default.
 *
 * One fixture rather than one per test file, for the reason the plan above is one: a catalogue
 * copied into four files is four things that can quietly stop agreeing about what the library
 * publishes - and the whole point of `opai::catalogue()` is that there is one answer.
 */
export const CATALOGUE: FamilyEntry[] = [
    family("denoise", [
        variant("stockholm", "Stockholm", ["fp32", "fp16"], STRENGTH),
        variant("gothenburg", "Gothenburg", ["fp32", "fp16"], STRENGTH),
        // The accent is the label's and the codename is plain ASCII, as the catalogue publishes them.
        variant("malmo", "Malmö", ["fp32", "fp16"], STRENGTH),
    ]),
    family("sharpen", [
        variant("moscow", "Moscow", ["fp32", "fp16"], STRENGTH),
        variant("petersburg", "Petersburg", ["fp32", "fp16"], STRENGTH),
        variant("novgorod", "Novgorod", ["fp32", "fp16"], STRENGTH),
    ]),
    family("light_adjustment", [
        variant("paris", "Paris", ["fp32", "fp16"], BIAS),
        variant("lyon", "Lyon", ["fp32", "fp16"], BIAS),
    ]),
    family("color_balance", [
        variant("rio", "Rio", ["fp32", "fp16"], BIAS),
        variant("saopaulo", "São Paulo", ["fp32", "fp16"], BIAS),
    ]),
    family("colorization", [variant("delhi", "Delhi"), variant("mumbai", "Mumbai"), variant("jaipur", "Jaipur")]),
    family("upscale", [
        variant("tokyo", "Tokyo", ["fp32", "fp16"], SCALE),
        variant("kyoto", "Kyoto", ["fp32", "fp16"], SCALE),
        variant("saitama", "Saitama", ["fp32", "fp16"], SCALE),
        // The one variant that does not publish the two float precisions: no fp32 build of its
        // diffusion transformer was ever released, and it is now also published quantised to int8.
        variant("osaka", "Osaka", ["fp16", "int8"], SCALE),
    ]),
    // Published, and drawn by nothing: "which model detects faces" is model vocabulary the library
    // declines to leave a front end to restate, but it is not an enhancement a user adds.
    family("detection", [variant("newyork", "New York")]),
    family("face_recovery", [
        variant("athens", "Athens", ["fp32", "fp16"], FACES_AND_FIDELITY),
        variant("santorini", "Santorini", ["fp32", "fp16"], FACES),
    ]),
];

/**
 * What `export_formats` answers, as `crates/gui/src/export/format.rs` builds it: every format, what it is written
 * with, the source extensions a Preserve export writes back as it, and the quality the four lossy ones take.
 */
export const EXPORT_FORMATS: ExportFormats = {
    formats: [
        { format: "avif", extension: "avif", preserves: ["avif"], quality: { min: 1, max: 100, default: 60 } },
        { format: "bmp", extension: "bmp", preserves: ["bmp"], quality: null },
        { format: "gif", extension: "gif", preserves: ["gif"], quality: null },
        { format: "heic", extension: "heic", preserves: ["heic", "heif"], quality: { min: 1, max: 100, default: 60 } },
        { format: "jpeg", extension: "jpg", preserves: ["jpeg", "jpg"], quality: { min: 1, max: 100, default: 90 } },
        { format: "png", extension: "png", preserves: ["png"], quality: null },
        { format: "tiff", extension: "tiff", preserves: ["tif", "tiff"], quality: null },
        { format: "webp", extension: "webp", preserves: ["webp"], quality: { min: 1, max: 100, default: 75 } },
    ],
    fallback: "tiff",
};

/** What each lossy format in {@link EXPORT_FORMATS} starts at, by format - the quality a user never moved. */
export const PUBLISHED_QUALITY = Object.fromEntries(
    EXPORT_FORMATS.formats.flatMap(({ format, quality }) => (quality ? [[format, quality.default]] : [])),
) as Record<"avif" | "heic" | "jpeg" | "webp", number>;

/** The order {@link CATALOGUE} publishes a chain in, as the application reads it off the catalogue. */
export const APPLY_ORDER = applyOrder(CATALOGUE);

/**
 * A photograph as Rust describes one, for the tests that need an image open.
 *
 * Two of them rather than one, because most of what this slice draws is about *which* of the open
 * images is current - and a single record cannot tell a canvas that names the first from one that
 * names the last. The identities are the 16 lowercase hexadecimal characters `renditionUrl` builds a
 * URL from, so a test can assert on the URL a component asked for.
 */
export const HOLIDAY: ImageRecord = {
    path: "/Users/someone/Pictures/holiday.png",
    identity: "0123456789abcdef",
    width: 3000,
    height: 2000,
    extension: "png",
    size: 8_421_504,
};

/** A second photograph, several folders deep, which is what the navbar's name is asserted against. */
export const SUNSET: ImageRecord = {
    path: "/Users/someone/Pictures/2026/Holidays/Iceland/sunset over the harbour.nef",
    identity: "fedcba9876543210",
    width: 6000,
    height: 4000,
    extension: "nef",
    size: 41_205_760,
};

/** Back to a window with nothing open, through zustand's own initial state rather than field by field. */
export const resetFileStore = () => useFileStore.setState(useFileStore.getInitialState(), true);

/**
 * A framing of an open photograph, for the regions that draw or measure one.
 *
 * **Injected, because nothing in this application sets a crop yet** - the Crop/Rotate dialog is the
 * next slice. Every framing the window draws today is one a test put in the store, which is what the
 * whole of this slice is verified against.
 */
export const FRAMING: CropInfo = {
    left: 100,
    top: 50,
    width: 1200,
    height: 1600,
    millidegrees: -1500,
    flipHorizontal: true,
    flipVertical: false,
};

/** Frames `file` the way the dialog will, and forgets every framing on the next reset. */
export const frame = (file: ImageRecord, crop: CropInfo = FRAMING) =>
    act(() => useCropStore.getState().setCrop(file.identity ?? "", crop));

/** Back to a window where nothing is framed. */
export const resetCropStore = () => useCropStore.setState(useCropStore.getInitialState(), true);

/**
 * The measurements the drawer's strip is windowed from, which jsdom answers 0 for.
 *
 * jsdom lays nothing out, and the virtualizer refuses to compute a range at all for a scroll element
 * of zero size - so without this the strip mounts no thumbnails and every assertion about what it
 * draws fails on an empty container rather than on what it is about.
 *
 * Three numbers, and the first is the one that is easy to get wrong: the virtualizer reads the scroll
 * element's box as `offsetWidth` and `offsetHeight`, *not* through `getBoundingClientRect`, which it
 * never calls. `clientHeight` is the third, which `useResizeObserver` subtracts the strip's padding
 * from to get a square item's side.
 *
 * Here rather than in each file that needs it, for the reason the fixtures above are here: two copies
 * of this are two things that can quietly stop agreeing about how wide the strip under test is, and
 * what the windowing does is measured against exactly that.
 *
 * Restored by the `restoreMocks` in vite.config.ts, so nothing has to undo it.
 */
export const stubStripGeometry = ({ width = 640, height = 128 } = {}) => {
    vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(height);
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(width);
    vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(height);
};

/** Opens images the way both routes in do, for a test that is about what the window then draws. */
export const openFiles = (...files: ImageRecord[]) => useFileStore.getState().addFiles(files);

/**
 * A 2D canvas context, which jsdom has none of.
 *
 * jsdom's `getContext` is not merely absent but *loud*: it logs "Not implemented" to the console for
 * every call, so a component that draws leaves that line in the output of every test that mounts it.
 * More to the point, a `null` context means the particle field's frame returns before it draws
 * anything - so a test asserting on what the loop did would be asserting against a loop that did
 * nothing, and would pass whatever the loop was changed to.
 *
 * The members stubbed are exactly the ones `components/ui/particles.tsx` calls. Anything it gains
 * will fail here as an undefined function rather than silently doing nothing, which is the right way
 * round for a fake.
 *
 * Restored by the `restoreMocks` in vite.config.ts, so nothing has to undo it.
 */
export const stubCanvas2D = () => {
    const context = {
        setTransform: vi.fn(),
        clearRect: vi.fn(),
        beginPath: vi.fn(),
        arc: vi.fn(),
        fill: vi.fn(),
        fillStyle: "",
    };

    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(context as unknown as CanvasRenderingContext2D);

    return context;
};

/**
 * The canvas drawing a chosen surface, as a saved preference leaves it.
 *
 * Through the store the dialog publishes into rather than through the settings store, because those
 * are deliberately not the same value - see `stores/canvas.ts`. A test that set the settings store
 * would be setting the *draft*, which the canvas is required not to follow, and would therefore
 * assert nothing about what is drawn.
 */
/**
 * Puts a background in force, as the settings dialog's Save does.
 *
 * `apply` rather than a write to a second store: the canvas reads the settings store directly, and
 * that store only ever holds committed values. A draft lives in the open dialog - see
 * `features/settings/draft.tsx` - which is why a test that wants one mounts the dialog.
 */
export const applyBackground = (background: Background) => useSettingsStore.getState().apply({ background });

/** Back to what a fresh profile draws, so a surface one test applied is not the next one's premise. */
export const resetSettingsStore = () => useSettingsStore.setState(useSettingsStore.getInitialState(), true);

/**
 * A settings draft standing on its own, for a test that mounts one row rather than the whole dialog.
 *
 * This gives the row the same thing the dialog gives it - `useDraftState`'s draft, seeded from what
 * the store holds with `seed` over it - and hands the test back the values so it can assert on what a
 * control wrote. A row mounted without a draft throws, which is the point, and not something a test
 * should work around by reaching into the store.
 */
export const DraftHarness = ({
    seed,
    onDraft,
    children,
}: {
    seed?: Partial<SettingsData>;
    onDraft?: (values: SettingsData) => void;
    children: ReactNode;
}) => {
    const { draft } = useDraftState(seed);

    onDraft?.(draft.values);

    return <SettingsDraftProvider draft={draft}>{children}</SettingsDraftProvider>;
};
