import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import "@/i18n";
import { forgetCatalogue } from "@/ipc/catalogue";
import type { Operation } from "@/ipc/enhance";
import type { Face } from "@/ipc/faces";
import { faceKey } from "@/lib/faces";
import { useCropStore } from "@/stores/crop";
import { useEnhancementStore } from "@/stores/enhancements";
import { useFacesStore } from "@/stores/faces";
import { CATALOGUE, FRAMING, HOLIDAY, openFiles, render, resetFileStore } from "@/test/support";
import { EnhancementList } from "./EnhancementList";

// The picker's detection is an `invoke` too, answered by hand: see `answerDetection`.
let answerDetection: (faces: Face[]) => void = () => {};

vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn((command: string) =>
        command === "detect_faces"
            ? new Promise((resolve) => {
                  answerDetection = resolve;
              })
            : Promise.resolve(CATALOGUE),
    ),
    convertFileSrc: vi.fn((identity: string) => `opai://localhost/${identity}`),
}));

const recovery = (codename: string, precision: Operation["precision"]): Operation => ({
    family: "face_recovery",
    codename,
    precision,
    parameters: {},
});

const face = (left: number): Face => ({
    bounding_box: { min: { x: left, y: 4 }, max: { x: left + 3, y: 7 } },
    landmarks: [
        { x: left + 1, y: 5 },
        { x: left + 2, y: 5 },
        { x: left + 1.5, y: 6 },
        { x: left + 1, y: 6.5 },
        { x: left + 2, y: 6.5 },
    ],
    confidence: 0.9,
    restorable: true,
    key: `${left},4,${left + 3},7`,
});

const identity = HOLIDAY.identity ?? "";

/** What the current image is set to have done to it. */
const stack = () => useEnhancementStore.getState().enhancements.get(HOLIDAY.path);

/** Puts one face recovery on the current image and draws the sidebar's list of it. */
const mount = async (operation = recovery("athens", "fp32")) => {
    useEnhancementStore.setState({ enhancements: new Map([[HOLIDAY.path, [operation]]]) });
    render(<EnhancementList />);

    await screen.findByText("Face Recovery");
};

const panel = () => document.querySelector("[data-slot='enhancement-options']");

/** Opens the row's options. A popover's trigger opens on the click, unlike a menu's. */
const open = async () => {
    fireEvent.click(screen.getByRole("button", { name: /Face Recovery/ }));

    await waitFor(() => expect(panel()).not.toBeNull());
};

/** The tray's own options. */
const models = () => [
    ...(document.querySelector("[data-slot='model-tray']")?.querySelectorAll('[role="radio"]') ?? []),
];

/** The sentence beneath the enhancement's name. */
const info = () => document.querySelector("[data-slot='enhancement-info']");

/** Records the faces found in the current photograph at the framing in force. */
const found = (faces: Face[], crop?: typeof FRAMING | undefined) =>
    useFacesStore.getState().setFaces(identity, crop, faces);

/** Records which of them the user has skipped, as applying a choice in the dialog would. */
const skip = (...faces: Face[]) =>
    act(() => useFacesStore.getState().setFaceChoice(identity, { skipped: faces.map(faceKey), restored: [] }));

/** The way into the chooser, drawn in the options panel. */
const selectFaces = () => screen.getByRole("button", { name: /Select faces/ });

const chooser = () => screen.queryByRole("dialog", { name: "Select faces" });

beforeEach(() => {
    vi.mocked(invoke).mockClear();
    localStorage.clear();
    forgetCatalogue();
    resetFileStore();
    useEnhancementStore.setState({ autopilot: true, enhancements: new Map() });
    useFacesStore.setState(useFacesStore.getInitialState(), true);
    useCropStore.setState(useCropStore.getInitialState(), true);
    openFiles(HOLIDAY);
});

