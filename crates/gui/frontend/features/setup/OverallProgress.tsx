import { useTranslation } from "react-i18next";
import { Progress } from "@/components/ui/progress";
import { installedCount, overallFraction, type SetupRow } from "@/stores/setup";

/**
 * How far through the whole install this launch is: the count, the percentage and the bar. Drawn in
 * both of the dialog's states - see `SetupBody`.
 */
export const OverallProgress = ({ rows }: { rows: SetupRow[] }) => {
    const { t } = useTranslation();

    const percent = Math.round(overallFraction(rows) * 100);

    return (
        <div className="mx-6 mb-4 flex flex-col gap-2">
            <div className="flex items-baseline justify-between">
                <span className="text-xs leading-[normal] text-muted-foreground">
                    {/*
                     * `count` is the denominator, because "components" pluralises on how many the
                     * machine needs rather than on how many are done - and the denominator is every
                     * component, including the ones that were already on disk, so the sentence does
                     * not change between two launches.
                     */}
                    {t("setup.installed", { count: rows.length, installed: installedCount(rows) })}
                </span>

                <span className="font-mono text-xs leading-[normal]">{percent}%</span>
            </div>

            <Progress value={percent} className="h-1.5 bg-secondary" />
        </div>
    );
};
