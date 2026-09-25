import { beforeEach, describe, expect, it } from "vitest";
import type { ImageRecord } from "@/ipc/images";
import { useTransformStore } from "@/stores/transform";
import { useFileStore } from "./files";

/** A record as Rust describes one, named by its file so a list of them reads as a list of photographs. */
const image = (name: string): ImageRecord => ({
    path: `/Users/someone/Pictures/${name}.png`,
    identity: name.padEnd(16, "0"),
    width: 3000,
    height: 2000,
    extension: "png",
    size: 8_421_504,
});

const holiday = image("holiday");
const sunset = image("sunset");
const harbour = image("harbour");

const state = () => useFileStore.getState();
const paths = () => state().files.map((file) => file.path);

describe("useFileStore", () => {
    beforeEach(() => {
        useFileStore.setState(useFileStore.getInitialState(), true);
        useTransformStore.setState(useTransformStore.getInitialState(), true);
    });

    it("opens nothing before anyone has asked", () => {
        expect(state().files).toEqual([]);
    });

    it("makes the first of the first batch the current image", () => {
        state().addFiles([holiday, sunset]);

        expect(paths()).toEqual([holiday.path, sunset.path]);
        expect(state().currentIndex).toBe(0);
    });

    it("moves the current image to the first of a later batch", () => {
        state().addFiles([holiday, sunset]);
        state().addFiles([harbour]);

        // The window shows what the user just asked for rather than what it was already showing, and
        // the images already open stay open behind it.
        expect(paths()).toEqual([holiday.path, sunset.path, harbour.path]);
        expect(state().currentIndex).toBe(2);
    });

    it("leaves the list and the current image alone when a batch is entirely already open", () => {
        state().addFiles([holiday, sunset]);
        state().addFiles([sunset]);
        state().addFiles([holiday, sunset]);

        // The subtle half of the rule: re-opening the same folder must not throw the canvas back to
        // its first photograph, which is what moving the index unconditionally would do.
        expect(paths()).toEqual([holiday.path, sunset.path]);
        expect(state().currentIndex).toBe(0);
    });

    it("keeps one of a file that arrives twice in one batch", () => {
        // A drop can carry the same file twice, and a scan against the existing list alone would miss
        // it.
        state().addFiles([holiday, holiday, sunset]);

        expect(paths()).toEqual([holiday.path, sunset.path]);
    });

    it("makes the first genuinely new file current when a batch is partly already open", () => {
        state().addFiles([holiday, sunset]);
        state().addFiles([holiday, harbour]);

        expect(paths()).toEqual([holiday.path, sunset.path, harbour.path]);
        expect(state().currentIndex).toBe(2);
    });
});

describe("the selection", () => {
    beforeEach(() => {
        useFileStore.setState(useFileStore.getInitialState(), true);
        useTransformStore.setState(useTransformStore.getInitialState(), true);
    });

    const selected = () => [...state().selectedPaths];

    it("picks the first of a batch opened into an empty window", () => {
        state().addFiles([holiday, sunset]);

        // A window with one photograph in it is ready to act on it, which is what the reference does
        // and the only moment anything but the user writes the selection.
        expect(selected()).toEqual([holiday.path]);
    });

    it("leaves the selection alone when a later batch arrives", () => {
        state().addFiles([holiday]);
        state().addFiles([sunset, harbour]);

        // What is picked at that point is a choice the user has made, and files turning up is not a
        // reason to revise it.
        expect(selected()).toEqual([holiday.path]);
    });

    it("leaves the selection alone when a batch adds nothing", () => {
        state().addFiles([holiday, sunset]);
        state().toggleSelected(sunset.path);
        state().addFiles([holiday, sunset]);

        expect(selected().sort()).toEqual([holiday.path, sunset.path].sort());
    });

    it("picks and unpicks one file, touching no other", () => {
        state().addFiles([holiday, sunset, harbour]);

        state().toggleSelected(sunset.path);
        expect(selected().sort()).toEqual([holiday.path, sunset.path].sort());

        state().toggleSelected(sunset.path);
        expect(selected()).toEqual([holiday.path]);
    });

    it("does not move the current image when a file is picked", () => {
        state().addFiles([holiday, sunset, harbour]);
        state().setCurrentIndex(2);

        state().toggleSelected(holiday.path);
        state().toggleSelected(sunset.path);

        // Picking is what a batch operation will act on; being current is what the canvas draws. The
        // whole reason they are separate is that the user reads one photograph while choosing several.
        expect(state().currentIndex).toBe(2);
    });

    it("picks every open image and unpicks every one of them", () => {
        state().addFiles([holiday, sunset, harbour]);

        state().selectAll();
        expect(selected().sort()).toEqual([holiday.path, sunset.path, harbour.path].sort());

        state().unselectAll();
        expect(selected()).toEqual([]);
    });

    /**
     * The standing rule `setTransform` in `stores/transform.ts` argues: a mutating writer is invisible
     * to every assertion above - each of them would pass on one.
     */
    it.each([
        ["toggleSelected", () => state().toggleSelected(sunset.path)],
        ["selectAll", () => state().selectAll()],
        ["unselectAll", () => state().unselectAll()],
    ])("replaces the Set rather than mutating it, in %s", (_name, write) => {
        state().addFiles([holiday, sunset, harbour]);

        const before = state().selectedPaths;
        write();

        expect(state().selectedPaths).not.toBe(before);
    });
});

