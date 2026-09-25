import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Particles } from "@/components/ui/particles";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { isLanguage, LANGUAGE_NAMES, LANGUAGE_TAGS } from "@/i18n/languages";
import { revealLog } from "@/ipc/logs";
import { report } from "@/lib/report";
import { cn } from "@/lib/utils";
import { BACKGROUNDS, type Background } from "@/stores/settings";
import { useSettingsDraft } from "./draft";
import {
    RADIO_FILL,
    SettingsButton,
    SettingsCard,
    SettingsRow,
    SettingsSelect,
    SettingsSelectItem,
    SettingsSwitch,
} from "./rows.tsx";

// Not by tag: the tag is never shown, so ordering by it would be ordering the list by something the
// user cannot see. `localeCompare` without a locale argument is deliberate - a list sorted under the
// *active* language would reorder itself as the user switched, which for a list whose whole purpose
// is to be recognisable while you cannot read the interface is the wrong kind of helpfulness.
const LANGUAGES = [...LANGUAGE_TAGS].sort((a, b) => LANGUAGE_NAMES[a].localeCompare(LANGUAGE_NAMES[b]));

/**
 * The language, named in the languages themselves. **Sorted by endonym**, in an order that does not
 * change with the active language.
 *
 * The choice is written to the dialog's draft and nowhere else. `changeLanguage` runs on Save, in the
 * dialog.
 */
const LanguageRow = () => {
    const { t } = useTranslation();
    const { values, update } = useSettingsDraft();

    return (
        <SettingsRow
            label={t("settings.app.language.title")}
            description={t("settings.app.language.description")}
            control={
                <SettingsSelect
                    label={t("settings.app.language.title")}
                    value={values.language}
                    // Narrowed rather than cast: the value comes back from a control, and the guard is
                    // already there for the persisted case.
                    onChange={(value) => isLanguage(value) && update({ language: value })}
                >
                    {LANGUAGES.map((tag) => (
                        <SettingsSelectItem key={tag} value={tag}>
                            {LANGUAGE_NAMES[tag]}
                        </SettingsSelectItem>
                    ))}
                </SettingsSelect>
            }
        />
    );
};

/**
 * Whether the application may send anonymous usage analytics.
 *
 * **It edits the draft and does nothing else.** Save puts the choice in the store, and
 * `lib/analytics.ts` hands it to Rust from there: off stops sending at once, on takes effect at the next
 * launch. Turning it off in particular reports nothing anywhere - an opt-out that is transmitted is not
 * an opt-out.
 */
const AnalyticsRow = () => {
    const { t } = useTranslation();
    const { values, update } = useSettingsDraft();

    return (
        <SettingsRow
            label={t("settings.app.analytics.title")}
            description={t("settings.app.analytics.description")}
            control={
                <SettingsSwitch
                    checked={values.analytics}
                    onCheckedChange={(analytics) => update({ analytics })}
                    aria-label={t("settings.app.analytics.title")}
                />
            }
        />
    );
};

/**
 * What a tile shows of its surface: the real particle field at the design's sample density, or the
 * canvas's own dot at a pitch that reads as a grid in 52px.
 *
 * The dot is `--color-surface-dot`, so the tile and the canvas cannot disagree about what a dot looks
 * like. Only the pitch is the tile's own: at the canvas's 48px, a 52px-high tile shows one row of dots
 * and does not read as a grid at all.
 */
const SAMPLE: Record<Background, ReactNode> = {
    particles: <Particles quantity={40} color="#ffffff" size={0.4} />,
    dotted: (
        <div className="absolute inset-0 bg-[radial-gradient(var(--color-surface-dot)_1px,transparent_1px)] bg-size-[16px_16px]" />
    ),
};

/**
 * Which of the two surfaces the preview canvas draws behind the image, each shown as a live sample.
 *
 * Like the language, the choice is written to the dialog's draft and nowhere else - the canvas reads
 * the settings store itself, so what is behind the dialog keeps the surface it had until Save.
 */