describe("what a face recovery can be set to", () => {
    it("offers both models at both tiers, and nothing else", async () => {
        await mount();
        await open();

        // Two models at two float precisions each. The fidelity is not among them: it is fixed at
        // maximum and offered nowhere, so a control for it would have a single legal position.
        expect(models()).toHaveLength(4);
        expect(screen.getByRole("radio", { name: "Athens HD" })).toBeInTheDocument();
        expect(screen.getByRole("radio", { name: "Athens SD" })).toBeInTheDocument();
        expect(screen.getByRole("radio", { name: "Santorini HD" })).toBeInTheDocument();
        expect(screen.getByRole("radio", { name: "Santorini SD" })).toBeInTheDocument();
    });

    it("shows the model in use as chosen", async () => {
        await mount();
        await open();

        expect(screen.getByRole("radio", { name: "Athens HD" })).toBeChecked();
        expect(screen.getByRole("radio", { name: "Santorini HD" })).not.toBeChecked();
    });

    it("replaces the operation on the stack when another model is chosen", async () => {
        await mount();
        await open();

        fireEvent.click(screen.getByRole("radio", { name: "Santorini SD" }));

        await waitFor(() => expect(stack()).toEqual([recovery("santorini", "fp16")]));
    });

    it("leaves the enhancement's faces alone when the model changes", async () => {
        // The faces belong to the pixels rather than to the model, and the stack's copy carries none
        // in any case: `useEnhancementRun` puts them in on the way to the run.
        await mount();
        found([face(0), face(20)]);
        await open();

        fireEvent.click(screen.getByRole("radio", { name: "Santorini HD" }));

        await waitFor(() => expect(stack()).toEqual([recovery("santorini", "fp32")]));
    });

    it("changes nothing by being opened and dismissed", async () => {
        await mount();
        const before = stack();

        await open();
        fireEvent.click(screen.getByRole("button", { name: "Close" }));

        await waitFor(() => expect(panel()).toBeNull());
        expect(stack()).toBe(before);
    });
});

describe("what a face recovery's row reports", () => {
    it("names the model, how many faces it will restore, and the quality", async () => {
        found([face(0), face(20)]);
        await mount();

        await waitFor(() => expect(info()).toHaveTextContent("Athens, 2 Faces, HD"));
    });

    it("counts one face in the singular", async () => {
        found([face(0)]);
        await mount();

        await waitFor(() => expect(info()).toHaveTextContent("Athens, 1 Face, HD"));
    });

    it("follows the framing, so a face framed out of the picture stops being counted", async () => {
        found([face(0), face(20)]);
        await mount();
        await waitFor(() => expect(info()).toHaveTextContent("2 Faces"));

        // The cut is applied and the photograph re-detected at the new framing, which is what the run
        // path does: one face is outside the rectangle now, so the row reports one.
        useCropStore.getState().setCrop(identity, FRAMING);
        found([face(0)], FRAMING);

        await waitFor(() => expect(info()).toHaveTextContent("Athens, 1 Face, HD"));
    });

    it("reports no count while the faces are not known yet", async () => {
        await mount();

        // Between adding the enhancement and the detection landing there is no count to report, and
        // `0 Faces` would look like an answer. The model and the quality are what is actually known.
        await waitFor(() => expect(info()).toHaveTextContent("Athens, HD"));
        expect(info()).not.toHaveTextContent("Face");
    });

    it("distinguishes a photograph where none were found from one not yet detected", async () => {
        found([]);
        await mount();

        await waitFor(() => expect(info()).toHaveTextContent("Athens, 0 Faces, HD"));
    });

    it("reports no count for a photograph whose bytes could not be read", async () => {
        // No identity means no pixels to serve and none to detect in, so there is nothing to count.
        resetFileStore();
        openFiles({ path: HOLIDAY.path, extension: "png" });

        await mount();

        await waitFor(() => expect(info()).toHaveTextContent("Athens, HD"));
    });
});

