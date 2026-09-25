import { act, fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import i18n from "@/i18n";
import { revealLog } from "@/ipc/logs";
import { report, reportCrash } from "@/lib/report";
import { render } from "@/test/support";
import { ErrorBoundary } from "./ErrorBoundary";

vi.mock("@/ipc/logs", () => ({ revealLog: vi.fn() }));
vi.mock("@/lib/report", () => ({ report: vi.fn(), reportCrash: vi.fn() }));

const Throws = (): never => {
    throw new TypeError("x is undefined");
};

const crashed = () =>
    render(
        <ErrorBoundary>
            <Throws />
        </ErrorBoundary>,
    );

beforeEach(() => {
    // React writes a caught render error to the console itself.
    vi.spyOn(console, "error").mockImplementation(() => {});
});

describe("ErrorBoundary", () => {
    it("draws its children while nothing has thrown", () => {
        render(
            <ErrorBoundary>
                <p>the application</p>
            </ErrorBoundary>,
        );

        expect(screen.getByText("the application")).toBeInTheDocument();
        expect(reportCrash).not.toHaveBeenCalled();
    });

    it("shows the recovery screen, with the error's text, in place of a child that threw", () => {
        crashed();

        expect(screen.getByRole("alert")).toBeInTheDocument();
        expect(screen.getByText(i18n.t("errors.boundary.title"))).toBeInTheDocument();
        expect(screen.getByText(i18n.t("errors.boundary.message"))).toBeInTheDocument();
        expect(screen.getByText("TypeError: x is undefined")).toBeInTheDocument();
    });

    it("records the crash once, with the component stack", () => {
        crashed();

        expect(reportCrash).toHaveBeenCalledOnce();
        const [error, componentStack] = vi.mocked(reportCrash).mock.calls[0] ?? [];
        expect(error).toBeInstanceOf(TypeError);
        expect(componentStack).toMatch(/Throws/);
    });

    it("reloads the window on Reload", () => {
        const reload = vi.fn();
        vi.stubGlobal("location", { ...window.location, reload });
        crashed();

        fireEvent.click(screen.getByRole("button", { name: i18n.t("errors.boundary.reload") }));

        expect(reload).toHaveBeenCalledOnce();
    });

    it("shows the log on Show logs", async () => {
        vi.mocked(revealLog).mockResolvedValue(undefined);
        crashed();

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: i18n.t("settings.app.logs.button") }));
        });

        expect(revealLog).toHaveBeenCalledOnce();
        expect(report).not.toHaveBeenCalled();
    });

    it("reports and tells the user when the log could not be shown", async () => {
        const failure = { kind: "revealLog", message: "no file manager" };
        vi.mocked(revealLog).mockRejectedValue(failure);
        crashed();

        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: i18n.t("settings.app.logs.button") }));
        });

        expect(report).toHaveBeenCalledWith("showing the log file failed", failure);
        expect(await screen.findByText(i18n.t("errors.showLogsFailed"))).toBeInTheDocument();
    });

    it("is in the language the application is running in", async () => {
        await act(async () => {
            await i18n.changeLanguage("sv");
        });
        crashed();

        expect(screen.getByRole("button", { name: "Ladda om" })).toBeInTheDocument();

        await act(async () => {
            await i18n.changeLanguage("en");
        });
    });
});
