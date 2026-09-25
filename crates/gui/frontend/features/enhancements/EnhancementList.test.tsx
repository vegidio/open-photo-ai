import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import { type FamilyEntry, forgetCatalogue } from "@/ipc/catalogue";
import type { Operation } from "@/ipc/enhance";
import { track } from "@/lib/faro";
import { useAutopilotStore } from "@/stores/autopilot";
import { useEnhancementStore } from "@/stores/enhancements";
import { useFileStore } from "@/stores/files";
import { APPLY_ORDER, CATALOGUE, HOLIDAY, openFiles, render, resetFileStore, SUNSET } from "@/test/support";
import { EnhancementList } from "./EnhancementList";

// Mocked at `lib/faro.ts`'s own boundary: what `track` does with an event is pinned by `faro.test.ts`.
vi.mock("@/lib/faro", () => ({
    track: vi.fn(),
    sendError: vi.fn(),
    pauseFaro: vi.fn(),
    // Untraced, as before Faro starts: the request goes out exactly as `invoke` alone would send it.
    traced: (_name: string, send: () => Promise<unknown>) => send(),
}));

// What the `catalogue` command answers, held in a binding so a case about a model the real
// catalogue does not publish can substitute one. The default is the library's own catalogue.
let published: FamilyEntry[] = [];

vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(() => Promise.resolve(published)),
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

/** One upscale family entry publishing exactly these precisions for one model. */
const publishing = (codename: string, label: string, precisions: Operation["precision"][]): FamilyEntry => ({
    family: "upscale",
    variants: [{ codename, label, precisions, parameters: [] }],
});

/** An upscale carrying whichever model, precision and scale a case is about. */
const upscale = (codename: string, precision: Operation["precision"], scale: number): Operation => ({
    family: "upscale",
    codename,
    precision,
    parameters: { scale },
});

/** A light adjustment carrying whichever model, precision and bias a case is about. */
const light = (codename: string, precision: Operation["precision"], bias: number): Operation => ({
    family: "light_adjustment",
    codename,
    precision,
    parameters: { bias },
});

/** A colour balance carrying whichever model, precision and bias a case is about. */
const balance = (codename: string, precision: Operation["precision"], bias: number): Operation => ({
    family: "color_balance",
    codename,
    precision,
    parameters: { bias },
});

/** A denoise carrying whichever model, precision and strength a case is about. */
const denoise = (codename: string, precision: Operation["precision"], strength: number): Operation => ({
    family: "denoise",
    codename,
    precision,
    parameters: { strength },
});

/** A sharpen carrying whichever model, precision and strength a case is about. */
const sharpen = (codename: string, precision: Operation["precision"], strength: number): Operation => ({
    family: "sharpen",
    codename,
    precision,
    parameters: { strength },
});

/** A colorization carrying whichever model and precision a case is about. */
const colorization = (codename: string, precision: Operation["precision"]): Operation => ({
    family: "colorization",
    codename,
    precision,
    parameters: {},
});

/** Puts a stack on the current image, which is what this list draws. */
const stack = (...operations: Operation[]) =>
    useEnhancementStore.setState({ enhancements: new Map([[HOLIDAY.path, operations]]) });

/** The line beneath each row's name, in the order the rows are drawn. */
const infoLines = () =>
    [...document.querySelectorAll("[data-slot='enhancement-info']")].map((element) => element.textContent);

const mount = async () => {
    render(<EnhancementList />);

    // The catalogue is an `invoke`, so the model's label and its tier arrive a tick after the mount.
    await waitFor(() => expect(infoLines()[0]).toMatch(/,/));
};

