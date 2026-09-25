import { describe, expect, it } from "vitest";
import { type CropInfo, cropQuery, framedDimensions } from "./crop";

/** One framing, which each test below varies a field of. */
const framing = (overrides: Partial<CropInfo> = {}): CropInfo => ({
    left: 10,
    top: 20,
    width: 30,
    height: 40,
    millidegrees: -1500,
    flipHorizontal: false,
    flipVertical: false,
    ...overrides,
});

describe("cropQuery", () => {
    it("spells the six fields in the order Rust's parser reads them", () => {
        // The grammar, pinned against `crop_of` in crates/gui/src/images/serve.rs. Nothing in either
        // toolchain notices when one side is changed alone - a broken URL is a request Rust refuses at
        // runtime.
        expect(cropQuery(framing())).toBe("10,20,30,40,-1500,-");
    });

    it("spells each pair of flips the way the parser names it", () => {
        expect(cropQuery(framing({ flipHorizontal: true }))).toBe("10,20,30,40,-1500,h");
        expect(cropQuery(framing({ flipVertical: true }))).toBe("10,20,30,40,-1500,v");
        expect(cropQuery(framing({ flipHorizontal: true, flipVertical: true }))).toBe("10,20,30,40,-1500,hv");
    });

    it("writes the turn as an integer, whichever way it turns", () => {
        // The reason the unit is thousandths of a degree rather than degrees: an integer has one
        // spelling, and Rust's field is an `i32` that refuses anything with a point in it.
        expect(cropQuery(framing({ millidegrees: 0 }))).toContain(",0,");
        expect(cropQuery(framing({ millidegrees: 90000 }))).toContain(",90000,");
        expect(cropQuery(framing({ millidegrees: -90000 }))).toContain(",-90000,");
    });

    it("produces nothing the parser has to percent-encode", () => {
        // Which is what lets the query go on the URL as written, and lets the cache key on the other
        // side be the text itself rather than whatever an encoder made of it.
        const query = cropQuery(framing({ flipHorizontal: true, flipVertical: true }));

        expect(encodeURIComponent(query)).toBe(query.replaceAll(",", "%2C"));
        expect(query).toMatch(/^\d+,\d+,\d+,\d+,-?\d+,(-|h|v|hv)$/);
    });
});

describe("framedDimensions", () => {
    const holiday = { width: 3000, height: 2000 };

    it("answers the rectangle's dimensions where a framing is set", () => {
        expect(framedDimensions(holiday, framing())).toEqual({ width: 30, height: 40 });
    });

    it("answers the file's own dimensions where none is", () => {
        expect(framedDimensions(holiday, undefined)).toEqual(holiday);
    });

    it("answers nothing measurable for a file the application could not measure", () => {
        // Both properties are absent rather than present and undefined, which under
        // `exactOptionalPropertyTypes` is what an absent `published` means to the pane.
        expect(framedDimensions({}, undefined)).toEqual({});
        expect(framedDimensions(undefined, undefined)).toEqual({});
    });

    it("answers the framing even for a file it could not measure", () => {
        expect(framedDimensions({}, framing())).toEqual({ width: 30, height: 40 });
    });
});
