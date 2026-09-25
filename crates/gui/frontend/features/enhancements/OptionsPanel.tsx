import type { ReactNode } from "react";
import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { PopoverClose, PopoverContent } from "@/components/ui/popover";
import { ADD_MENU_GAP, ENHANCEMENT_ROW_HEIGHT } from "@/lib/constants";

// The design puts the panel 16px clear of the sidebar's leading edge (screen 11), which is the same gap
// the add menu keeps - and unlike that menu, this one needs no correction for the panel's own padding:
// a row spans the sidebar's full width, so its box *is* the sidebar's edge and `sideOffset`, which
// Radix measures from the trigger, is the gap itself.
/** How far the panel sits from the row it opens from. */
const PANEL_GAP = ADD_MENU_GAP;

// `align="start"` puts the two top edges together and this walks the panel down. Well inside the
// row's own height, which is what Radix clamps a cross-axis offset at - floating-ui's `limitShift`
// refuses to let a panel float free of the thing that anchors it.
/** How far down the row the panel's top edge starts: the row's vertical centre. */
const PANEL_ALIGN = ENHANCEMENT_ROW_HEIGHT / 2;

type OptionsPanelProps = {
    /** The enhancement's own name, which is what the panel is titled with. */
    title: string;
    /** That family's controls. */
    children: ReactNode;
};

/** The frame an enhancement's options open in: a titled panel anchored to its row. */
export const OptionsPanel = ({ title, children }: OptionsPanelProps) => {
    // One frame for every family, which is what keeps each family's panel from rediscovering the
    // anchoring: every family differs in what it puts *inside* this, and in nothing else. The
    // reference reaches the same conclusion with its own shared `OptionsPopover`.
    const { t } = useTranslation();

    return (
        <PopoverContent
            side="left"
            align="start"
            sideOffset={PANEL_GAP}
            alignOffset={PANEL_ALIGN}
            // `p-0` over the generated primitive's `p-4`, because this panel is not uniformly padded:
            // the design draws a 40px header flush to the panel's edges with a rule under it, and the
            // body padded inside that. The 296px width and the 10px radius are the design's too, where
            // the primitive is generated at 288px and the application's own 8px.
            className="w-[296px] overflow-hidden rounded-[10px] p-0"
            data-slot="enhancement-options"
        >
            <div className="flex h-10 items-center justify-between border-b border-border pr-1.5 pl-3">
                <span className="font-medium text-muted-foreground text-xs">{title}</span>

                <PopoverClose
                    className="flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-hidden"
                    aria-label={t("common.close")}
                >
                    {/* 1.75 rather than the application-wide 1.5, as every close box in this design is. */}
                    <X className="size-4" strokeWidth={1.75} />
                </PopoverClose>
            </div>

            <div className="flex flex-col gap-4 px-3 py-3.5">{children}</div>
        </PopoverContent>
    );
};
