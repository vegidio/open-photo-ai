import type { ReactNode } from "react";
import { XIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { DialogClose, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";

/**
 * The 48px bar every dialog in this design is topped with: a title on the left, a close box on the
 * right, a rule underneath.
 *
 * In `components/ui/` rather than in the settings feature that is its first caller, because it is not
 * the settings dialog's: crop, export, faces and About all draw exactly this bar, and the metrics -
 * the height, the asymmetric `0 10px 0 20px` inset, the 28px box the icon sits in - are the design's
 * decision about what a dialog looks like rather than about what any one of them holds.
 *
 * It renders {@link DialogTitle} rather than a styled span, which is what makes it the dialog's
 * accessible name: Radix's content warns when it has none, and a dialog announced only as "dialog" is
 * one a screen reader user has to explore to identify. The visible title and the announced one are
 * therefore the same string by construction rather than by a matching `aria-label` someone has to
 * remember to update.
 *
 * The close control is a {@link DialogClose}, so dismissing goes through Radix's own `onOpenChange`
 * and lands wherever the keyboard's Escape lands. A dialog whose close button called a handler
 * directly would have two ways out that could do different things - which for this application is
 * not academic, since closing the settings dialog has to restore a snapshot.
 *
 * `showCloseButton={false}` draws the bar without it, for a dialog that has to be answered rather
 * than dismissed - named as `DialogContent` names the same switch.
 *
 * `children` sit between the title and the close box, taking the room left - export's summary of its
 * batch. Outside the title, so what the dialog is announced as does not change as the batch runs.
 */
export const DialogTitleBar = ({
    title,
    showCloseButton = true,
    className,
    children,
}: {
    title: ReactNode;
    showCloseButton?: boolean;
    className?: string;
    children?: ReactNode;
}) => {
    const { t } = useTranslation();

    return (
        <div
            data-slot="dialog-title-bar"
            className={cn(
                "flex h-12 flex-none items-center justify-between gap-4 border-b border-border pr-2.5 pl-5",
                className,
            )}
        >
            <DialogTitle className="font-semibold text-sm">{title}</DialogTitle>

            {children}

            {showCloseButton && (
                <DialogClose
                    className="flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-hidden"
                    aria-label={t("common.close")}
                >
                    {/*
                     * 1.75 rather than the application-wide 1.5. The design draws this one glyph heavier
                     * than the rest, and at 16px the difference is the close box reading as a control
                     * rather than as a hairline; `LucideProvider`'s weight is the default, not a rule.
                     */}
                    <XIcon className="size-4" strokeWidth={1.75} />
                </DialogClose>
            )}
        </div>
    );
};
