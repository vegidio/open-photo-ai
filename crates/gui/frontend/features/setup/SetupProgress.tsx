import { Download } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { DialogFooter } from "@/components/ui/dialog";
import { Progress } from "@/components/ui/progress";
import { quit } from "@/ipc/setup";
import { APP_NAME } from "@/lib/constants";
import { isWorking, liveState, type SetupRow } from "@/stores/setup";
import { ComponentRow, PRESENTATION } from "./ComponentRow";
import { SetupBody, SetupHeader, SetupNote } from "./regions";

/**
 * Screen 02: the state of the dialog while the application is installing what this machine needs.
 *
 * Everything that is also true of a failure - the card, the overall bar, the bordered list, the shape
 * of the aside - belongs to `SetupDialog`; what is here is what only an install in progress says.
 */
export const SetupProgress = ({ rows }: { rows: SetupRow[] }) => {
    const { t } = useTranslation();

    return (
        <>
            <SetupHeader
                icon={Download}
                tint="border-primary/28 bg-primary/12 text-primary"
                title={t("setup.title", { app: APP_NAME })}
            >
                {/*
                 * Two sentences from two keys rather than one: the clause about what is being
                 * downloaded and the clause about it happening once have to be free to reorder in
                 * translation.
                 */}
                {t("setup.components")} {t("setup.cached")}
            </SetupHeader>

            <SetupBody rows={rows}>
                {rows.map((row) => (
                    <ProgressRow key={row.name} rows={rows} row={row} />
                ))}
            </SetupBody>

            <SetupNote>{t("setup.throttling")}</SetupNote>

            <DialogFooter className="mt-5 flex-row items-center gap-3 border-t px-4 py-3">
                <span className="flex-1 text-xs leading-[normal] text-foreground-dim">{t("setup.footer")}</span>

                <Button variant="secondary" onClick={() => void quit()}>
                    {t("setup.quit")}
                </Button>
            </DialogFooter>
        </>
    );
};

/**
 * One component: what it is doing, and its own track while it works.
 *
 * Takes the whole list as well as its own row, because what this one says depends on whether anything
 * after it has started - see `liveState`.
 */
const ProgressRow = ({ rows, row }: { rows: SetupRow[]; row: SetupRow }) => {
    const { t } = useTranslation();

    /*
     * The live state, not the reported one. A component comes to rest in the phase it finished in, so
     * reading the report would leave a completed row saying `Extracting` with a full track for the
     * rest of the launch, with the next component transferring beneath it - two rows working at once,
     * which the library never does.
     *
     * `liveState` rather than `settledState` because the hand-off is one moment on screen: this row
     * becomes `Installed` in the same report that makes the next one `Downloading`, rather than going
     * quiet first and leaving the list with nothing happening in it. The failure dialog reads settled
     * instead - see `liveState` for why a stopped attempt must not be read this way.
     */
    const state = liveState(rows, row);

    const { icon, accent, name, row: tint } = PRESENTATION[state];

    return (
        <ComponentRow
            row={row}
            icon={icon}
            accent={accent}
            nameTier={name}
            tint={tint}
            state={t(`setup.state.${state}`)}
        >
            {/*
             * Only while there is work in flight. A queued row with a track at zero would say a
             * transfer had started, and an installed row with a full one would say it had just
             * finished - neither of which is what those states mean.
             */}
            {isWorking(state) && <Progress value={row.fraction * 100} className="h-1 bg-secondary" />}
        </ComponentRow>
    );
};