describe("the current image", () => {
    beforeEach(() => {
        useFileStore.setState(useFileStore.getInitialState(), true);
        useTransformStore.setState(useTransformStore.getInitialState(), true);
    });

    it("is whichever one the user chose", () => {
        state().addFiles([holiday, sunset, harbour]);

        state().setCurrentIndex(1);

        expect(state().currentIndex).toBe(1);
    });

    it("changes nothing else about what the window holds", () => {
        state().addFiles([holiday, sunset, harbour]);
        state().selectAll();

        const files = state().files;
        const selectedPaths = state().selectedPaths;

        state().setCurrentIndex(1);

        // Not `toEqual`: the same images in the same order with the same ones picked is the same
        // objects, and a writer that rebuilt either would re-render every thumbnail in the strip.
        expect(state().files).toBe(files);
        expect(state().selectedPaths).toBe(selectedPaths);
    });
});

describe("the current file", () => {
    beforeEach(() => {
        useFileStore.setState(useFileStore.getInitialState(), true);
        useTransformStore.setState(useTransformStore.getInitialState(), true);
    });

    /**
     * Read through the selector rather than through `renderHook`: `useCurrentFile` is that selector
     * applied to the store, and calling it as a hook would be testing zustand's subscription rather
     * than which photograph it names.
     */
    const current = () => useFileStore.getState().files.at(useFileStore.getState().currentIndex);

    it("is nothing while nothing is open", () => {
        expect(current()).toBeUndefined();
    });

    it("is the file the index names", () => {
        state().addFiles([holiday, sunset]);
        expect(current()).toEqual(holiday);

        state().addFiles([harbour]);
        expect(current()).toEqual(harbour);
    });
});

