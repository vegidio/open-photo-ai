import type { ReactNode } from "react";
import { Check, Clock, Download, type LucideIcon, PackageOpen } from "lucide-react";
import { cn } from "@/lib/utils";
import type { SetupRow, SetupRowState } from "@/stores/setup";
import { formatSize } from "./size";

// One table rather than four ternaries down the row, so a state added later is one entry and cannot
// be added to three of them. The colours are tokens because every colour in this codebase is - see
// `style.css` on `--success-bright` - and the two working states share the accent deliberately: what
// distinguishes them is the label and the icon, not the colour.
//
// It lives beside the row rather than inside either state of the dialog, because both of them draw
// from it - see `OUTCOME` in `SetupFailure`.
/** How each state a component can report is drawn. */
export const PRESENTATION: Record<SetupRowState, { icon: LucideIcon; accent: string; name: string; row: string }> = {
    installed: { icon: Check, accent: "text-success-bright", name: "text-muted-foreground", row: "" },
    downloading: { icon: Download, accent: "text-primary", name: "text-foreground", row: "bg-primary/6" },
    extracting: { icon: PackageOpen, accent: "text-primary", name: "text-foreground", row: "bg-primary/6" },
    queued: { icon: Clock, accent: "text-foreground-dim", name: "text-foreground-dim", row: "" },
};

/**
 * One component's row, in either state of the dialog: an icon, its name, its published size, what it
 * is doing or did, and one optional line under all of that.
 *
 * The name is not translated and is drawn in the monospace face. **The second line is a fixed 14px
 * slot**, indented to the name, with its content aligned to the bottom.
 */
export const ComponentRow = ({
    row,
    icon: Icon,
    accent,
    nameTier,
    tint,
    state,
    children,
}: {
    row: SetupRow;
    icon: LucideIcon;
    /** The colour of the icon and the state column. */
    accent: string;
    /** Which tier of grey the component's name is drawn in. */
    nameTier: string;
    /** The row's own background, where the state tints it. */
    tint: string;
    /** What this component is doing or did, already translated. */
    state: ReactNode;
    /** The track, or the reason. Nothing for a row with neither, which is then 40pt rather than 54pt. */
    children?: ReactNode;
}) => (
    <div className={cn("flex flex-col border-b px-3 py-2.5", tint)}>
        <div className="flex items-center gap-2.5">
            <Icon className={cn("size-3.75 flex-none", accent)} strokeWidth={1.75} />

            {/*
             * Untranslated and monospace because it is an identifier - the four product names the core
             * library reports, or a model's artifact id.
             */}
            <span className={cn("min-w-0 flex-1 truncate font-mono text-[13px]", nameTier)}>{row.name}</span>

            <span className="font-mono text-xs leading-[normal] text-foreground-dim">{formatSize(row.size)}</span>

            <span className={cn("w-26 text-right text-xs leading-[normal]", accent)} data-slot="setup-row-state">
                {state}
            </span>
        </div>

        {/*
         * The fixed slot is the whole reason this component exists. The progress state puts a 4px track
         * in it and the failure state puts a 12px line of text in it, each aligned to the bottom, so the
         * two come out the same height without either one naming a gap. Two rows with their own `gap-*`
         * drift apart; a slot cannot disagree with itself. `justify-end` is what makes the remaining
         * space read as the gap.
         *
         * The slot also owns the 25px indent that lines its content up with the component name, because
         * it is the only element here that knows the gutter - the icon's `size-[15px]` plus the
         * `gap-2.5` above it. A child that indented itself would be re-deriving that sum, in whatever
         * unit it happened to pick, and would have to know that padding shrinks a `w-full` child where
         * a margin does not.
         */}
        {children && <div className="flex h-3.5 flex-col justify-end pl-6.25">{children}</div>}
    </div>
);
