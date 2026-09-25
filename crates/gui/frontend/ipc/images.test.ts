import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, type Mock, vi } from "vitest";
import { type CropInfo, cropQuery } from "./crop";
import {
    describeImages,
    forgetInputExtensions,
    type ImageRecord,
    type ImagesError,
    inputExtensions,
    openImages,
    renditionFor,
    renditionUrl,
    revealImage,
} from "./images";

// Mocked at the `invoke` boundary, so the names asserted below are the ones that would actually go on
// the wire. This is the frontend half of a contract whose Rust half is `#[tauri::command] fn
// open_images` and `fn describe_images` in crates/gui/src/images/files.rs; renaming either side alone is
// what this exists to catch.
//
// `convertFileSrc` is mocked too, and only to observe what it is ASKED. What it answers is a platform
// detail this file must not restate: the whole reason it is called is that the URL differs between
// macOS, Linux and Windows.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), convertFileSrc: vi.fn(() => "opai://localhost/IDENTITY") }));

const invoked = invoke as unknown as Mock;
const converted = convertFileSrc as unknown as Mock;

/** A record as Rust serializes one, with everything it could read. */
const holiday: ImageRecord = {
    path: "/Users/someone/Pictures/holiday.png",
    identity: "0123456789abcdef",
    width: 3000,
    height: 2000,
    extension: "png",
    size: 8_421_504,
};

describe("openImages", () => {
    it("calls the command Rust registers, by name, with the wording it was given", () => {
        // The two strings are the frontend's because the frontend owns the catalogue. The extension
        // list is deliberately NOT here: it is derived in Rust from what the decoder actually opens,
        // so a format the library gains is offered without this file being touched.
        openImages("Select Image", "Images");

        expect(invoked).toHaveBeenCalledWith("open_images", { title: "Select Image", filterName: "Images" });
    });

    it("resolves with the records Rust described, untouched", async () => {
        invoked.mockResolvedValueOnce([holiday]);

        await expect(openImages("Select Image", "Images")).resolves.toEqual([holiday]);
    });

    it("treats an empty selection as no files rather than as a failure", async () => {
        // Choosing nothing is a choice, and a picker the platform would not open reports the same
        // empty answer - `blocking_pick_files` hands back an `Option`, so Rust cannot tell the two
        // apart and does not pretend to. Either way it resolves rather than rejecting, and nothing
        // here turns that back into an error.
        invoked.mockResolvedValueOnce([]);

        await expect(openImages("Select Image", "Images")).resolves.toEqual([]);
    });

    it("rejects with the whole of what Rust serialized, untouched", async () => {
        // The only failure this command has: the files were chosen and the application closed before
        // they could be described. Not a picker failure, which is the empty selection above.
        const rejection: ImagesError = {
            kind: "openImages",
            message: "the files could not be described: runtime shutting down",
        };
        invoked.mockRejectedValueOnce(rejection);

        await expect(openImages("Select Image", "Images")).rejects.toEqual(rejection);
    });

    it("does not call the dialog plugin directly", () => {
        // `@tauri-apps/plugin-dialog` is not a dependency of this application and no capability grants
        // the plugin's commands. The picker is opened from Rust, which is where the filter list and
        // the describe-and-admit that follows it both live.
        openImages("Select Image", "Images");

        expect(invoked).toHaveBeenCalledTimes(1);
        expect(invoked).toHaveBeenCalledWith("open_images", expect.anything());
    });
});

describe("describeImages", () => {
    it("calls the command Rust registers, by name, with the paths it was given", () => {
        describeImages(["/Users/someone/Pictures/a.png", "/Users/someone/Pictures/b.nef"]);

        expect(invoked).toHaveBeenCalledWith("describe_images", {
            paths: ["/Users/someone/Pictures/a.png", "/Users/someone/Pictures/b.nef"],
        });
    });

    it("resolves with records of the same shape the picker produces", async () => {
        // The requirement stated from this side: a file dragged onto the window is the same photograph
        // as a file chosen from a dialog, so there is one type and one set of callers.
        invoked.mockResolvedValueOnce([holiday]);

        await expect(describeImages([holiday.path])).resolves.toEqual([holiday]);
    });

    it("rejects with its own kind rather than the picker's", async () => {
        const rejection: ImagesError = { kind: "describeImages", message: "the files could not be described" };
        invoked.mockRejectedValueOnce(rejection);

        await expect(describeImages([holiday.path])).rejects.toEqual(rejection);
    });
});

