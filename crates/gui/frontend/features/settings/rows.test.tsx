import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { render } from "@/test/support";
import {
    SettingsButton,
    SettingsCard,
    SettingsRow,
    SettingsSelect,
    SettingsSelectItem,
    SettingsSwitch,
} from "./rows.tsx";

/**
 * The pieces every settings page is built from.
 *
 * Tested here rather than once per page: what a row has to do - draw its label and description beside
 * its control - and what the controls' fill pins have to survive are the same wherever they are used.
 */
describe("SettingsRow", () => {
    it("draws its label, its description and its control", () => {
        render(
            <SettingsCard>
                <SettingsRow
                    label="Analytics"
                    description="Send anonymous usage data."
                    control={<SettingsSwitch checked aria-label="Analytics" />}
                />
            </SettingsCard>,
        );

        expect(screen.getByText("Analytics")).toBeInTheDocument();
        expect(screen.getByText("Send anonymous usage data.")).toBeInTheDocument();
        expect(screen.getByRole("switch", { name: "Analytics" })).toBeChecked();
    });
});

describe("SettingsSelect", () => {
    const mount = (onChange = vi.fn()) => {
        render(
            <SettingsSelect label="Language" value="en" onChange={onChange}>
                <SettingsSelectItem value="en">English</SettingsSelectItem>
                <SettingsSelectItem value="sv">Svenska</SettingsSelectItem>
            </SettingsSelect>,
        );

        return onChange;
    };

    it("is named by its label and shows its current value", () => {
        mount();

        expect(screen.getByRole("combobox", { name: "Language" })).toHaveTextContent("English");
    });

    it("keeps the design's --background well over the generated dark fill", () => {
        mount();

        // `twMerge` does not treat `bg-background` and the generated `dark:bg-input/30` as conflicting,
        // and the dark variant is forced app-wide - so the pin has to be a `dark:` rule of its own.
        const trigger = screen.getByRole("combobox", { name: "Language" });
        expect(trigger).toHaveClass("dark:bg-background", "dark:hover:bg-background", "w-[180px]");
        expect(trigger).not.toHaveClass("dark:bg-input/30");
    });

    it("reports the option the user chose", () => {
        const onChange = mount();

        // Driven by the keyboard throughout. Radix's select listens for pointer capture, which jsdom
        // does not implement, so a click on an option never reaches it.
        fireEvent.keyDown(screen.getByRole("combobox", { name: "Language" }), { key: "Enter" });
        fireEvent.keyDown(screen.getByRole("option", { name: "Svenska" }), { key: "Enter" });

        expect(onChange).toHaveBeenCalledWith("sv");
    });
});

describe("SettingsButton", () => {
    it("reports the press and keeps its unfilled pin", () => {
        const onPress = vi.fn();
        render(<SettingsButton onPress={onPress}>Show logs</SettingsButton>);

        const button = screen.getByRole("button", { name: "Show logs" });
        fireEvent.click(button);

        expect(onPress).toHaveBeenCalledOnce();
        expect(button).toHaveClass("dark:bg-transparent", "w-[180px]");
        expect(button).not.toHaveClass("dark:bg-input/30");
    });
});

describe("SettingsSwitch", () => {
    it("draws an off track rather than nothing", () => {
        render(<SettingsSwitch checked={false} aria-label="Analytics" />);

        expect(screen.getByRole("switch", { name: "Analytics" })).toHaveClass("data-[state=unchecked]:bg-input");
    });
});
