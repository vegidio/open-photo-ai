import { invoke } from "@tauri-apps/api/core";
import { describe, expect, it, type Mock, vi } from "vitest";
import { type LinksError, openLink } from "./links";

// Mocked at the `invoke` boundary, so the name asserted below is the one that would actually go on
// the wire. This is the frontend half of a contract whose Rust half is `#[tauri::command] fn open_link`
// in crates/gui/src/links.rs.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const invoked = invoke as unknown as Mock;

describe("openLink", () => {
    // The three strings Rust's `the_window_names_a_link_by_the_strings_the_frontend_sends` pins from the
    // other side.
    it.each(["repository", "website", "releases"] as const)(
        "sends %s by name to the command Rust registers",
        (link) => {
            openLink(link);

            expect(invoked).toHaveBeenCalledWith("open_link", { link });
        },
    );

    it("propagates a failure rather than swallowing it", async () => {
        const rejection: LinksError = {
            kind: "openLink",
            message: "https://vinicius.io could not be opened in the browser: no application is registered for https",
        };
        invoked.mockRejectedValueOnce(rejection);

        await expect(openLink("website")).rejects.toEqual(rejection);
    });
});
