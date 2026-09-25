import { type ComponentProps, type ReactNode, useState } from "react";
import { Ellipsis } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { ImageRecord } from "@/ipc/images";
import { revealImage } from "@/ipc/images";
import { isMacOs, isWindows } from "@/ipc/os";
import { FILE_MENU_GAP, FILE_MENU_GLYPH } from "@/lib/constants";
import { report } from "@/lib/report";
import { cn } from "@/lib/utils";
import { useFileStore } from "@/stores/files";

/** Where the menu sits relative to the control that opened it. */
type Anchor = {
    side: "top" | "bottom";
    align: "start" | "center";
    sideOffset: number;
    alignOffset?: number;
    // `alignOffset` cannot do this shift - see `THUMBNAIL_ANCHOR`.
    /** A further shift along the align axis, in pixels, applied as a margin rather than through Radix. */
    nudge?: number;
};

// Radix expresses this natively - `align="center"` on `side="bottom"` means exactly that relationship -
// and it is the reference implementation's anchoring for the same menu, unchanged.
//
// **Twice the gap, because half of it is spent inside the navbar.** `sideOffset` is measured from the
// trigger's own box, and that box is a `FILE_MENU_GLYPH`-tall control centred in a 48px bar - so there
// are already (48 - 15) / 2 = 16.5px between the trigger's bottom edge and the bar's. One gap's worth
// of offset therefore puts the menu flush against the bar it dropped out of. Two puts it the design's
// own gap clear of the bar, which is the edge a reader's eye measures from.
//
// The thumbnail's anchoring has no equivalent: its trigger sits at the very bottom of the thumbnail,
// so there is nothing between the two for an offset to be spent on.
/** The navbar's menu drops below its control, centred on it (screen 09b). */
export const NAVBAR_ANCHOR: Anchor = { side: "bottom", align: "center", sideOffset: FILE_MENU_GAP * 2 };

// **Not expressible with `align` alone.** Radix offers `start` (leading edges flush), `center` and
// `end` (trailing edges flush); what the design draws is the menu's leading edge *past* the control's
// trailing edge. `align="start"` shifted by the control's own width brings the two edges flush, which
// is what `FILE_MENU_GLYPH` buys here.
//
// **And the last step of it is not expressible with `alignOffset` either.** Radix positions through
// floating-ui's `shift` with a `limitShift` limiter, which clamps a cross-axis offset at the point
// where the floating element would stop overlapping its anchor - a menu is not allowed to float free
// of the control that opened it. The trigger is `FILE_MENU_GLYPH` wide, so 15px is the whole of what
// an `alignOffset` can move this menu: asking for 31 moves it 15 and leaves the corners touching,
// which is measurably what the window draws. So the gap is a margin instead, applied after
// positioning, where nothing clamps it. Measured in the running window: `GAPx=16.0 GAPy=16.0`.
//
// **The alignment is only right while the trigger's box is the glyph's box** - see `FILE_MENU_GLYPH`,
// and `FileMenuTrigger`, which secures it by making the glyph itself the trigger.
//
// A **deliberate divergence from the reference**, which anchors this menu to the icon's centre. The
// design draws the corner relationship, and the design is the authority for this application's
// appearance.
/**
 * The thumbnail's menu rises above its control and extends away from it: the menu's lower leading
 * corner set against the control's upper trailing corner and a gap clear of it in both axes
 * (screen 09), so that the menu does not cover the thumbnail it was opened from.
 */
export const THUMBNAIL_ANCHOR: Anchor = {
    side: "top",
    align: "start",
    sideOffset: FILE_MENU_GAP,
    alignOffset: FILE_MENU_GLYPH,
    nudge: FILE_MENU_GAP,
};

type FileOptionsMenuProps = {
    /** The image every one of the three actions acts on. */
    file: ImageRecord;
    /** Where the menu sits: {@link NAVBAR_ANCHOR} or {@link THUMBNAIL_ANCHOR}. */
    anchor: Anchor;
    // The drawer is `absolute inset-x-0` inside the shell's `overflow-hidden` box, so that box is what
    // actually clips a menu, and it is the one thing that has to be measured against. Without it Radix
    // measures the viewport, which is wider - so a menu opened from the last thumbnail of a scrolled
    // strip would be positioned perfectly and then cut in half by the sidebar's edge. The parent rather
    // than the drawer itself, because the drawer is only 176px tall and this menu is supposed to rise
    // clear above it.
    /**
     * What the menu is portalled into, for the thumbnail's - see `Drawer`. Its *parent* becomes the
     * menu's collision boundary.
     *
     * Absent for the navbar's, which portals to the body as Radix does by default: nothing in the
     * navbar folds when something outside it is pressed. `null` while the drawer's ref has not
     * attached yet, which is one render and is the same case as absent.
     */
    container?: HTMLElement | null | undefined;
    /** The control that opens it, rendered as the trigger. */
    children: ReactNode;
};

/**
 * The file options menu: close this image, close every open image, and show this image in the
 * platform's file manager, which the item names.
 *
 * It acts on the file it is handed rather than on the current one. That is not the same image at both
 * call sites: the navbar names the current image, while a thumbnail's menu acts on the photograph
 * whose thumbnail carries it, which is very often not the one being drawn.
 *
 * The menu closes before any action writes to the store.
 */