describe("closing an image", () => {
    beforeEach(() => {
        useFileStore.setState(useFileStore.getInitialState(), true);
        useTransformStore.setState(useTransformStore.getInitialState(), true);
    });

    /** How closely one photograph is being looked at, so a close has something to forget. */
    const magnify = (file: ImageRecord) =>
        useTransformStore.getState().setTransform(file.identity ?? "", { scale: 4, x: -10, y: -20 });

    const transforms = () => useTransformStore.getState().transforms;

    it("removes the image from the window", () => {
        state().addFiles([holiday, sunset, harbour]);

        state().closeFile(sunset.path);

        expect(paths()).toEqual([holiday.path, harbour.path]);
    });

    it("leaves the current image alone when the one closed was above it", () => {
        // The first arm that is not the fixup: rows after the current one shift, the current one does
        // not, so nothing has to be corrected.
        state().addFiles([holiday, sunset, harbour]);
        state().setCurrentIndex(0);

        state().closeFile(harbour.path);

        expect(state().currentIndex).toBe(0);
        expect(state().files[state().currentIndex]?.path).toBe(holiday.path);
    });

    it("shifts the current image down when the one closed was below it", () => {
        // The rows below it moved down by one, so the index has to follow them to keep naming the same
        // photograph.
        state().addFiles([holiday, sunset, harbour]);
        state().setCurrentIndex(2);

        state().closeFile(holiday.path);

        expect(state().currentIndex).toBe(1);
        expect(state().files[state().currentIndex]?.path).toBe(harbour.path);
    });

    it("moves forward when the current image is closed and another follows it", () => {
        // The middle rule, and the non-obvious one: the index is unchanged and the photograph that
        // followed has slid into the slot. Closing repeatedly walks through what is open rather than
        // snapping back to the first.
        state().addFiles([holiday, sunset, harbour]);
        state().setCurrentIndex(1);

        state().closeFile(sunset.path);

        expect(state().currentIndex).toBe(1);
        expect(state().files[state().currentIndex]?.path).toBe(harbour.path);
    });

    it("moves back when the current image was the last in the order", () => {
        // There is nothing after it to move forward to, so the index follows the list's new end.
        state().addFiles([holiday, sunset, harbour]);
        state().setCurrentIndex(2);

        state().closeFile(harbour.path);

        expect(state().currentIndex).toBe(1);
        expect(state().files[state().currentIndex]?.path).toBe(sunset.path);
    });

    it("leaves no current image when the only open one is closed", () => {
        state().addFiles([holiday]);

        state().closeFile(holiday.path);

        expect(paths()).toEqual([]);
        expect(state().currentIndex).toBe(0);
        // 0 addresses nothing while the list is empty, which is what makes it the honest answer here.
        expect(state().files.at(state().currentIndex)).toBeUndefined();
    });

    it("takes the closed image out of the selection and leaves the rest picked", () => {
        state().addFiles([holiday, sunset, harbour]);
        state().selectAll();

        state().closeFile(sunset.path);

        expect(state().selectedPaths.has(sunset.path)).toBe(false);
        expect(state().selectedPaths.has(holiday.path)).toBe(true);
        expect(state().selectedPaths.has(harbour.path)).toBe(true);
    });

    it("forgets how the closed image was being looked at", () => {
        // What makes "a closed image's view is forgotten" true: the entry goes, so a file re-opened at
        // the same identity is found fitted rather than still magnified.
        state().addFiles([holiday, sunset]);
        magnify(holiday);
        magnify(sunset);

        state().closeFile(holiday.path);

        expect(transforms().has(holiday.identity ?? "")).toBe(false);
        expect(transforms().has(sunset.identity ?? "")).toBe(true);
    });

    it("closes a file whose bytes could not be read", () => {
        // No identity, so nothing to remove from the transform map - and still a file the user opened
        // and must be able to close.
        const unreadable: ImageRecord = { path: "/Users/someone/Pictures/broken.png", extension: "png" };
        state().addFiles([unreadable, holiday]);

        state().closeFile(unreadable.path);

        expect(paths()).toEqual([holiday.path]);
    });

    it("does nothing for a path that is not open", () => {
        state().addFiles([holiday, sunset]);
        state().setCurrentIndex(1);

        state().closeFile("/Users/someone/Pictures/never-opened.png");

        // Not a fixup applied to a row that was never there, which is what an unguarded index of -1
        // would have produced.
        expect(paths()).toEqual([holiday.path, sunset.path]);
        expect(state().currentIndex).toBe(1);
    });
});

describe("closing every image", () => {
    beforeEach(() => {
        useFileStore.setState(useFileStore.getInitialState(), true);
        useTransformStore.setState(useTransformStore.getInitialState(), true);
    });

    it("empties the window, the selection and the views", () => {
        state().addFiles([holiday, sunset, harbour]);
        state().selectAll();
        useTransformStore.getState().setTransform(holiday.identity ?? "", { scale: 4, x: 0, y: 0 });
        state().setCurrentIndex(2);

        state().closeAll();

        expect(paths()).toEqual([]);
        expect(state().selectedPaths.size).toBe(0);
        expect(state().currentIndex).toBe(0);
        expect(useTransformStore.getState().transforms.size).toBe(0);
    });

    it("empties a window holding one image", () => {
        state().addFiles([holiday]);

        state().closeAll();

        expect(paths()).toEqual([]);
    });

    it("is harmless with nothing open", () => {
        state().closeAll();

        expect(paths()).toEqual([]);
        expect(state().currentIndex).toBe(0);
    });
});
