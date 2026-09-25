import { useCallback, useEffect, useState } from "react";
import { DrawerBody } from "@/features/drawer/DrawerBody";
import { DrawerHeader } from "@/features/drawer/DrawerHeader";
import { DRAWER_BLEEDING, DRAWER_HEIGHT } from "@/lib/constants";
import { useDrawerStore } from "@/stores/drawer";
import { useFileStore } from "@/stores/files";

/**
 * The strip of open images, drawn over the bottom of the canvas.
 *
 * Folded, the body is translated down by exactly its own height, so the 48px header stays flush with
 * the window's bottom edge. The canvas above is inset by that 48px permanently, whether the drawer is
 * folded or not, so unfolding slides this over the canvas without resizing it.
 *
 * **It unfolds itself whenever the number of open images changes to more than one**, and **folds
 * itself when the last open image is closed**. A press outside it folds it too.
 */
export const Drawer = () => {
    const open = useDrawerStore((state) => state.open);
    const setOpen = useDrawerStore((state) => state.setOpen);

    // The count rather than the list, so this re-renders when an image is added and not when one is
    // picked or made current.
    const count = useFileStore((state) => state.files.length);

    /*
     * The drawer's own element, in state rather than in a ref, because two things read it and one of
     * them has to re-render when it arrives: the click-away check below, and the strip, which hands it
     * to each thumbnail's menu as the element to portal into. A ref is `null` on the render that
     * mounts this subtree and mutating it later re-renders nothing, so the menus would portal to the
     * body - outside the element the check tests containment against, which is the whole problem:
     * choosing a menu action would fold the drawer out from under the menu being operated. Portalled
     * into the drawer, the containment is true rather than special-cased, for the next portal-based
     * control in the strip as much as for the menu.
     *
     * The callback is `useCallback`'d and that is load-bearing rather than tidy: React detaches and
     * re-attaches a ref callback whose identity changed, so an inline one would be called with `null`
     * and then the node on every render - writing state each time, and settling on whichever came
     * last. Stable, it is called once on mount.
     */
    const [element, setElement] = useState<HTMLDivElement | null>(null);
    const ref = useCallback((node: HTMLDivElement | null) => setElement(node), []);

    useEffect(() => {
        // Here rather than in `Preview`, where the reference has it: nothing about unfolding a drawer
        // is the canvas's business.
        //
        // On the count, not on the batch, which is what makes the two subtle cases fall out rather
        // than being written: a batch that is entirely already open does not change the count, so a
        // drawer the user folded stays folded; a batch with something genuinely new in it does, so a
        // folded drawer opens again. That is the reference's behaviour, keyed on the same number, and
        // it is deliberate - unfolding only on the crossing from one to two is what "a self-opening
        // drawer" usually means and is not what this does.
        if (count > 1) setOpen(true);

        // By either route - one image closed singly, or all of them at once. Keyed on the same count
        // rather than written into the menu's two handlers, where the reference does it: the drawer is
        // what knows whether it is showing, and a rule expressed at the two call sites would have to
        // be repeated at the third. An unfolded drawer over an empty strip reports nothing and covers
        // the canvas that has just become the only thing left to say - which is the state the window
        // starts in.
        if (count === 0) setOpen(false);
    }, [count, setOpen]);

    useEffect(() => {
        if (!open) return;

        /*
         * A click outside folds the drawer, so that the canvas it is covering is reachable again by
         * the same gesture that moves attention back to it - which is what the reference's
         * `ClickAwayListener` does. A listener on the document rather than an overlay, because an
         * overlay would swallow the click that the canvas or the sidebar is being reached *for*.
         *
         * `pointerdown` rather than `click`: a press that starts inside the strip and drifts outside
         * it - a scroll of the strip on a trackpad, a drag begun on a thumbnail - reports its click
         * against the common ancestor, which is outside this element, and would fold the drawer out
         * from under the gesture. Where the press went down is the honest answer to "did the user
         * reach past the drawer".
         *
         * Registered only while the drawer is open, so a folded drawer costs the document nothing.
         *
         * Tauri's drag-and-drop is not a DOM event, so a drop over the canvas produces no click and
         * leaves this open - and then the effect above unfolds it anyway. That is the intended
         * sequence rather than a conflict: images arrived, and the strip is what has something new to
         * say about them.
         */
        const onPointerDown = (event: PointerEvent) => {
            if (!element?.contains(event.target as Node)) setOpen(false);
        };

        document.addEventListener("pointerdown", onPointerDown);

        return () => document.removeEventListener("pointerdown", onPointerDown);
    }, [open, setOpen, element]);

    return (
        <div
            ref={ref}
            className="absolute inset-x-0 bottom-0 z-10 bg-card transition-transform duration-300 ease-out"
            /*
             * The fold is expressed here rather than as a `translate-y-32` class, because 32 in
             * Tailwind's scale is 8rem is `DRAWER_HEIGHT` - the same number in a second unit, in a
             * second place, with nothing linking the two.
             */
            style={{
                height: DRAWER_HEIGHT + DRAWER_BLEEDING,
                transform: open ? undefined : `translateY(${DRAWER_HEIGHT}px)`,
            }}
        >
            <DrawerHeader open={open} />

            <DrawerBody height={DRAWER_HEIGHT} drawer={element} />
        </div>
    );
};
