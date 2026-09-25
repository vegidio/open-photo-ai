import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import { forgetCatalogue } from "@/ipc/catalogue";
import type { Operation } from "@/ipc/enhance";
import { useEnhancementStore } from "@/stores/enhancements";
import { CATALOGUE, HOLIDAY, openFiles, render, resetFileStore } from "@/test/support";
import { EnhancementList } from "./EnhancementList";
import { ScaleControl } from "./ScaleControl";

vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(() => Promise.resolve(CATALOGUE)),
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

const upscale = (codename: string, precision: Operation["precision"], scale: number): Operation => ({
    family: "upscale",
    codename,
    precision,
    parameters: { scale },
});

/** What the current image is set to have done to it. */
const stack = () => useEnhancementStore.getState().enhancements.get(HOLIDAY.path);

/** Puts one upscale on the current image and draws the sidebar's list of it. */
const mount = async (operation = upscale("kyoto", "fp32", 2)) => {
    useEnhancementStore.setState({ enhancements: new Map([[HOLIDAY.path, [operation]]]) });
    render(<EnhancementList />);

    // The row names the model off the catalogue, which is an `invoke`: waiting for it is waiting
    // for the tray to have something to offer.
    await screen.findByText(/Kyoto|Tokyo|Osaka|Saitama/);
};

/** The panel itself, which is queried by its slot rather than by a role Radix may or may not set. */
const panel = () => document.querySelector("[data-slot='enhancement-options']");

/** Opens the row's options. A popover's trigger opens on the click, unlike a menu's. */
const open = async () => {
    fireEvent.click(screen.getByRole("button", { name: /Upscale/ }));

    await waitFor(() => expect(panel()).not.toBeNull());
};

const scaleField = () => screen.getByRole("textbox", { name: "Scale" });

/** The tray's own options, which is what separates them from the scale group's four. */
const models = () => [
    ...(document.querySelector("[data-slot='model-tray']")?.querySelectorAll('[role="radio"]') ?? []),
];

beforeEach(() => {
    localStorage.clear();
    forgetCatalogue();
    resetFileStore();
    useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
    openFiles(HOLIDAY);
});

describe("the options an enhancement's row opens", () => {
    it("opens from the row, titled with the enhancement's name", async () => {
        await mount();

        await open();

        expect(panel()).toBeInTheDocument();
        expect(panel()).toHaveTextContent("AI Model");
    });

    it("changes nothing by being opened and dismissed", async () => {
        await mount();
        const before = stack();

        await open();
        fireEvent.click(screen.getByRole("button", { name: "Close" }));

        // Opening a panel to look at what an enhancement is set to must not write the stack:
        // writing it is what cancels the run in flight and asks for another.
        await waitFor(() => expect(panel()).toBeNull());
        expect(stack()).toBe(before);
    });
});

describe("the model tray", () => {
    it("offers every model at every precision the catalogue publishes", async () => {
        await mount();
        await open();

        // Four upscale models: three at two float precisions, Osaka at fp16 and int8 - which is
        // what reading the tiers positionally gives, rather than a table of per-model overrides.
        expect(models()).toHaveLength(8);
        expect(screen.getByRole("radio", { name: "Kyoto HD" })).toBeInTheDocument();
        expect(screen.getByRole("radio", { name: "Osaka SD" })).toBeInTheDocument();
    });

    it("shows the model in use as chosen", async () => {
        await mount();
        await open();

        // `aria-checked` rather than `data-state`, which is what the selected surface is painted
        // off and the one a `TooltipTrigger asChild` around an item cannot overwrite.
        expect(screen.getByRole("radio", { name: "Kyoto HD" })).toBeChecked();
        expect(screen.getByRole("radio", { name: "Tokyo HD" })).not.toBeChecked();
    });

    it("writes the chosen model and its precision to the stack", async () => {
        await mount();
        await open();

        fireEvent.click(screen.getByRole("radio", { name: "Osaka SD" }));

        // Osaka's SD build is int8, not fp16: the tier is a statement about quality rather than
        // about a number of bits, and the two disagree for that model.
        await waitFor(() => expect(stack()).toEqual([upscale("osaka", "int8", 2)]));
    });

    it("carries each model's description once, on its first precision", async () => {
        await mount();
        await open();

        // One marker per model, not per option: the explanation is about the model, and repeating
        // it would put two of them on every row of the tray.
        const marked = models().filter((item) => item.querySelector("svg"));

        expect(marked).toHaveLength(4);
        expect(marked.map((item) => item.textContent)).toEqual(["Tokyo HD", "Kyoto HD", "Saitama HD", "Osaka HD"]);
    });

    /**
     * `pointerMove` rather than `pointerOver` or `pointerEnter`: a Radix tooltip opens off the
     * move, and it opens on a timer even at this provider's zero delay - hence the wait.
     */
    it("explains a model when the pointer is on its marker, not anywhere on the option", async () => {
        await mount();
        await open();

        const tokyo = models().find((item) => item.textContent === "Tokyo HD") as HTMLElement;

        // Crossing the pill is what a user does on the way to another model, and it must stay
        // silent: the panel would otherwise cover the tray the whole way down it.
        fireEvent.pointerMove(tokyo);
        await waitFor(() => expect(screen.queryByRole("tooltip")).not.toBeInTheDocument());

        fireEvent.pointerMove(tokyo.querySelector("[data-slot='tooltip-trigger']") as HTMLElement);
        await waitFor(() => expect(screen.getByRole("tooltip")).toHaveTextContent("without exaggeration"));
    });
});

