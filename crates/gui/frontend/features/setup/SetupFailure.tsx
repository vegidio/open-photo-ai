import { type ReactNode, useEffect, useState } from "react";
import { type LucideIcon, RotateCcw, TriangleAlert } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { DialogFooter } from "@/components/ui/dialog";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { quit } from "@/ipc/setup";
import { APP_NAME } from "@/lib/constants";
import { report } from "@/lib/report";
import {
    type SetupFailure as Failure,
    installedCount,
    type SetupRow,
    type StoppedShape,
    settledState,
    stoppedRow,
    stoppedShape,
} from "@/stores/setup";
import { ComponentRow, PRESENTATION } from "./ComponentRow";
import { SetupBody, SetupHeader, SetupNote } from "./regions";

/** How long `Copied!` stays up: long enough to read, short enough not to sit over the row. */
const COPIED_FOR_MS = 1_600;

/** What a row says once the attempt has failed: it finished, it stopped, or it was never reached. */
type Outcome = "installed" | "error" | "skipped";

// A table rather than a template-literal key, so a shape added to `StoppedShape` is a type error here
// rather than a `t()` call that renders its own key at runtime.
/** The sentence each shape of attempt adds to the header, keyed by the shape the store derived. */
const SUMMARY: Record<StoppedShape, string> = {
    only: "setup.error.summary.only",
    first: "setup.error.summary.first",
    last: "setup.error.summary.last",
    rest: "setup.error.summary.rest",
};

// Reached through the progress state's table rather than restated here: a component this launch
// finished with is `Installed` and one it never reached is `Queued`, with the same icon, the same tier
// of grey and the same name colour, because they are the same facts about the same component. A row
// that renamed itself or changed its icon when the dialog changed state would read as a state the
// component was never in.
/**
 * How each outcome is drawn, and what it is called.
 *
 * Two of the three are **the progress state's own entries**. Only `error` is this state's own, and it
 * stands where `downloading` stood.
 */
const OUTCOME: Record<Outcome, { icon: LucideIcon; accent: string; name: string; row: string; label: string }> = {
    installed: { ...PRESENTATION.installed, label: "setup.state.installed" },
    skipped: { ...PRESENTATION.queued, label: "setup.state.queued" },
    error: {
        icon: TriangleAlert,
        accent: "text-destructive",
        name: "text-foreground",
        row: "bg-destructive/6",
        label: "setup.error.state.error",
    },
};

/**
 * Screen 02b: the state of the dialog once the attempt has stopped.
 *
 * Everything that is also true of an install in progress - the card, the overall bar, the bordered
 * list, the shape of the aside - belongs to `SetupDialog`. What is here is the failure: which
 * component stopped, the reason it gave, whether the known cause applies, and the two actions.
 */
export const SetupFailure = ({
    failure,
    rows,
    onRetry,
}: {
    failure: Failure;
    rows: SetupRow[];
    onRetry: () => void;
}) => {
    const { t } = useTranslation();

    // Derived rather than stored, and derivable because the core library installs one component at a
    // time - see `stoppedRow`. It can be nothing, which is the common case on a machine where every
    // component was already on disk and only the runtime would not start.
    const stopped = stoppedRow(rows);

    // Which sentence follows the first one, which is a question about the shape of the attempt rather
    // than about this component - see `stoppedShape`. Nothing to add is a real answer: a failure that
    // belongs to no component has no "stopped partway" to describe.
    const shape = stoppedShape(rows);

    return (
        <>
            <SetupHeader
                icon={TriangleAlert}
                tint="border-destructive/28 bg-destructive/12 text-destructive"
                title={t("setup.error.title")}
            >
                {/*
                 * Joined here rather than by JSX whitespace between two expressions, which is a rule
                 * about where the newlines fell rather than about the sentences: the separator has to
                 * survive the formatter reflowing this block, and has to be absent when there is no
                 * second sentence so the first does not end in a stray space.
                 *
                 * The second sentence says how far the attempt got, which the bar below also carries
                 * as a figure. The two are deliberately not the same statement: the bar says how much
                 * of what this machine needs is on disk, and this says what became of the attempt -
                 * that one component stopped part way through, and whether anything behind it was left
                 * untouched. That last part is the one thing nothing else on screen states, and
                 * without it the queued rows under a failure read as work still to come.
                 */}
                {[
                    t("setup.error.needs", { app: APP_NAME }),
                    shape && t(SUMMARY[shape], { finished: installedCount(rows), total: rows.length }),
                ]
                    .filter(Boolean)
                    .join(" ")}
            </SetupHeader>

            <SetupBody rows={rows}>
                {rows.map((row) => (
                    <FailureRow
                        key={row.name}
                        row={row}
                        /*
                         * The drawn state, not the reported one, and through the same helper the
                         * progress dialog's rows go through: a component that downloaded comes to
                         * rest on `extracting`, and only one that was already on disk ever reports
                         * `installed`. Reading the report would draw every component this launch
                         * actually installed as one it never reached.
                         */
                        outcome={
                            row === stopped ? "error" : settledState(row) === "installed" ? "installed" : "skipped"
                        }
                    >
                        {/*
                         * Against the component it belongs to, so which failure is about which
                         * component needs no working out. As a child rather than a prop because a
                         * prop that is sometimes absent is a prop that has to be typed `| undefined`
                         * under `exactOptionalPropertyTypes`, and children may already be nothing.
                         */}
                        {row === stopped && <Reason message={failure.message} />}
                    </FailureRow>
                ))}
            </SetupBody>

            {/*
             * On its own below the list where no component was working - a runtime that would not
             * start, a configuration directory that could not be written - in the treatment the row
             * would have given it. Attributing that failure to whichever component happened to be
             * last would be the alternative.
             */}
            {!stopped && (
                <div className="mx-6 mt-4">
                    <Reason message={failure.message} />
                </div>
            )}

            {/*
             * Only where getting the files was the problem. Advice to wait a few minutes is false
             * about a runtime that will not load or a platform nothing is published for, and a dialog
             * that gives it anyway is one a user learns to stop reading.
             */}
            {failure.kind === "transfer" && <SetupNote>{t("setup.error.throttling")}</SetupNote>}

            <DialogFooter className="mt-5 flex-row items-center gap-3 border-t px-4 py-3">
                <div className="flex-1" />

                <Button variant="secondary" onClick={() => void quit()}>
                    {t("setup.quit")}
                </Button>

                {/*
                 * Withheld where a second attempt cannot succeed - nothing published for this
                 * machine, a name the application cannot use, a runtime that already failed to load
                 * in this process. A button whose every press reproduces the same error in an instant
                 * teaches a user that the application is broken rather than that the situation is.
                 * Which those failures are is what `Failure` decides, so this state has no second
                 * rule about it.
                 */}
                {failure.kind !== "unrecoverable" && (
                    <Button onClick={onRetry}>
                        <RotateCcw strokeWidth={1.75} />
                        {t("setup.error.tryAgain")}
                    </Button>
                )}
            </DialogFooter>
        </>
    );
};

