import type { ParseKeys } from "i18next";
import { Cpu, type LucideIcon, SlidersHorizontal, Sparkles, Upload } from "lucide-react";
import type { SettingsData } from "@/stores/settings";

/** One page of the settings surface: what the nav draws for it, what it opens with, and what it holds. */
export type SettingsPage = {
    id: string;
    icon: LucideIcon;
    titleKey: ParseKeys;
    descriptionKey: ParseKeys;
    // The preferences rather than the rows, because this is what two things read: Reset to defaults
    // puts exactly these back, and the check below proves every preference is on a page.
    /** The preferences this page holds, as the store names them. */
    keys: readonly (keyof SettingsData)[];
};

// One table read by the nav, the page pane and Reset, so the three cannot disagree about which page a
// preference is on. The icons are the ones the design's paths draw.
/** The four pages, in the order the nav lists them. The surface opens on the first. */
export const SETTINGS_PAGES = [
    {
        id: "general",
        icon: SlidersHorizontal,
        titleKey: "settings.pages.general.title",
        descriptionKey: "settings.pages.general.description",
        keys: ["language", "background", "analytics"],
    },
    {
        id: "performance",
        icon: Cpu,
        titleKey: "settings.pages.performance.title",
        descriptionKey: "settings.pages.performance.description",
        keys: ["processor"],
    },
    {
        id: "enhancements",
        icon: Sparkles,
        titleKey: "settings.pages.enhancements.title",
        descriptionKey: "settings.pages.enhancements.description",
        keys: ["models", "autopilotExcluded"],
    },
    {
        id: "export",
        icon: Upload,
        titleKey: "settings.pages.export.title",
        descriptionKey: "settings.pages.export.description",
        keys: ["quality"],
    },
] as const satisfies readonly SettingsPage[];

// `as const satisfies` above rather than a `SettingsPage[]` annotation, which is what makes this a
// union of literals rather than `string` - so the page bodies in `SettingsDialog` can be total over it.
/** The id of a page, as a union of the four the table declares. */
export type SettingsPageId = (typeof SETTINGS_PAGES)[number]["id"];

/**
 * Nothing, or the compile error that a preference is on no page.
 *
 * `Exclude` is `never` exactly while every key of `SettingsData` is listed above, so a field added to
 * the store and forgotten here stops the typecheck, naming itself in the error - a preference nobody
 * can reach is a build failure, not a setting that silently has no control. The reverse needs no
 * check: `keys` is typed as `keyof SettingsData`, so a page listing a key the store lacks does not
 * compile where it is written. A key listed on two pages is `pages.test.ts`'s to catch.
 */
type Unplaced<T extends never = Exclude<keyof SettingsData, (typeof SETTINGS_PAGES)[number]["keys"][number]>> = T;
export type _EveryPreferenceIsOnAPage = Unplaced;