export const FileOptionsMenu = ({ file, anchor, container, children }: FileOptionsMenuProps) => {
    // One component for both call sites, parameterised by where it is anchored and by nothing else.
    // The three items, their order, the separator's position and the platform-dependent label are
    // written once, which is the only thing keeping the navbar's copy from drifting from the drawer's -
    // the reference implementation reaches the same conclusion with its shared `MenuFileOptions`. A
    // user who has learned what the control on a thumbnail does has learned what the one in the navbar
    // does.
    const { t } = useTranslation();
    // Held here rather than left to Radix, for one reason: two of the three actions destroy the trigger
    // this menu is anchored to. *Close image* unmounts the thumbnail it opened from, and *Close all
    // images* unmounts the whole strip. Closing first means the unmount happens with no open menu left
    // to return focus into a subtree that has gone.
    const [open, setOpen] = useState(false);

    const closeFile = useFileStore((state) => state.closeFile);
    const closeAll = useFileStore((state) => state.closeAll);

    /*
     * Every action goes through this, so "the menu is shut before the store is touched" is one rule in
     * one place rather than a line each handler has to remember - and forgetting it in one of the three
     * would be invisible until someone closed an image from a thumbnail.
     */
    const act = (action: () => void) => {
        setOpen(false);
        action();
    };

    /*
     * Guarded rather than left floating, exactly as the log file's button in Settings is: a file can be
     * moved or deleted after it was opened, and a menu item that quietly does nothing reads as a broken
     * one. The reason in full goes to the console and the catalogue's own sentence goes to the user -
     * a toast fades before anyone copies a path out of it.
     */
    const reveal = () =>
        act(() => {
            revealImage(file.path).catch((error: unknown) => {
                report("showing the image in the file manager failed", error);
                toast.error(t("errors.revealFileFailed"));
            });
        });

    // The platform the label is written for, through i18next's `context` rather than by interpolating a
    // name into a sentence: word order around a product name is not universal, so the whole sentence
    // has to be one translatable unit. An undefined context falls back to the base key, so everything
    // that is neither macOS nor Windows takes the base key, which names a file manager generically, and
    // there is nothing extra to keep in sync.
    //
    // `isMacOs()` and `isWindows()` answer synchronously, which is what lets the label be right on
    // first paint rather than settling a tick later.
    const platform = isMacOs() ? "darwin" : isWindows() ? "windows" : undefined;

    return (
        /*
         * `modal={false}`, where Radix defaults to `true`. Modal disables pointer events everywhere
         * outside the open menu, so a press on a *second* thumbnail's control never reaches it: the
         * first menu stays up and the second never opens, and the user has to dismiss before they can
         * open another. Non-modal, the press dismisses the first and opens the second in one gesture,
         * which is how a row of controls is expected to behave.
         *
         * Two things go with it and both are wanted here. Modal marks everything outside the menu
         * `aria-hidden` - including the drawer the menu was opened from, which hides the strip from a
         * screen reader while its own menu is up - and it locks body scrolling, which a menu anchored
         * to a thumbnail has no business doing.
         */
        <DropdownMenu open={open} onOpenChange={setOpen} modal={false}>
            <DropdownMenuTrigger asChild>{children}</DropdownMenuTrigger>

            {/*
             * `container` is where the menu is portalled to, and the thumbnail's is the drawer - see
             * `Drawer` for why. Absent, as the navbar leaves it, is Radix's own default.
             *
             * Spread conditionally for the reason the rest of this codebase gives: under
             * `exactOptionalPropertyTypes` an explicit `undefined` is not an absent prop.
             */}
            <DropdownMenuContent
                side={anchor.side}
                align={anchor.align}
                sideOffset={anchor.sideOffset}
                {...(anchor.alignOffset !== undefined && { alignOffset: anchor.alignOffset })}
                {...(container && { container })}
                {...(container?.parentElement && { collisionBoundary: container.parentElement })}
                {...(anchor.nudge !== undefined && { style: { marginLeft: anchor.nudge } })}
                className="min-w-[8rem]"
                data-slot="file-options-menu"
            >
                {/* 13px is the reference's size for these items, and the design's. */}
                <DropdownMenuItem className="text-[13px]" onSelect={() => act(() => closeFile(file.path))}>
                    {t("menu.file.close")}
                </DropdownMenuItem>

                <DropdownMenuItem className="text-[13px]" onSelect={() => act(closeAll)}>
                    {t("menu.file.closeAll")}
                </DropdownMenuItem>

                <DropdownMenuSeparator />

                <DropdownMenuItem className="text-[13px]" onSelect={reveal}>
                    {t("menu.file.showIn", { ...(platform && { context: platform }) })}
                </DropdownMenuItem>
            </DropdownMenuContent>
        </DropdownMenu>
    );
};

type FileMenuTriggerProps = Omit<ComponentProps<"button">, "aria-label" | "style" | "type"> & {
    /** The file the menu will act on, named in the control's accessible label. */
    name: string;
};

/**
 * The control that opens a {@link FileOptionsMenu}: an ellipsis sized to the glyph, and nothing more.
 *
 * `className` is the caller's, for the placement the surrounding row wants; what the trigger *is*
 * stays here.
 */
export const FileMenuTrigger = ({ name, className, ...props }: FileMenuTriggerProps) => {
    // Both call sites - the navbar's file name and the drawer thumbnail's caption - draw the same
    // control, so it is one control. It lives beside the menu because the menu is what its size is
    // *for*: its box must be the glyph's box - see `FILE_MENU_GLYPH`.
    const { t } = useTranslation();

    return (
        <button
            // Spread first, so nothing the trigger is *for* can be overwritten by what wraps it.
            // `DropdownMenuTrigger asChild` clones its child and passes the handlers and the ref that
            // open the menu down as props; a component that dropped them would render a button that
            // looks right and does nothing - which is a failure no type catches.
            {...props}
            type="button"
            aria-label={t("menu.file.options", { name })}
            className={cn("flex flex-none items-center justify-center", className)}
            style={{ width: FILE_MENU_GLYPH, height: FILE_MENU_GLYPH }}
        >
            <Ellipsis className="size-[15px]" strokeWidth={2.75} aria-hidden="true" />
        </button>
    );
};
