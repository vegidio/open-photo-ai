import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { SETTINGS_PAGES, type SettingsPageId } from "./pages.ts";

/** The column on the left: the four pages, one of them current. Choosing one shows it in the pane. */
export const SettingsNav = ({
    active,
    onSelect,
}: {
    active: SettingsPageId;
    onSelect: (page: SettingsPageId) => void;
}) => {
    const { t } = useTranslation();

    // `#141417` is the design's own and has no token: a step below `--card`, so the nav reads as a
    // column beside the page rather than as part of it.
    return (
        <nav
            aria-label={t("settings.title")}
            className="flex w-46 flex-none flex-col gap-0.5 border-border border-r bg-[#141417] px-2 py-3"
        >
            {SETTINGS_PAGES.map(({ id, icon: Icon, titleKey }) => (
                <button
                    key={id}
                    type="button"
                    // `aria-current` rather than only a background: which page is showing is information,
                    // and a colour is the one form of it a screen reader cannot read.
                    aria-current={id === active ? "page" : undefined}
                    onClick={() => onSelect(id)}
                    className={cn(
                        "flex h-8.5 items-center gap-2.5 rounded-md px-2.5 text-left text-[13px] text-muted-foreground transition-colors hover:bg-accent/50",
                        id === active && "bg-accent text-foreground hover:bg-accent",
                    )}
                >
                    <Icon className="size-4 flex-none" aria-hidden />
                    <span className="truncate">{t(titleKey)}</span>
                </button>
            ))}
        </nav>
    );
};