describe("inputExtensions", () => {
    // The module holds the fetch for the life of the window, which is the behaviour under test and
    // would otherwise leak the first test's answer into the rest of the file.
    beforeEach(forgetInputExtensions);

    it("calls the command Rust registers, by name, and takes no arguments", () => {
        invoked.mockResolvedValueOnce([]);

        inputExtensions();

        // No arguments at all: the list is the decoder's own, so there is nothing to ask it about.
        expect(invoked).toHaveBeenCalledWith("input_extensions");
    });

    it("resolves with the extensions Rust reported, untouched", async () => {
        invoked.mockResolvedValueOnce(["jpg", "png", "nef"]);

        await expect(inputExtensions()).resolves.toEqual(["jpg", "png", "nef"]);
    });

    it("asks once however many times it is read", async () => {
        invoked.mockResolvedValueOnce(["jpg"]);

        const first = await inputExtensions();
        const second = await inputExtensions();

        // The value is compiled into the binary, so a second call would be the same bytes over the
        // wire for an answer that cannot have changed.
        expect(invoked).toHaveBeenCalledTimes(1);
        expect(second).toBe(first);
    });

    it("asks again after a call that failed", async () => {
        invoked.mockRejectedValueOnce(new Error("the IPC bridge is not up"));
        await expect(inputExtensions()).rejects.toThrow("the IPC bridge is not up");

        invoked.mockResolvedValueOnce(["jpg"]);
        await expect(inputExtensions()).resolves.toEqual(["jpg"]);

        // A kept rejection would make one failed call permanent for the life of the window, and the
        // cost of that is every later drop refusing every file the user dropped.
        expect(invoked).toHaveBeenCalledTimes(2);
    });
});

/** One framing, as the Crop/Rotate dialog will hand it over. Injected: nothing sets one yet. */
const framing: CropInfo = {
    left: 10,
    top: 20,
    width: 30,
    height: 40,
    millidegrees: -1500,
    flipHorizontal: true,
    flipVertical: false,
};

describe("renditionUrl", () => {
    it("asks convertFileSrc for the identity under this application's scheme", () => {
        // The whole point of going through it: `opai://localhost/...` on macOS and Linux and
        // `http://opai.localhost/...` on Windows. Writing either form out here would be a build that
        // works on the machine it was developed on.
        renditionUrl("0123456789abcdef");

        expect(converted).toHaveBeenCalledWith("0123456789abcdef", "opai");
    });

    it("appends the bound as the size the pixels are capped at", () => {
        expect(renditionUrl("0123456789abcdef", 100)).toBe("opai://localhost/IDENTITY?size=100");
    });

    it("asks for no size at all when no bound is wanted", () => {
        // An absent `size` is the image's own dimensions, so `?size=0` would say the same thing in more
        // characters - and, being a different URL, would cost the webview a second copy of the same
        // pixels in its cache.
        expect(renditionUrl("0123456789abcdef")).toBe("opai://localhost/IDENTITY");
        expect(renditionUrl("0123456789abcdef", 0)).toBe("opai://localhost/IDENTITY");
    });

    it("never invokes a command", () => {
        // It is a URL, not a call. The pixels arrive because the webview loads it, which is the whole
        // difference from the reference implementation's base64-over-IPC transport.
        renditionUrl("0123456789abcdef", 128);

        expect(invoked).not.toHaveBeenCalled();
    });

    it("asks for no framing at all when none is wanted", () => {
        // The property the whole slice rests on: until the dialog ships there is nothing to set a
        // crop, so every URL this window builds is byte-for-byte the one it built before - and every
        // rendition Rust has already produced under that key is still the answer to it.
        expect(renditionUrl("0123456789abcdef")).toBe("opai://localhost/IDENTITY");
        expect(renditionUrl("0123456789abcdef", 100)).toBe("opai://localhost/IDENTITY?size=100");
        expect(renditionUrl("0123456789abcdef", 100, undefined)).toBe("opai://localhost/IDENTITY?size=100");
    });

    it("appends the framing as the crop the pixels are cut to", () => {
        expect(renditionUrl("0123456789abcdef", 0, framing)).toBe("opai://localhost/IDENTITY?crop=10,20,30,40,-1500,h");
        expect(renditionUrl("0123456789abcdef", 100, framing)).toBe(
            "opai://localhost/IDENTITY?size=100&crop=10,20,30,40,-1500,h",
        );
    });

    it("spells the framing through cropQuery rather than by hand", () => {
        // One grammar, in one place, pinned against Rust's parser by `crop.test.ts`. A second
        // spelling here would be the drift this application spends its shape avoiding.
        expect(renditionUrl("0123456789abcdef", 0, framing)).toContain(`crop=${cropQuery(framing)}`);
    });
});

