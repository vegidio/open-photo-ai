import { useTranslation } from "react-i18next";
import { RATIOS } from "@/features/crop/ratios";
import { cn } from "@/lib/utils";

// At 1 the grid is the design's own sizes, which read as a row of faint dots rather than as ten
// shapes to be told apart - which is the whole job the grid has, so it is drawn a quarter again as
// large.
//
// What bounds it: each cell is 107px - the 256px settings column less its 16px padding a side, halved
// across the 10px gap - and the well, the 8px beside it and the label share that. At this scale the
// well is 45px and the label has about 54px for four characters at 12px, so there is room to grow it
// by half again before the label starts to run out.
/**
 * How large the grid draws, as a multiple of the shapes `ratios.ts` declares.
 *
 * **The one number to change to resize the grid.** Every rectangle is the table's own proportions
 * times this, so they all grow together and none of the ten changes shape; the well grows with them
 * so the ring keeps the same margin around the widest of them whatever this is set to.
 */
const RATIO_BOX_SCALE = 1.25;

// 36px, the size the design draws it at. It has to be scaled with the boxes rather than fixed: the
// widest of them is 21x12, whose diagonal is 24px, and a rectangle is inscribed in a circle by its
// diagonal - so a box that grew inside a well that did not would eventually push its corners
// through the ring.
/** The well's diameter at a scale of 1, which everything about the well is derived from. */
const WELL_SIZE = 36;

/** The well's ring, which is a constant 2px rather than a scaled one - a hairline is a hairline. */
const WELL_BORDER = 2;

/**
 * The aspect-ratio grid: ten options, two to a row, each drawn as a rectangle in its own proportions.
 *
 * Free is dashed, being the absence of a constraint rather than a shape. The option in force is
 * ringed in the accent colour and filled, which is the design's own marking, and each option is a
 * `<button>` with `aria-pressed`.
 */
export const AspectRatios = ({
    selected,
    onSelect,
}: {
    selected: string;
    onSelect: (key: string) => void;
}) => {
    // **The shape is the icon.** The reference draws Material's `MdCropLandscape` and `MdCropPortrait`
    // glyphs, which say "this is a landscape ratio" and nothing about *which* - 5:4, 4:3 and 3:2 are
    // three copies of one picture there. The design replaces them with a box in each ratio's own
    // proportions, so the grid can be read without reading it; `RatioOption.box` says where those
    // proportions come from.
    const { t } = useTranslation();

    return (
        <div className="grid grid-cols-2 gap-x-2.5 gap-y-2" data-slot="aspect-ratios">
            {RATIOS.map((option) => {
                const marked = option.key === selected;

                // A `<button>` per option rather than a radio group: choosing a ratio *reshapes the box
                // now* - it is an action with a lasting mark, not a field being filled in.
                return (
                    <button
                        key={option.key}
                        type="button"
                        aria-pressed={marked}
                        onClick={() => onSelect(option.key)}
                        className="flex min-w-0 items-center gap-2 rounded-md text-left text-xs leading-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-hidden"
                        data-slot="aspect-ratio"
                    >
                        <span
                            style={{
                                width: WELL_SIZE * RATIO_BOX_SCALE,
                                height: WELL_SIZE * RATIO_BOX_SCALE,
                                borderWidth: WELL_BORDER,
                            }}
                            className={cn(
                                "flex flex-none items-center justify-center rounded-full transition-colors",
                                marked
                                    ? "border-primary bg-secondary text-foreground"
                                    : "border-input text-muted-foreground hover:border-ring",
                            )}
                        >
                            {/*
                             * `currentColor` and an inline size: the well above owns the colour, and
                             * the proportions are data - a Tailwind class cannot be built from a
                             * number the table carries, and `w-[18px]` per option would be ten
                             * literals restating the table, none of which could be scaled.
                             *
                             * Rounded per edge rather than left fractional, so every rectangle lands
                             * on whole pixels and its 1.5px stroke stays as crisp as its neighbours'.
                             */}
                            <span
                                style={{
                                    width: Math.round(option.box.width * RATIO_BOX_SCALE),
                                    height: Math.round(option.box.height * RATIO_BOX_SCALE),
                                }}
                                className={cn(
                                    "box-border rounded-[2px] border-[1.5px] border-current",
                                    option.key === "free" && "border-dashed",
                                )}
                            />
                        </span>

                        <span className={cn("truncate", marked ? "text-foreground" : "text-muted-foreground")}>
                            {option.translate ? t(option.label) : option.label}
                        </span>
                    </button>
                );
            })}
        </div>
    );
};