describe("the enhancements on the current image", () => {
    beforeEach(() => {
        vi.mocked(track).mockClear();
        localStorage.clear();
        forgetCatalogue();
        resetFileStore();
        useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
        published = CATALOGUE;
        openFiles(HOLIDAY);
    });

    it("draws nothing at all for an image with no enhancements", () => {
        render(<EnhancementList />);

        expect(document.querySelector("[data-slot='enhancement-list']")).toBeNull();
    });

    it("names the enhancement and reports the model, the scale and the quality beneath it", async () => {
        stack(upscale("kyoto", "fp32", 2));
        await mount();

        expect(screen.getByText("Upscale")).toBeInTheDocument();
        // The model's published label, the scale as a multiplier and the tier its precision sits at -
        // `precisions[0]` being HD, read positionally off what the catalogue publishes.
        expect(infoLines()).toEqual(["Kyoto, 2x, HD"]);
    });

    it("reports a light adjustment's bias as a whole percentage", async () => {
        stack(light("paris", "fp32", 0.5));
        await mount();

        expect(infoLines()).toEqual(["Paris, 50%, HD"]);
    });

    it("reports a negative bias with its sign", async () => {
        // The direction is what a negative bias changes, so dropping the sign would report the opposite
        // adjustment.
        stack(light("lyon", "fp16", -0.4));
        await mount();

        expect(infoLines()).toEqual(["Lyon, -40%, SD"]);
    });

    it("shows a light adjustment between a face recovery and an upscale, whatever order they were added in", async () => {
        const store = useEnhancementStore.getState();
        store.addEnhancement(HOLIDAY.path, upscale("kyoto", "fp32", 2), APPLY_ORDER);
        store.addEnhancement(
            HOLIDAY.path,
            {
                family: "face_recovery",
                codename: "athens",
                precision: "fp32",
                parameters: { fidelity: 1 },
            },
            APPLY_ORDER,
        );
        store.addEnhancement(HOLIDAY.path, light("paris", "fp32", 0.5), APPLY_ORDER);
        await mount();

        const names = [...document.querySelectorAll("[data-slot='enhancement-row'] button span span:first-child")].map(
            (element) => element.textContent,
        );
        expect(names).toEqual(["Face Recovery", "Light Adjustment", "Upscale"]);
    });

    it("reports a colour balance's bias as a whole percentage", async () => {
        stack(balance("rio", "fp32", 0.5));
        await mount();

        expect(infoLines()).toEqual(["Rio, 50%, HD"]);
    });

    it("reports a colour balance's negative bias with its sign", async () => {
        stack(balance("rio", "fp32", -0.4));
        await mount();

        expect(infoLines()).toEqual(["Rio, -40%, HD"]);
    });

    it("shows a colour balance between a light adjustment and an upscale, whatever order they were added in", async () => {
        const store = useEnhancementStore.getState();
        store.addEnhancement(HOLIDAY.path, upscale("kyoto", "fp32", 2), APPLY_ORDER);
        store.addEnhancement(HOLIDAY.path, balance("rio", "fp32", 0.5), APPLY_ORDER);
        store.addEnhancement(HOLIDAY.path, light("paris", "fp32", 0.5), APPLY_ORDER);
        await mount();

        const names = [...document.querySelectorAll("[data-slot='enhancement-row'] button span span:first-child")].map(
            (element) => element.textContent,
        );
        expect(names).toEqual(["Light Adjustment", "Color Balance", "Upscale"]);
    });

    it("reports a denoise's strength as a whole percentage", async () => {
        stack(denoise("stockholm", "fp32", 1));
        await mount();

        expect(infoLines()).toEqual(["Stockholm, 100%, HD"]);
    });

    it("reports a strength past the model's own output", async () => {
        stack(denoise("malmo", "fp32", 2.5));
        await mount();

        expect(infoLines()).toEqual(["Malmö, 250%, HD"]);
    });

    it("shows a denoise above a face recovery and an upscale, whatever order they were added in", async () => {
        const store = useEnhancementStore.getState();
        store.addEnhancement(HOLIDAY.path, upscale("kyoto", "fp32", 2), APPLY_ORDER);
        store.addEnhancement(
            HOLIDAY.path,
            {
                family: "face_recovery",
                codename: "athens",
                precision: "fp32",
                parameters: { fidelity: 1 },
            },
            APPLY_ORDER,
        );
        store.addEnhancement(HOLIDAY.path, denoise("stockholm", "fp32", 1), APPLY_ORDER);
        await mount();

        const names = [...document.querySelectorAll("[data-slot='enhancement-row'] button span span:first-child")].map(
            (element) => element.textContent,
        );
        expect(names).toEqual(["Denoise", "Face Recovery", "Upscale"]);
    });

    it("reports a sharpen's strength as a whole percentage", async () => {
        stack(sharpen("moscow", "fp32", 1));
        await mount();

        expect(infoLines()).toEqual(["Moscow, 100%, HD"]);
    });

    it("reports a sharpen's strength past the model's own output", async () => {
        stack(sharpen("novgorod", "fp32", 2.5));
        await mount();

        expect(infoLines()).toEqual(["Novgorod, 250%, HD"]);
    });

    it("shows a sharpen between a colour balance and an upscale, whatever order they were added in", async () => {
        const store = useEnhancementStore.getState();
        store.addEnhancement(HOLIDAY.path, upscale("kyoto", "fp32", 2), APPLY_ORDER);
        store.addEnhancement(HOLIDAY.path, balance("rio", "fp32", 0.5), APPLY_ORDER);
        store.addEnhancement(HOLIDAY.path, sharpen("moscow", "fp32", 1), APPLY_ORDER);
        await mount();

        const names = [...document.querySelectorAll("[data-slot='enhancement-row'] button span span:first-child")].map(
            (element) => element.textContent,
        );
        expect(names).toEqual(["Color Balance", "Sharpen", "Upscale"]);
    });

    it("reports a colorization's model and quality alone", async () => {
        stack(colorization("delhi", "fp32"));
        await mount();

        expect(infoLines()).toEqual(["Delhi, HD"]);
    });

    it("follows a colorization when its model changes", async () => {
        stack(colorization("delhi", "fp32"));
        await mount();

        useEnhancementStore.getState().replaceEnhancement(HOLIDAY.path, colorization("jaipur", "fp16"));

        await waitFor(() => expect(infoLines()).toEqual(["Jaipur, SD"]));
    });

    it("shows a colorization between a face recovery and a light adjustment, whatever order they were added in", async () => {
        const store = useEnhancementStore.getState();
        store.addEnhancement(HOLIDAY.path, light("paris", "fp32", 0.5), APPLY_ORDER);
        store.addEnhancement(
            HOLIDAY.path,
            {
                family: "face_recovery",
                codename: "athens",
                precision: "fp32",
                parameters: { fidelity: 1 },
            },
            APPLY_ORDER,
        );
        store.addEnhancement(HOLIDAY.path, colorization("delhi", "fp32"), APPLY_ORDER);
        await mount();

        const names = [...document.querySelectorAll("[data-slot='enhancement-row'] button span span:first-child")].map(
            (element) => element.textContent,
        );
        expect(names).toEqual(["Face Recovery", "Colorization", "Light Adjustment"]);
    });

    it("reports a fractional scale without rounding it away", async () => {
        stack(upscale("kyoto", "fp16", 1.5));
        await mount();

        // `parseInt` truncates 1.5x to 1x in the reference; three decimal places also hides the
        // float noise a parsed value arrives with, without touching anything a user can type.
        expect(infoLines()).toEqual(["Kyoto, 1.5x, SD"]);
    });

    it("reports the precision's own spelling where a tier would say nothing", async () => {
        // A third published precision lands on a position the catalogues have no tier name for, and
        // inventing one - or reusing "SD" - would mislabel it. `int8` says exactly what it is.
        published = [publishing("kyoto", "Kyoto", ["fp32", "fp16", "int8"])];
        stack(upscale("kyoto", "int8", 2));
        await mount();

        expect(infoLines()).toEqual(["Kyoto, 2x, int8"]);
    });

    it("reports the operation's own precision for a model published at one", async () => {
        // A tier label says nothing where there is nothing to tell it apart from, so `modelChoice`
        // publishes no quality at all for such a model - and the line still has a slot to fill.
        published = [publishing("kyoto", "Kyoto", ["fp16"])];
        stack(upscale("kyoto", "fp16", 2));
        await mount();

        expect(infoLines()).toEqual(["Kyoto, 2x, fp16"]);
    });

    it("follows the enhancement when its model or scale changes", async () => {
        stack(upscale("kyoto", "fp32", 2));
        await mount();

        useEnhancementStore.getState().replaceEnhancement(HOLIDAY.path, upscale("tokyo", "fp16", 4));

        await waitFor(() => expect(infoLines()).toEqual(["Tokyo, 4x, SD"]));
    });

    it("offers a remove control on every row, reachable by keyboard", async () => {
        stack(upscale("kyoto", "fp32", 2));
        await mount();

        const remove = screen.getByRole("button", { name: "Remove enhancement" });

        // Drawn at zero opacity rather than not drawn: hover is not reachable from a keyboard, and a
        // control that is only mounted on hover cannot be tabbed to at all.
        expect(remove).toBeInTheDocument();
        expect(remove).not.toBeDisabled();

        remove.focus();
        expect(remove).toHaveFocus();
    });

    it("removes the one it is asked for and leaves the rest of the stack in order", async () => {
        const kyoto = upscale("kyoto", "fp32", 2);
        stack(kyoto);
        await mount();

        fireEvent.click(screen.getByRole("button", { name: "Remove enhancement" }));

        await waitFor(() => expect(useEnhancementStore.getState().enhancements.get(HOLIDAY.path)).toEqual([]));
        expect(document.querySelector("[data-slot='enhancement-list']")).toBeNull();
        expect(track).toHaveBeenCalledExactlyOnceWith("enhancement_removed", { family: "upscale" });
    });

    it("reports a model the catalogue no longer publishes by its own codename", async () => {
        stack(upscale("berlin", "fp32", 2));
        render(<EnhancementList />);

        // Honest rather than thrown: the same drift `lib/enhancements.ts` answers for a family the
        // catalogue stops publishing, and it is also what the render before the catalogue arrives
        // looks like.
        await waitFor(() => expect(infoLines()).toEqual(["berlin, 2x, fp32"]));
    });
});

