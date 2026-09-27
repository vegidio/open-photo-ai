import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { suppressContextMenu } from "./contextMenu";

/** Right-clicks the element and answers whether the webview's menu would still open. */
const menuOpens = (element: Element): boolean =>
    element.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));

let unsuppress: () => void;

beforeEach(() => {
    unsuppress = suppressContextMenu();
});

afterEach(() => {
    unsuppress();
    document.body.replaceChildren();
});

describe("suppressContextMenu", () => {
    it("suppresses the menu on the window's content", () => {
        const image = document.body.appendChild(document.createElement("img"));

        expect(menuOpens(image)).toBe(false);
        expect(menuOpens(document.body)).toBe(false);
    });

    it("keeps the menu in text fields", () => {
        const input = document.body.appendChild(document.createElement("input"));
        const textarea = document.body.appendChild(document.createElement("textarea"));

        expect(menuOpens(input)).toBe(true);
        expect(menuOpens(textarea)).toBe(true);
    });

    it("keeps the menu once removed", () => {
        unsuppress();

        expect(menuOpens(document.body)).toBe(true);
    });
});
