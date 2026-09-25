import { ChevronsDown, ChevronsUp, Minus, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { PreviewFullIcon, PreviewSideIcon, PreviewSplitIcon } from "@/components/icons/preview-modes";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Separator } from "@/components/ui/separator";
import { Slider } from "@/components/ui/slider";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useCurrentTransform } from "@/hooks/useCurrentTransform";
import { useOpenImages } from "@/hooks/useOpenImages";
import { ZOOM_BUTTON_STEP, ZOOM_MAX, ZOOM_MIN } from "@/lib/constants";
import { track } from "@/lib/faro";
import { useDrawerStore } from "@/stores/drawer";
import { useCurrentFile, useFileStore, useHasFiles } from "@/stores/files";
import { isPreviewMode, PREVIEW_MODES, usePreviewStore } from "@/stores/preview";
import { useTransformStore } from "@/stores/transform";

/** The label and the icon each comparison is offered under, in the order the design draws them. */
const MODES = {
    full: { labelKey: "drawer.preview.full", Icon: PreviewFullIcon },
    side: { labelKey: "drawer.preview.sideBySide", Icon: PreviewSideIcon },
    split: { labelKey: "drawer.preview.split", Icon: PreviewSplitIcon },
} as const;

type DrawerHeaderProps = {
    /** Whether the drawer's body is showing, which is all the fold toggle's icon reports. */
    open: boolean;
};

/**
 * The 48px strip that stays visible when the drawer is folded, and every control it carries.
 *
 * Add images opens the picker and takes no `disabled` at all - adding images is how the empty state
 * stops being empty. The other five are unavailable until an image is open.
 */