describe("renditionFor", () => {
    it("carries the framing through to the URL", () => {
        expect(renditionFor({ identity: "0123456789abcdef" }, 100, framing)).toBe(
            renditionUrl("0123456789abcdef", 100, framing),
        );
    });

    it("answers nothing for a record nothing can serve, framing or not", () => {
        // A file whose bytes could not be read was never admitted, so there is no rendition to frame.
        expect(renditionFor({}, 100, framing)).toBeUndefined();
        expect(renditionFor(undefined, 100, framing)).toBeUndefined();
    });
});

describe("revealImage", () => {
    it("calls the command Rust registers, by name, with the path it was given", () => {
        // The frontend half of the contract whose Rust half is `#[tauri::command] fn reveal_image`,
        // pinned the way `ipc/logs.test.ts` pins `reveal_log`. The argument name matters as much as
        // the command name: Tauri matches it to the parameter, so `filePath` here would reject at
        // runtime with nothing failing to compile.
        revealImage(holiday.path);

        expect(invoked).toHaveBeenCalledWith("reveal_image", { path: holiday.path });
    });

    it("sends the path rather than the identity", () => {
        // By path, unlike everything else about a photograph. An identity is absent exactly when the
        // file's bytes could not be read, and such a file is still an open image - the one a user is
        // most likely to want to look at on disk.
        revealImage(holiday.path);

        expect(invoked).not.toHaveBeenCalledWith(
            "reveal_image",
            expect.objectContaining({ identity: expect.anything() }),
        );
    });

    it("does not call the opener plugin directly", () => {
        // The plugin still does the revealing, from Rust. Its own command is `async`, so Tauri runs it
        // on a runtime worker - where macOS aborts the process, Linux panics and never settles the
        // promise, and Windows COM wants a main-thread apartment.
        revealImage(holiday.path);

        expect(invoked).toHaveBeenCalledTimes(1);
    });

    it("propagates a refusal rather than swallowing it", async () => {
        // A path the application never opened is refused by Rust, and the window has to hear about it:
        // a menu item that quietly does nothing reads as a broken one.
        const rejection: ImagesError = {
            kind: "revealImage",
            message: "the image is not one this application has opened",
        };
        invoked.mockRejectedValueOnce(rejection);

        await expect(revealImage("/etc/passwd")).rejects.toEqual(rejection);
    });

    it("propagates a file manager that would not open", async () => {
        // The other half of the one variant: the file was opened and has since been moved or deleted,
        // which the plugin reports because it canonicalizes the path before it reveals it.
        const rejection: ImagesError = {
            kind: "revealImage",
            message: "the image could not be shown in the file manager: no such file or directory",
        };
        invoked.mockRejectedValueOnce(rejection);

        await expect(revealImage(holiday.path)).rejects.toEqual(rejection);
    });
});
