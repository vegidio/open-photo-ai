import { Fragment } from "react";
import { useTranslation } from "react-i18next";
import { familyEntry, useCatalogue } from "@/hooks/useCatalogue";
import type { FamilyEntry } from "@/ipc/catalogue";
import { ENHANCEMENTS, type Enhancement, familiesWhere, modelChoice, modelLabel } from "@/lib/enhancements";
import { cn } from "@/lib/utils";
import { useSettingsDraft } from "./draft";
import { SettingsCard, SettingsSelect, SettingsSelectItem, SettingsSelectSeparator, SettingsSwitch } from "./rows.tsx";

/** The table's three columns, shared by the header and every row so the two cannot fall out of line. */
const COLUMNS = "grid grid-cols-[minmax(0,1fr)_180px_72px] items-center gap-4 px-4";

/** One enhancement: its name, its default model drawn from the catalogue, and whether Autopilot may suggest it. */
const EnhancementRow = ({ entry, enhancement }: { entry: FamilyEntry | undefined; enhancement: Enhancement }) => {
    const { t } = useTranslation();
    const { values, update } = useSettingsDraft();
    const { family, icon: Icon } = enhancement;
    const name = t(enhancement.nameKey);

    // No model vocabulary of its own: the names, the labels and which precisions each is published at
    // all come from `modelChoice`, over the catalogue, and the label is `modelLabel`'s.
    const { options, selected } = modelChoice(entry, values.models[family]);
    const allowed = !values.autopilotExcluded.includes(family);

    return (
        <div data-slot="enhancement-row" className={cn(COLUMNS, "h-12.5")}>
            <span className="flex min-w-0 items-center gap-2.5 text-sm">
                <Icon className="size-4 flex-none text-muted-foreground" aria-hidden />
                <span className="truncate">{name}</span>
            </span>

            <SettingsSelect
                label={name}
                value={selected}
                className="h-8 w-full"
                onChange={(value) => update({ models: { ...values.models, [family]: value } })}
            >
                {options.map((option, index) => (
                    <Fragment key={option.value}>
                        {/*
                         * A rule between models, not between options. Every model contributes one option
                         * per precision it publishes, so the list reads as pairs - Stockholm HD, Stockholm
                         * SD, Gothenburg HD, Gothenburg SD - and without a break the eye has to re-read
                         * each label to find where one model ends. `first` is the option's own, set where
                         * the list is built, so the model tray's marker falls exactly where this rule does.
                         */}
                        {index > 0 && option.first && <SettingsSelectSeparator />}

                        <SettingsSelectItem value={option.value}>{modelLabel(t, option)}</SettingsSelectItem>
                    </Fragment>
                ))}
            </SettingsSelect>

            <span className="flex justify-end">
                {/*
                 * Named for its family: a column of seven switches that all announce as "Autopilot" is
                 * seven controls nobody can tell apart.
                 *
                 * Written as the list of families switched *off* - see `SettingsData.autopilotExcluded`
                 * for why - and in `ENHANCEMENTS`' order, so the stored list does not depend on the
                 * order the user happened to click in.
                 */}
                <SettingsSwitch
                    checked={allowed}
                    aria-label={t("settings.enhancements.autopilotLabel", { name })}
                    onCheckedChange={(on) =>
                        update({
                            autopilotExcluded: familiesWhere((candidate) =>
                                candidate === family ? !on : values.autopilotExcluded.includes(candidate),
                            ),
                        })
                    }
                />
            </span>
        </div>
    );
};

/**
 * Screen 16c: one table, one row per enhancement in pipeline order - its default model and whether
 * Autopilot may suggest it, side by side, so each family is listed once. Detection gets no row.
 */
export const EnhancementsPage = () => {
    const { t } = useTranslation();
    const families = useCatalogue();

    // One component drawn per entry of `ENHANCEMENTS` rather than one per family: what differs between
    // them is which family they are for, and that is data - so a family added to that list is a row,
    // a switch and an "allowed" default here without a file being written. Detection is named nowhere
    // in that list, which is how the catalogue's eighth family stays out of a dialog about enhancements.
    return (
        <SettingsCard>
            <div className={cn(COLUMNS, "h-9 font-medium text-foreground-dim text-xs")}>
                <span>{t("settings.enhancements.columns.enhancement")}</span>
                <span>{t("settings.enhancements.columns.model")}</span>
                <span className="text-right">{t("settings.enhancements.columns.autopilot")}</span>
            </div>

            {ENHANCEMENTS.map((enhancement) => (
                <EnhancementRow
                    key={enhancement.family}
                    enhancement={enhancement}
                    entry={familyEntry(families, enhancement.family)}
                />
            ))}
        </SettingsCard>
    );
};