const BackgroundCard = () => {
    const { t } = useTranslation();
    const { values, update } = useSettingsDraft();
    const title = t("settings.app.background.title");

    // Not live, although the design's mock toggles the canvas as the radio is clicked: that
    // demonstrates two states on one page rather than claiming the preference applies live.
    return (
        <SettingsCard>
            {/* One child, so the rule the card draws between its children has nothing to divide. */}
            <div className="flex flex-col gap-3 px-4 pt-3.5 pb-4">
                <div className="flex flex-col gap-1">
                    <span className="text-sm">{title}</span>
                    <p className="m-0 text-foreground-dim text-xs leading-normal">
                        {t("settings.app.background.description")}
                    </p>
                </div>

                {/*
                 * A radio group rather than two buttons, so a screen reader hears "radio group, Particles
                 * selected". Named by the card's own title: a group announces its options and not what the
                 * choice is *about*.
                 */}
                <RadioGroup
                    className="grid grid-cols-2 gap-3"
                    value={values.background}
                    aria-label={title}
                    // Narrowed by the list rather than cast: Radix hands back the `value` of whichever item
                    // was chosen, and those are the only values this rendered.
                    onValueChange={(next) => {
                        const chosen = BACKGROUNDS.find((background) => background === next);
                        if (chosen) update({ background: chosen });
                    }}
                >
                    {BACKGROUNDS.map((background) => {
                        const id = `settings_background_${background}`;

                        return (
                            <div key={background} className="flex min-w-0 flex-col gap-2.5">
                                {/*
                                 * The tile is a second label for the same radio, which makes the whole
                                 * tile the hit target without nesting the radio's button inside a label.
                                 * It carries no text, so the radio's accessible name is still the one below.
                                 */}
                                <label
                                    htmlFor={id}
                                    data-slot="background-tile"
                                    className={cn(
                                        "-outline-offset-1 relative h-13 cursor-pointer overflow-hidden rounded-md bg-background outline-2",
                                        values.background === background ? "outline-primary" : "outline-border",
                                    )}
                                >
                                    {SAMPLE[background]}
                                </label>

                                <div className="flex min-w-0 items-center gap-2">
                                    <RadioGroupItem id={id} value={background} className={RADIO_FILL} />
                                    <label htmlFor={id} className="cursor-pointer truncate text-[13px]">
                                        {t(`settings.app.background.${background}`)}
                                    </label>
                                </div>
                            </div>
                        );
                    })}
                </RadioGroup>
            </div>
        </SettingsCard>
    );
};

/**
 * Show the user their log file, selected in the platform's file manager. A failure is reported in a
 * toast, in the language the application is running in.
 */
const LogsRow = () => {
    const { t } = useTranslation();

    const showLogs = async () => {
        try {
            // One command: `revealLog` resolves the path in Rust, so nothing this side sends decides
            // which file is shown and there is no path to resolve here first.
            await revealLog();
        } catch (error) {
            // Reported rather than passed over - this is the one control in the dialog whose whole
            // purpose is to help someone file a report about something else, and a button that quietly
            // does nothing leaves them with neither the file nor a reason. The reason in full is
            // reported rather than put into the toast: `errors.showLogsFailed` already tells the user
            // where to look in words, and a toast is a notice that fades before anyone copies a path
            // out of it.
            report("showing the log file failed", error);
            toast.error(t("errors.showLogsFailed"));
        }
    };

    return (
        <SettingsRow
            label={t("settings.app.logs.title")}
            description={t("settings.app.logs.description")}
            control={<SettingsButton onPress={showLogs}>{t("settings.app.logs.button")}</SettingsButton>}
        />
    );
};

/** Screen 16: the language and analytics, the preview background, and the logs - three cards. */
export const GeneralPage = () => (
    <>
        <SettingsCard>
            <LanguageRow />
            <AnalyticsRow />
        </SettingsCard>

        <BackgroundCard />

        <SettingsCard>
            <LogsRow />
        </SettingsCard>
    </>
);
