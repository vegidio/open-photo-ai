import type { ReactNode } from "react";
import { Info, type LucideIcon } from "lucide-react";
import { DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import type { SetupRow } from "@/stores/setup";
import { OverallProgress } from "./OverallProgress";

// Shared because the geometry is - a 40px tile, 16px of gap, a 16px semibold title over 13px prose -
// and because `DialogTitle` and `DialogDescription` are what label the dialog for a screen reader in
// both states.
/** The head of either state: the tinted icon tile, the title and the sentences under it. */
export const SetupHeader = ({
    icon: Icon,
    tint,
    title,
    children,
}: {
    icon: LucideIcon;
    /** The `border-*` and `bg-*` pair for the icon tile, which is the state's accent at 28% and 12%. */
    tint: string;
    title: string;
    children: ReactNode;
}) => (
    <DialogHeader className="flex-row gap-4 px-6 pt-6 pb-5 text-left">
        <span className={cn("flex size-10 flex-none items-center justify-center rounded-[10px] border", tint)}>
            <Icon className="size-5" strokeWidth={1.5} />
        </span>

        <div className="flex flex-col gap-1.5">
            <DialogTitle className="text-base leading-[normal] font-semibold tracking-[-0.01em]">{title}</DialogTitle>

            {/*
             * Two lines' worth of box whether or not the state fills it. Both states say two
             * sentences, but the failure's second one is absent where there is nothing to add about
             * how far the attempt got - and a header that shrank by a line would move everything
             * under it as the dialog changed state. `lh` is the description's own line height, so
             * this stays two lines if the type ever changes.
             */}
            <DialogDescription className="min-h-[2lh] text-[13px]/[1.6] text-pretty">{children}</DialogDescription>
        </div>
    </DialogHeader>
);

/** The overall bar and the bordered list, which both states draw identically. */
export const SetupBody = ({ rows, children }: { rows: SetupRow[]; children: ReactNode }) => (
    <>
        {/*
         * In the failure state too, and not as a leftover: the question - how much of what this machine
         * needs is on disk - has the same answer it had a moment earlier, and the failing component's
         * own bar is gone, so this is the only thing still saying how far the attempt got. It is also
         * what keeps the two states the same height.
         */}
        <OverallProgress rows={rows} />

        {/* Nothing to draw a border around where a failure was raised before anything was planned. */}
        {rows.length > 0 && <div className="mx-6 flex flex-col overflow-hidden rounded-lg border">{children}</div>}
    </>
);

// What it says differs - why this is slow, or why retrying in a few minutes tends to work - but it is
// the same box in the same place, and the failure state withholds it rather than restyling it.
/** The bordered aside both states put under the list: an info icon and one paragraph. */
export const SetupNote = ({ children }: { children: ReactNode }) => (
    <div className="mx-6 mt-4 flex gap-2.5 rounded-lg border bg-background p-3">
        <Info className="mt-px size-4 flex-none text-muted-foreground" strokeWidth={1.5} />
        <p className="text-xs/[1.6] text-pretty text-muted-foreground">{children}</p>
    </div>
);