describe("a photograph being analysed", () => {
    const analysingRow = () => screen.queryByRole("status");

    beforeEach(() => {
        localStorage.clear();
        forgetCatalogue();
        resetFileStore();
        useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
        useAutopilotStore.setState(useAutopilotStore.getInitialState(), true);
        published = CATALOGUE;
        openFiles(HOLIDAY, SUNSET);
    });

    it("shows the analysing row in place of the list", () => {
        // A light adjustment added by hand while the analysis runs: hidden until the analysis answers.
        stack(light("paris", "fp32", 0.5));
        useAutopilotStore.getState().begin(HOLIDAY.path, "suggest-1", undefined);

        render(<EnhancementList />);

        expect(analysingRow()).toHaveTextContent("Analysing image...");
        expect(screen.queryByText("Light Adjustment")).not.toBeInTheDocument();
        expect(document.querySelector("[data-slot='enhancement-list']")).toBeNull();
    });

    it("shows it for a photograph with no stack yet", () => {
        useAutopilotStore.getState().begin(HOLIDAY.path, "suggest-1", undefined);

        render(<EnhancementList />);

        expect(analysingRow()).toBeInTheDocument();
    });

    it("shows another photograph's own list when the user moves to one not being analysed", async () => {
        useEnhancementStore.setState({ enhancements: new Map([[SUNSET.path, [upscale("kyoto", "fp32", 2)]]]) });
        useAutopilotStore.getState().begin(HOLIDAY.path, "suggest-1", undefined);
        render(<EnhancementList />);

        act(() => useFileStore.getState().setCurrentIndex(1));

        expect(analysingRow()).toBeNull();
        await waitFor(() => expect(infoLines()).toEqual(["Kyoto, 2x, HD"]));

        // And back again, while the analysis is still under way.
        act(() => useFileStore.getState().setCurrentIndex(0));
        expect(analysingRow()).toBeInTheDocument();
    });

    it("gives way to the list once the analysis answers", () => {
        useAutopilotStore.getState().begin(HOLIDAY.path, "suggest-1", undefined);
        render(<EnhancementList />);

        act(() => {
            useEnhancementStore.getState().addEnhancements(HOLIDAY.path, [light("paris", "fp32", 0.5)], APPLY_ORDER);
            useAutopilotStore.getState().end(HOLIDAY.path, "suggest-1");
        });

        expect(analysingRow()).toBeNull();
        expect(screen.getByText("Light Adjustment")).toBeInTheDocument();
    });
});
