import { call } from "./invoke";

// Written a second time in `crates/gui/src/links.rs`, as a `#[tauri::command]` function's name. A
// rename on one side alone is not a type error but an `invoke` that rejects at runtime: `links.test.ts`
// pins this half, and `generate_handler![]` the Rust one by refusing to compile against a function
// that does not exist.
const OPEN_LINK_COMMAND = "open_link";

/**
 * A page the application links to, as Rust's `Link` deserializes it. Any other string is refused on
 * the Rust side, which is where the address each one opens is decided.
 */
export type Link = "repository" | "website" | "releases";

/**
 * Why a link could not be opened: Rust's `LinksError`, tagged like every other command's.
 * `message` is the reason in full, in English and untranslated.
 */
export type LinksError = {
    kind: "openLink";
    message: string;
};

/**
 * Open `link`'s page in the system's default browser, never in this window.
 *
 * **Names the page, not the address**: this file sends one of three names, and Rust decides what each
 * opens - so nothing the window says can send the user anywhere else.
 */
export const openLink = (link: Link) => call<void>(OPEN_LINK_COMMAND, { link });