/** One component: what it ended up having done, and - for the one that stopped - why. */
const FailureRow = ({ row, outcome, children }: { row: SetupRow; outcome: Outcome; children?: ReactNode }) => {
    const { t } = useTranslation();

    const { icon, accent, name, row: tint, label } = OUTCOME[outcome];

    return (
        <ComponentRow row={row} icon={icon} accent={accent} nameTier={name} tint={tint} state={t(label)}>
            {children}
        </ComponentRow>
    );
};

/**
 * The reason the initialization gave: one line of it, and the whole of it on a click.
 *
 * **One line**, ellipsised rather than wrapped, and selectable. **The whole of it on a click**, copied
 * to the clipboard and confirmed by a `Copied!` tooltip.
 */
const Reason = ({ message }: { message: string }) => {
    // One line because it shares the row's 14px second-line slot with the progress state's track - see
    // `ComponentRow`. The whole of it on a click is what makes the truncation affordable. A summary is
    // the thing this screen exists to stop doing - the Wails app shows one fixed sentence whatever went
    // wrong - so the sentence a user puts in a bug report has to be the core library's own rendered
    // chain, URL and all, not the part of it that fitted. `select-text` stays for the part that is on
    // screen; the application turns selection off everywhere else, and this is the deliberate exception.
    //
    // The confirmation is a tooltip held open rather than shown on hover: there is nothing to say about
    // this line until it has been copied, and a `Copied!` that appeared on hover would be a claim made
    // before anything happened.
    const { t } = useTranslation();
    const [copied, setCopied] = useState(false);

    useEffect(() => {
        if (!copied) return;

        // Cleared on a timer rather than on mouse-out, because the pointer never has to leave the
        // line for the copy to have happened - and cleaned up on unmount, which is a real case:
        // pressing Try again puts this state away while the tooltip is still up.
        const timer = setTimeout(() => setCopied(false), COPIED_FOR_MS);

        return () => clearTimeout(timer);
    }, [copied]);

    const copy = () => {
        // `navigator.clipboard` needs a secure context, which this webview is - localhost in
        // development, the custom protocol in a release build. Optional rather than assumed so that
        // a context without it degrades to doing nothing instead of throwing inside a click handler.
        void navigator.clipboard?.writeText(message).then(
            () => setCopied(true),
            (error: unknown) => report("Could not copy the failure reason", error),
        );
    };

    return (
        <Tooltip open={copied}>
            <TooltipTrigger asChild>
                <button
                    type="button"
                    onClick={copy}
                    // An explicit box rather than a natural line box, because `truncate` needs
                    // one to ellipsise against - and the natural 12px for 11px text, so nothing
                    // is trimmed off the descenders.
                    className="block h-3 w-full cursor-pointer truncate text-left font-mono text-[11px] leading-3 select-text text-destructive-bright"
                    data-slot="setup-error-reason"
                >
                    {message}
                </button>
            </TooltipTrigger>

            <TooltipContent>{t("setup.error.copied")}</TooltipContent>
        </Tooltip>
    );
};