describe("the way into the Select faces chooser", () => {
    it("draws the Faces block beneath the models", async () => {
        found([face(0), face(20)]);
        await mount();
        await open();

        expect(screen.getByText("Faces")).toBeInTheDocument();
        expect(selectFaces()).toBeInTheDocument();
    });

    it("draws the help as the two lines the catalogue breaks it into", async () => {
        // The break is part of the centred layout, so it sits in the catalogue where a translator can
        // move it - which is why it comes through `<Trans>` rather than as one flat string.
        found([face(0)]);
        await mount();
        await open();

        const help = screen.getByText(/You can select individual/);

        expect(help).toHaveTextContent("You can select individual");
        expect(help).toHaveTextContent("faces that you want to enhance.");
        expect(help.querySelector("br")).not.toBeNull();
    });

    it("reports how many faces are chosen", async () => {
        found([face(0), face(20)]);
        await mount();
        await open();

        expect(selectFaces()).toHaveTextContent("Select faces (2)");
    });

    it("counts down as faces are skipped", async () => {
        found([face(0), face(20)]);
        skip(face(0));
        await mount();
        await open();

        expect(selectFaces()).toHaveTextContent("Select faces (1)");
    });

    it("is unavailable for a photograph in which no faces were found", async () => {
        // Nothing to choose among, so the control says so rather than opening on an empty picture.
        found([]);
        await mount();
        await open();

        expect(selectFaces()).toBeDisabled();
    });

    it("is unavailable while the faces are not known yet", async () => {
        await mount();
        await open();

        expect(selectFaces()).toBeDisabled();
    });

    it("asks for the faces when it is opened over a photograph whose faces are not known, and offers them", async () => {
        // The picker's own detection, so Select faces does not wait for the whole chain to answer.
        await mount();
        await open();

        expect(invoke).toHaveBeenCalledWith("detect_faces", expect.objectContaining({ source: identity }));

        act(() => answerDetection([face(0), face(20)]));

        await waitFor(() => expect(selectFaces()).toBeEnabled());
        expect(useFacesStore.getState().faces.get(identity)).toEqual({ faces: [face(0), face(20)] });
    });

    it("asks for nothing when the faces at the framing in force are already known", async () => {
        found([face(0)]);
        await mount();
        await open();

        expect(invoke).not.toHaveBeenCalledWith("detect_faces", expect.anything());
    });

    it("is available for a photograph with faces in it", async () => {
        found([face(0)]);
        await mount();
        await open();

        expect(selectFaces()).toBeEnabled();
    });

    it("opens the chooser and takes the panel down with it, without writing anything", async () => {
        // The dialog is a sibling of the popover rather than a child, so the panel closing does not
        // unmount it mid-open - which is what mounting it inside would do.
        found([face(0), face(20)]);
        await mount();
        await open();

        const before = stack();
        fireEvent.click(selectFaces());

        await waitFor(() => expect(chooser()).not.toBeNull());
        await waitFor(() => expect(panel()).toBeNull());

        expect(chooser()).not.toBeNull();
        expect(stack()).toBe(before);
        expect(useFacesStore.getState().choices.size).toBe(0);
    });
});

describe("what a face recovery's row reports about the choice", () => {
    it("reports every face found while none are skipped", async () => {
        found([face(0), face(20)]);
        await mount();

        await waitFor(() => expect(info()).toHaveTextContent("Athens, 2 Faces, HD"));
    });

    it("reports the ratio while some are skipped", async () => {
        // A row cannot say `2 Faces` while the run restores one.
        found([face(0), face(20)]);
        skip(face(0));
        await mount();

        await waitFor(() => expect(info()).toHaveTextContent("Athens, 1/2 Faces, HD"));
    });

    it("reports none of two while both are skipped", async () => {
        found([face(0), face(20)]);
        skip(face(0), face(20));
        await mount();

        await waitFor(() => expect(info()).toHaveTextContent("Athens, 0/2 Faces, HD"));
    });

    it("pluralises on the total rather than on the chosen count", async () => {
        // `0/2 Faces` rather than `0/2 Face`, which is the reference's choice and what makes the
        // English read correctly; a language with more plural forms adds keys to its own catalogue.
        found([face(0)]);
        skip(face(0));
        await mount();

        await waitFor(() => expect(info()).toHaveTextContent("Athens, 0/1 Face, HD"));
    });

    it("still reports the model and the quality alone while the faces are not known", async () => {
        // Only a known set is counted, and only a known set can be chosen from: a choice recorded at
        // some other framing must not make this read a ratio.
        skip(face(0));
        await mount();

        await waitFor(() => expect(info()).toHaveTextContent("Athens, HD"));
        expect(info()).not.toHaveTextContent("Face");
    });

    it("ignores a skipped face that is not among the ones found", async () => {
        // What a framing change comes to: every face is at new coordinates, so nothing matches and
        // every face in the new framing is chosen.
        found([face(0), face(20)]);
        skip(face(80));
        await mount();

        await waitFor(() => expect(info()).toHaveTextContent("Athens, 2 Faces, HD"));
    });

    it("keeps the choice when the face recovery is removed and another is added", async () => {
        // The choice is about the pixels, not about the operation: it is kept in the faces store
        // keyed by identity, so replacing the enhancement is not an event it hears about. Keeping it
        // beside the stack instead would make this an effect somebody has to remember to write.
        found([face(0), face(20)]);
        skip(face(0));
        await mount();

        await waitFor(() => expect(info()).toHaveTextContent("Athens, 1/2 Faces, HD"));

        act(() => {
            useEnhancementStore.setState({ enhancements: new Map([[HOLIDAY.path, []]]) });
        });
        act(() => {
            useEnhancementStore.setState({
                enhancements: new Map([[HOLIDAY.path, [recovery("santorini", "fp16")]]]),
            });
        });

        await waitFor(() => expect(info()).toHaveTextContent("Santorini, 1/2 Faces, SD"));
    });
});
