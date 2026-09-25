import { Component, type ErrorInfo, type ReactNode } from "react";
import { RotateCcw, TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { showLogs } from "@/lib/logs";
import { reportCrash } from "@/lib/report";

type State = {
    /** What was thrown, once something has been. Boxed so that a thrown `undefined` still counts. */
    caught?: { error: unknown };
};

/**
 * Catches a render crash anywhere below it, records it, and shows {@link RecoveryScreen} in place of
 * the blank window it would otherwise leave.
 *
 * A class because React has no hook for `getDerivedStateFromError` or `componentDidCatch`.
 */
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
    override state: State = {};

    static getDerivedStateFromError(error: unknown): State {
        return { caught: { error } };
    }

    override componentDidCatch(error: unknown, info: ErrorInfo) {
        // The component stack is the useful half of a render crash, and it exists only here: it is not on
        // the error. React 19 sends a caught error to `onCaughtError`, not to `window`'s `error` event, so
        // this is its one record.
        reportCrash(error, info.componentStack ?? undefined);
    }

    override render() {
        const { caught } = this.state;
        if (!caught) return this.props.children;

        return <RecoveryScreen error={caught.error} />;
    }
}

/** The error's own text, as a user would copy it into a report. */
const describe = (error: unknown) => (error instanceof Error ? `${error.name}: ${error.message}` : String(error));

// A function component, unlike the Go application's inline fallback, so it can use `useTranslation`.
// No design exists for it: it is drawn in the setup failure dialog's manner (screen 02b), from the same
// tokens and the same `Button`.
/**
 * What the window shows once it could not draw itself: what happened, the error's text, Reload, and
 * Show logs - the crash has just been written there.
 */
const RecoveryScreen = ({ error }: { error: unknown }) => {
    const { t } = useTranslation();

    return (
        // A drag region, because the title bar is overlaid on the content and the navbar that is the
        // window's handle is gone with everything else.
        <div data-tauri-drag-region className="flex h-screen items-center justify-center bg-background p-8">
            <div
                role="alert"
                className="flex w-full max-w-[35rem] flex-col overflow-hidden rounded-xl border bg-card shadow-[0_24px_60px_rgb(0_0_0/0.7)]"
            >
                <div className="flex gap-4 px-6 pt-6 pb-5">
                    <span className="flex size-10 flex-none items-center justify-center rounded-[10px] border border-destructive/28 bg-destructive/12 text-destructive">
                        <TriangleAlert className="size-5" />
                    </span>

                    <div className="flex min-w-0 flex-col gap-1.5">
                        <h1 className="text-base leading-[normal] font-semibold tracking-[-0.01em]">
                            {t("errors.boundary.title")}
                        </h1>
                        <p className="text-[13px]/[1.6] text-pretty text-muted-foreground">
                            {t("errors.boundary.message")}
                        </p>
                    </div>
                </div>

                {/* Selectable, the deliberate exception to the application's rule: it is for copying. */}
                <pre
                    data-slot="recovery-error"
                    className="mx-6 max-h-40 overflow-auto rounded-lg border bg-background p-3 font-mono text-[11px]/[1.6] break-words whitespace-pre-wrap select-text text-destructive-bright"
                >
                    {describe(error)}
                </pre>

                <div className="mt-5 flex items-center gap-3 border-t px-4 py-3">
                    <div className="flex-1" />

                    <Button variant="secondary" onClick={() => void showLogs(t)}>
                        {t("settings.app.logs.button")}
                    </Button>

                    <Button onClick={() => window.location.reload()}>
                        <RotateCcw strokeWidth={1.75} />
                        {t("errors.boundary.reload")}
                    </Button>
                </div>
            </div>
        </div>
    );
};