describe("the scale control", () => {
    it("reports the scale the enhancement carries", async () => {
        await mount(upscale("kyoto", "fp32", 1.5));
        await open();

        expect(scaleField()).toHaveValue("1.5");
        expect(screen.getByRole("radio", { name: "Custom" })).toBeChecked();
    });

    it("takes a value inside the published range", async () => {
        await mount();
        await open();

        fireEvent.change(scaleField(), { target: { value: "3" } });

        await waitFor(() => expect(stack()).toEqual([upscale("kyoto", "fp32", 3)]));
    });

    it("brings a value above the range down rather than refusing it", async () => {
        await mount();
        await open();

        fireEvent.change(scaleField(), { target: { value: "12" } });

        // The catalogue publishes 1..8 for every upscale model, and a user who typed 12 meant "as
        // much as you can" rather than "reject this".
        await waitFor(() => expect(stack()).toEqual([upscale("kyoto", "fp32", 8)]));
        expect(scaleField()).toHaveValue("8");
    });

    it("holds a trailing decimal separator while it is being typed", async () => {
        await mount();
        await open();

        fireEvent.change(scaleField(), { target: { value: "1." } });

        // Without this the field rewrites itself to `1` between the two keystrokes and 1.5 is
        // unreachable. Nothing is written until a digit arrives.
        expect(scaleField()).toHaveValue("1.");
        expect(stack()).toEqual([upscale("kyoto", "fp32", 2)]);

        fireEvent.change(scaleField(), { target: { value: "1.5" } });

        await waitFor(() => expect(stack()).toEqual([upscale("kyoto", "fp32", 1.5)]));
    });

    it("reaches the published maximum through Max", async () => {
        await mount();
        await open();

        fireEvent.click(screen.getByRole("button", { name: "Max" }));

        await waitFor(() => expect(stack()).toEqual([upscale("kyoto", "fp32", 8)]));
    });

    it("offers the common multipliers as shortcuts", async () => {
        await mount();
        await open();

        expect(screen.getByRole("radio", { name: "2x" })).toBeChecked();

        fireEvent.click(screen.getByRole("radio", { name: "4x" }));

        await waitFor(() => expect(stack()).toEqual([upscale("kyoto", "fp32", 4)]));
    });

    it("shows Custom as chosen for a value no shortcut names, and changes nothing when pressed", async () => {
        await mount(upscale("kyoto", "fp32", 3));
        await open();

        const custom = screen.getByRole("radio", { name: "Custom" });
        expect(custom).toBeChecked();

        fireEvent.click(custom);

        // Custom names no value of its own - it reports that the typed one matches no shortcut.
        expect(stack()).toEqual([upscale("kyoto", "fp32", 3)]);
    });

    it("holds a value below the range while it is being typed, and settles it on leaving", async () => {
        await mount();
        await open();

        fireEvent.change(scaleField(), { target: { value: "0.5" } });

        // Held rather than raised, for the reason the trailing separator is: a value below the
        // minimum is a prefix of every longer value that reaches it.
        expect(scaleField()).toHaveValue("0.5");
        expect(stack()).toEqual([upscale("kyoto", "fp32", 2)]);

        fireEvent.blur(scaleField());

        // Walking away from one is not part-way through typing it, so it lands inside the range.
        await waitFor(() => expect(stack()).toEqual([upscale("kyoto", "fp32", 1)]));
        expect(scaleField()).toHaveValue("1");
    });

    it("writes nothing when the field is left without having moved", async () => {
        await mount();
        await open();

        const before = stack();

        fireEvent.blur(scaleField());

        // The same array, not merely an equal one: writing the stack is what asks for a new run, and
        // a field the user only tabbed through must not cost one.
        expect(stack()).toBe(before);
    });

    it("normalizes a value typed with trailing zeros to the one the enhancement carries", async () => {
        await mount();
        await open();

        fireEvent.change(scaleField(), { target: { value: "2.0" } });
        fireEvent.blur(scaleField());

        expect(scaleField()).toHaveValue("2");
        expect(stack()).toEqual([upscale("kyoto", "fp32", 2)]);
    });
});

/**
 * The control on its own, for the one rule the catalogue cannot exercise through the panel.
 *
 * Every upscale model the library publishes carries the same 1..8 range - `Scale::MIN` and
 * `Scale::MAX` - so the fixture does too, and a minimum above 1 is unreachable from it. The control
 * reads its bounds per model precisely so a model publishing a different range needs no edit here;
 * this is what stops multi-digit entry being collateral of one arriving.
 */
describe("the scale control's own bounds", () => {
    it("holds a prefix below a minimum above one, rather than rewriting the field", () => {
        const onChange = vi.fn();

        render(<ScaleControl value={4} min={2} max={8} onChange={onChange} />);

        const field = screen.getByRole("textbox", { name: "Scale" });
        fireEvent.change(field, { target: { value: "1" } });

        // The `1` of a `12` the user is reaching for. Raised on the keystroke it would become `2`
        // and `12` would be untypeable - the separator failure, one digit earlier.
        expect(field).toHaveValue("1");
        expect(onChange).not.toHaveBeenCalled();

        fireEvent.change(field, { target: { value: "12" } });

        // And 12 is above the maximum, which is not a prefix of anything: brought down as it lands.
        expect(onChange).toHaveBeenCalledWith(8);
    });
});