export const DrawerHeader = ({ open }: DrawerHeaderProps) => {
    // Six controls in one file rather than six files. They split when enough of them have behaviour to
    // make the indirection worth it; two of them have it now.
    const { t } = useTranslation();
    // Read from the file store here, where it is used, rather than computed in the shell and threaded
    // through `Drawer`: a prop passed through a component that does not read it makes that component's
    // signature claim a dependency it has not got.
    const disabled = !useHasFiles();
    const openImages = useOpenImages("browse");
    const toggleDrawer = useDrawerStore((state) => state.toggle);
    const previewMode = usePreviewStore((state) => state.previewMode);
    const setPreviewMode = usePreviewStore((state) => state.setPreviewMode);

    /*
     * The three states the design draws, derived in one selector rather than read as two numbers and
     * compared here: zustand re-runs a selector on every store write, and a selector returning one of
     * three strings re-renders this header only when the answer actually changes - where
     * `state.selectedPaths` would return a new Set on every pick and re-render it on all of them.
     *
     * `none` covers a window with nothing open as well as one with nothing picked, which is right:
     * the control is unavailable in the first case and reads the same way in both.
     */
    const selection = useFileStore((state) => {
        if (state.selectedPaths.size === 0) return "none";

        return state.selectedPaths.size === state.files.length ? "all" : "some";
    });

    const selectAll = useFileStore((state) => state.selectAll);
    const unselectAll = useFileStore((state) => state.unselectAll);

    /*
     * The zoom control reads the current image's magnification as well as setting it, which is what
     * makes the thumb follow a wheel zoom and makes switching images show that image's own value
     * rather than the one just left.
     *
     * The position is passed through untouched, and no anchor is set: this control is not pointed at
     * any part of the photograph, so the canvas holds the middle of the pane still instead.
     */
    const file = useCurrentFile();
    const transform = useCurrentTransform();
    const setTransform = useTransformStore((state) => state.setTransform);

    const setScale = (scale: number) => {
        if (!file?.identity) return;

        setTransform(file.identity, { scale, x: transform.x, y: transform.y });
    };

    return (
        <div className="flex h-12 items-center gap-2 border-t border-border pr-3 pl-1.5">
            {/*
             * The name is what the control *will do*, not what the drawer is: a button reading Hide
             * images while the strip is showing is the instruction, and the icon is the state. The
             * `data-slot` stays, because tests query by it.
             */}
            <Button
                variant="ghost"
                size="icon-sm"
                disabled={disabled}
                onClick={toggleDrawer}
                aria-label={t(open ? "drawer.hideImages" : "drawer.showImages")}
                data-slot="drawer-fold"
            >
                {open ? <ChevronsDown className="size-5" /> : <ChevronsUp className="size-5" />}
            </Button>

            <Separator orientation="vertical" className="data-[orientation=vertical]:h-5" />

            <Button variant="ghost" size="sm" className="font-normal" onClick={openImages}>
                <Plus className="size-4.5" />
                {t("drawer.addImages")}
            </Button>

            <Separator orientation="vertical" className="data-[orientation=vertical]:h-5" />

            {/*
             * A real `<label>`, because the words next to a checkbox are part of its hit area in
             * every native control. Associated by `htmlFor` rather than by wrapping: Radix renders
             * the checkbox as a `<button role="checkbox">`, and a linter reading the JSX sees a label
             * around no `<input>` and cannot tell the difference. `for` on a labelable element is the
             * association either way, so this states it instead of suppressing the question.
             */}
            <label
                htmlFor="drawer-select-all"
                className="flex h-8 items-center gap-2 rounded-md px-2 text-[13px] has-[:disabled]:opacity-50"
            >
                {/*
                 * Radix's own third state rather than a tick drawn at half opacity: `indeterminate`
                 * is a real value of `checked`, so the control reports `aria-checked="mixed"` and is
                 * announced as a partial selection rather than as an unticked box. The design draws
                 * it as the dash this renders.
                 *
                 * Operating it picks everything unless everything is already picked, which is the
                 * rule the label states: from none and from some it selects all, and only from all
                 * does it unselect. Radix hands `true` for both of the first two, so the write is
                 * read off the state rather than off what the control reports.
                 */}
                <Checkbox
                    id="drawer-select-all"
                    disabled={disabled}
                    checked={selection === "all" ? true : selection === "some" ? "indeterminate" : false}
                    onCheckedChange={() => (selection === "all" ? unselectAll() : selectAll())}
                />
                {t(selection === "all" ? "drawer.unselectAll" : "drawer.selectAll")}
            </label>

            <div className="flex-1" />

            {/*
             * `type='single'`, and with no value at all while nothing is open - which is what the
             * Wails app does (`value={disabled ? undefined : previewMode}`) and what keeps the tray
             * from claiming a comparison for an image that does not exist. Spread conditionally
             * because under `exactOptionalPropertyTypes` an explicit `undefined` is not an absent
             * prop, and Radix reads the two differently.
             */}
            <ToggleGroup
                type="single"
                disabled={disabled}
                {...(!disabled && { value: previewMode })}
                onValueChange={(value: string) => {
                    // Radix reports an empty string when the mode already chosen is pressed again.
                    // A comparison is always one of the three, so that deselection is ignored rather
                    // than stored as "no comparison".
                    if (!isPreviewMode(value)) return;

                    setPreviewMode(value);
                    track("preview_mode_changed", { mode: value });
                }}
                /*
                 * `spacing` rather than a `gap-*` class: at the default 0 the vendored group squares
                 * off every inner corner (`data-[spacing=0]:rounded-none`, with only the first and
                 * last keeping an outer radius), and the design draws three separately rounded 6px
                 * pills inside an 8px tray. Passing a spacing opts out of that joined-segment shape
                 * and sets the 2px gap the design draws, in one place instead of two fighting rules.
                 */
                spacing={0.5}
                className="h-8 rounded-lg bg-secondary p-0.5"
            >
                {PREVIEW_MODES.map((mode) => {
                    const { labelKey, Icon } = MODES[mode];
                    const label = t(labelKey);

                    return (
                        <Tooltip key={mode}>
                            <TooltipTrigger asChild>
                                {/*
                                 * The tooltip's own text as the accessible name, which is the one
                                 * place these three icon-only buttons already have a catalogue
                                 * string to use - `drawer.preview.*` names the mode, not the
                                 * tooltip.
                                 *
                                 * Keyed off `aria-checked`, not the `data-[state=on]` the generated
                                 * toggle styles itself with, because the tooltip above destroys it:
                                 * `asChild` hands the trigger's own props to this item, the item
                                 * spreads them last, and the tooltip's `data-state="closed"` lands
                                 * on top of the toggle's "on", and every mode renders as
                                 * unselected. `aria-checked` is Radix's own answer for a
                                 * `role="radio"`, it is what assistive technology already reads
                                 * here, and nothing overwrites it.
                                 *
                                 * The colour it switches to is the design's, and it has to be: both
                                 * `--accent` (what the generated style reaches for) and
                                 * `--secondary` (the tray) are #27272a in this palette, so even an
                                 * intact `data-[state=on]:bg-accent` would have painted the pill
                                 * exactly the colour of the tray behind it. The design raises the
                                 * chosen one a tier - #3f3f46 on #27272a - and dims the two it is
                                 * not, so the group reads at a glance.
                                 *
                                 * `bg-input` is that #3f3f46, used as a surface rather than a
                                 * border, as the Autopilot switch's unchecked track already does.
                                 *
                                 * The `aria-checked:hover:*` pair is not redundant: the base
                                 * variants carry a plain `hover:bg-muted`, which ties on specificity
                                 * with a single-variant `aria-checked:bg-input` and would drop the
                                 * selected pill back to the tray colour for as long as the pointer
                                 * sat on it. Two variants beat one, so the selection holds.
                                 */}
                                <ToggleGroupItem
                                    value={mode}
                                    aria-label={label}
                                    className="h-7 w-11 rounded-md px-0 text-muted-foreground hover:bg-transparent hover:text-foreground aria-checked:bg-input aria-checked:text-foreground aria-checked:hover:bg-input"
                                >
                                    <Icon className="size-4.5" />
                                </ToggleGroupItem>
                            </TooltipTrigger>

                            <TooltipContent>{label}</TooltipContent>
                        </Tooltip>
                    );
                })}
            </ToggleGroup>

            <Separator orientation="vertical" className="mx-1.5 data-[orientation=vertical]:h-5" />

            <div className="flex min-w-26 flex-[0_1_11.5rem] items-center gap-2.5" data-slot="drawer-zoom">
                {/*
                 * Real buttons where the design draws bare glyphs, which is the same call the fold
                 * toggle got: the mockup draws *every* control as a styled `<span>`, including the
                 * ones already shipped as buttons, so "the mockup drew no button chrome" is not
                 * evidence about interactivity. The reference makes both live, stepping by 0.5.
                 *
                 * A step past either end holds at that end rather than being refused - the store
                 * clamps every write, so this is the same rule the wheel and the slider get.
                 */}
                <Button
                    variant="ghost"
                    size="icon-sm"
                    disabled={disabled}
                    onClick={() => setScale(transform.scale - ZOOM_BUTTON_STEP)}
                    aria-label={t("drawer.zoomOut")}
                >
                    <Minus className="size-4.5 shrink-0" />
                </Button>

                {/*
                 * `thumbLabel` rather than `aria-label`: Radix puts `role="slider"` on the thumb, so
                 * a name passed to the root would land on an element that has no role to name - which
                 * is precisely why the vendored slider has the prop.
                 */}
                <Slider
                    disabled={disabled}
                    min={ZOOM_MIN}
                    max={ZOOM_MAX}
                    step={0.1}
                    value={[transform.scale]}
                    onValueChange={([scale]) => scale !== undefined && setScale(scale)}
                    thumbLabel={t("drawer.zoom")}
                    /*
                     * The bubble the thumb carries under the pointer and through a drag, because this
                     * is the one slider in the application that prints its value nowhere else: the
                     * four export rows have a value span beside them, and the zoom has only a thumb
                     * somewhere along a 1-to-8 track.
                     *
                     * Rounded to the step rather than shown raw - the wheel moves in 0.05s, so a
                     * scale reaches the slider as 1.05 or 3.4500000000000006 - and then trimmed of a
                     * trailing zero through `Number`, so a whole magnification reads 4x rather than
                     * 4.0x. That trim is a deliberate departure from the reference, which formats
                     * every value with `toFixed(1)`.
                     */
                    formatValue={(scale) => t("drawer.zoomScale", { scale: Number(scale.toFixed(1)) })}
                />

                <Button
                    variant="ghost"
                    size="icon-sm"
                    disabled={disabled}
                    onClick={() => setScale(transform.scale + ZOOM_BUTTON_STEP)}
                    aria-label={t("drawer.zoomIn")}
                >
                    <Plus className="size-4.5 shrink-0" />
                </Button>
            </div>
        </div>
    );
};
