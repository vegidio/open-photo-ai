import { describe, expect, expectTypeOf, it } from "vitest";
import { type EventName, type EventParams, type FormatList, formatList, formatsOf } from "./events";

// Ported with `formatList` from the Go application's `analytics/buckets.test.ts`.
describe("formatList", () => {
    it("normalises, de-duplicates and sorts", () => {
        expect(formatList([".JPG", "jpg", ".png"])).toBe("jpg,png");
    });

    // Order must not matter: the same mix of files dragged in a different order has to be one value, not two.
    it("is order-independent", () => {
        expect(formatList([".cr2", ".jpg"])).toBe(formatList([".jpg", ".cr2"]));
    });

    it("drops empty extensions rather than emitting a stray separator", () => {
        expect(formatList(["", ".jpg", "."])).toBe("jpg");
        expect(formatList([])).toBe("");
    });

    // Only ever extensions - a file name here would be user content on a telemetry payload.
    it("keeps a dotted name to its final extension only", () => {
        expect(formatList([".tar.gz"])).toBe("tar.gz");
    });
});

describe("formatsOf", () => {
    it("says the types of the files and nothing of their names", () => {
        expect(formatsOf(["/photos/Holiday.JPG", "C:\\scans\\scan.tiff", "IMG_2.jpg"])).toBe("jpg,tiff");
    });

    it("has nothing to say of a file with no extension, or of a dotfile", () => {
        expect(formatsOf(["/photos/README", "/photos/.hidden", "/a.b/raw"])).toBe("");
    });
});

/** `true` where `T` is a bare `string`, or a union that admits one. `FormatList` is branded, so it is not. */
type AdmitsAnyString<T> = string extends T ? true : false;

/** A parameter's value, or its elements' for a list. `never` stays `never`, which `infer` would widen to `unknown`. */
type Element<T> = [T] extends [never] ? never : T extends readonly (infer I)[] ? I : T;

/** The parameters, across every event, whose type admits any string. */
type Leaks = {
    [E in EventName]: {
        [P in keyof EventParams[E]]: [Element<EventParams[E][P]>] extends [never]
            ? never
            : AdmitsAnyString<Element<EventParams[E][P]>> extends true
              ? `${E}.${P & string}`
              : never;
    }[keyof EventParams[E]];
}[EventName];

describe("the catalogue", () => {
    // Checked by `tsc`, which runs over the tests: an event that could carry a name, a path or an error's
    // text fails the build, not a dashboard review.
    it("types no parameter as a bare string", () => {
        expectTypeOf<Leaks>().toEqualTypeOf<never>();
        expectTypeOf<AdmitsAnyString<FormatList>>().toEqualTypeOf<false>();
        expectTypeOf<AdmitsAnyString<string>>().toEqualTypeOf<true>();
    });

    it("names the thirteen events the design keeps", () => {
        expectTypeOf<EventName>().toEqualTypeOf<
            | "app_ready"
            | "files_added"
            | "files_refused"
            | "enhancement_added"
            | "enhancement_removed"
            | "autopilot_run"
            | "preview_mode_changed"
            | "crop_applied"
            | "export_started"
            | "export_finished"
            | "settings_saved"
            | "tensorrt_prompt_answered"
            | "update_opened"
        >();
    });

    it("never names the Analytics choice among the changed settings", () => {
        expectTypeOf<"analytics">().not.toExtend<EventParams["settings_saved"]["changed"][number]>();
    });
});
