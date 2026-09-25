import { useState } from "react";
import { Maximize2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Popover, PopoverAnchor, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { framedDimensions } from "@/ipc/crop";
import type { ImageRecord } from "@/ipc/images";
import { upscaleFactor } from "@/lib/enhancements";
import { cn } from "@/lib/utils";
import { useImageCrop } from "@/stores/crop";
import { useFileEnhancements } from "@/stores/enhancements";

/** `1200 x 1600`, the one spelling of a pair of dimensions in this application. */
const spell = (width: number, height: number) => `${width} x ${height}`;

/** One labelled row of the comparison, which the panel draws two or three of. */
const Row = ({ label, dimensions }: { label: string; dimensions: string }) => (
    <>
        <span className="text-[12px]/[16px]">{label}</span>
        <span className="font-mono text-[12px]/[16px] text-muted-foreground">{dimensions}</span>
    </>
);

/**
 * What the current image measures, and what a crop and an enlargement each cost.
 *
 * **The block reports what the user is working on rather than the file alone**: the enhanced
 * result's dimensions where the stack enlarges the photograph, and otherwise the framing's, falling
 * back to the file's. **Resting on it compares all three.**
 *
 * Absent for a file whose header could not be read, unless it is framed: a *framed* photograph always
 * has dimensions, so a file this application could not measure still reports its framing.
 */
export const NavbarDimensions = ({ file }: { file: ImageRecord }) => {
    const { t } = useTranslation();

    /*
     * The enhancement stack's shape, through the same selector the sidebar's rows read - and the one
     * number derived from it lives in `lib/enhancements.ts` beside the other derivations of that shape
     * rather than inline here.
     */
    const scale = upscaleFactor(useFileEnhancements(file.path));
    const crop = useImageCrop(file.identity);
    const { width, height } = framedDimensions(file, crop);

    /*
     * Hover rather than click, which is what the design draws and what the reference does. A Radix
     * popover is opened by its trigger's press, so the two pointer handlers below are what make it a
     * hover - and `open` is held here because that is the state they write.
     */
    const [open, setOpen] = useState(false);

    // Nothing measurable to report, and a block reading "undefined x undefined" is a worse answer than
    // the one an empty window gets. The framing case cannot reach this: a rectangle with no area is not
    // a framing Rust will describe, so a crop always has both.
    if (width === undefined || height === undefined) return;

    const original = file.width !== undefined && file.height !== undefined ? spell(file.width, file.height) : undefined;
    const framing = crop ? spell(width, height) : undefined;
    const output = spell(Math.round(width * scale), Math.round(height * scale));

    return (
        /*
         * The separator is inside rather than beside, so the whole block is absent together: the block
         * can be absent while the file is not, and a rule beside it would be left with nothing on one
         * side of it.
         */
        <div className="flex h-8 items-center gap-3" data-slot="navbar-dimensions">
            <Popover open={open} onOpenChange={setOpen}>
                {/* Positioned, so the anchor below can sit on the trigger's own bottom edge. */}
                <div className="relative">
                    <PopoverTrigger
                        onPointerEnter={() => setOpen(true)}
                        onPointerLeave={() => setOpen(false)}
                        className={cn(
                            "flex flex-col items-center gap-0.5 rounded-md px-3 py-0.5",
                            /*
                             * The ghost button's hover, so resting on this reads the same way as resting
                             * on Settings beside it - which is what it is, a control in the navbar.
                             *
                             * The four classes are `buttonVariants`' `ghost` arm plus the `transition-all`
                             * from its base, copied rather than reached through `Button`: this is a
                             * two-line block, and borrowing the component would mean overriding its
                             * direction, gap, height, padding, text size and weight to get back to the
                             * shape the design draws. A hover rule that changes in the theme has to be
                             * changed here too, which is the cost of that.
                             */
                            "transition-all hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50",
                        )}
                        data-slot="navbar-dimensions-trigger"
                    >
                        <span className="text-[13px]/[16px]">{t("navbar.dimensions.title")}</span>
                        <span className="font-mono text-[12px]/[16px] text-muted-foreground">
                            {/*
                             * The output size when the stack enlarges the photograph, and otherwise the
                             * framing's - falling back to the file's own. One expression rather than three
                             * branches, because it is one rule, the reference's own: report the size of the
                             * thing the user is working towards.
                             */}
                            {scale > 1 ? output : (framing ?? spell(width, height))}
                        </span>
                    </PopoverTrigger>

                    {/*
                     * What the panel is actually positioned against: a point of no width on the trigger's
                     * bottom edge, at the middle of it.
                     *
                     * With `align="end"` the panel's right edge meets the right edge of whatever it is
                     * anchored to, so anchoring it to a zero-width point puts that corner exactly on the
                     * trigger's bottom centre - which is where the design puts it. Reaching the same place
                     * with `alignOffset` would mean measuring the trigger on every open and halving it,
                     * because the label's width follows the language.
                     */}
                    <PopoverAnchor asChild>
                        <span aria-hidden className="pointer-events-none absolute bottom-0 left-1/2 block size-0" />
                    </PopoverAnchor>
                </div>

                {/*
                 * `pointer-events-none`, so the panel the pointer is now over cannot take the pointer
                 * away from the trigger that opened it - which would close it, put the pointer back over
                 * the trigger, and reopen it, forever. The panel is a comparison to read, not one to
                 * interact with, so it gives up nothing.
                 */}
                <PopoverContent
                    align="end"
                    // 20.5px of air below the block, measured from the anchor above - which sits on the
                    // trigger's bottom edge, so this is the gap between the two the design asks for.
                    //
                    // The half pixel is real on a Retina display and not on any other: Radix rounds the
                    // position it computes to the device pixel ratio, so this lands exactly at 2x and at
                    // 21px on a 1x screen. It is the right way round - the gap is never *under* what was
                    // asked for - and there is no way to have both without drawing the panel blurred.
                    sideOffset={20.5}
                    className="pointer-events-none w-auto p-4"
                    data-slot="navbar-dimensions-popover"
                >
                    <div className="flex flex-row items-center gap-3">
                        <Maximize2 className="size-4" aria-hidden />
                        {/*
                         * Bold here and not on the trigger above, which draws the same string: this one
                         * heads the block of rows under it, and the trigger's is a label beside a value.
                         * Left at the regular weight the heading and the three row labels below it all
                         * weigh the same, and the panel reads as four rows rather than a title over a
                         * comparison.
                         */}
                        <span className="font-bold text-[13px]/[16px]">{t("navbar.dimensions.title")}</span>
                    </div>

                    <div className="mt-2 ml-7 grid grid-cols-[minmax(56px,auto)_auto] gap-x-3 gap-y-2">
                        {/*
                         * Original is the file's own size, always - it is the number the comparison is a
                         * comparison against, and a framing does not change what is on disk. Absent only
                         * for a photograph this application could not measure, which can still be framed.
                         */}
                        {original && <Row label={t("navbar.dimensions.original")} dimensions={original} />}

                        {/* Only where a crop is set: with none, Cropped and Original are one number. */}
                        {framing && <Row label={t("navbar.dimensions.cropped")} dimensions={framing} />}

                        <Row label={t("navbar.dimensions.output")} dimensions={output} />
                    </div>
                </PopoverContent>
            </Popover>

            <Separator orientation="vertical" className="data-[orientation=vertical]:h-5" />
        </div>
    );
};
