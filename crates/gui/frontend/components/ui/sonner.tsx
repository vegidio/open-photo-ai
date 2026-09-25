"use client";

import type { CSSProperties } from "react";
import { CircleCheckIcon, InfoIcon, Loader2Icon, OctagonXIcon, TriangleAlertIcon } from "lucide-react";
import { Toaster as Sonner, type ToasterProps } from "sonner";
import { DRAWER_BLEEDING, DRAWER_HEIGHT } from "@/lib/constants";
import { useDrawerStore } from "@/stores/drawer";

/** How far a notice floats above the drawer's folded header, which it is stacked on top of. */
const TOAST_GAP = 24;

/**
 * The application's toaster.
 *
 * The generated file is hand-edited in three places. It reads the theme from `next-themes`, a
 * package this application does not have and must not gain: dark is forced app-wide in `style.css` - `@custom-variant dark` is
 * rebound to the class on `<html>` rather than to `prefers-color-scheme` - so the theme is a
 * constant and a provider that could make it anything else would be a second, contradicting answer.
 * The import is therefore removed and the theme pinned to what the palette already is.
 *
 * `--normal-*` stay as generated: they are already this application's tokens, because the CLI writes
 * shadcn's variable names and `style.css` defines them.
 *
 * The other two edits - where notices sit, and what a warning or a failure is coloured - each carry
 * their reasoning at the props below.
 */
const Toaster = ({ ...props }: ToasterProps) => {
    /*
     * Subscribed to rather than read at the moment a notice is enqueued, which is what the reference
     * does with `getState()` in its `useNotify`. It does that deliberately, so that `App`, `Preview`
     * and `SidebarEnhancements` do not all re-render on every drawer toggle just to recompute a
     * margin they are not displaying - and that argument does not apply here: the offset is a prop of
     * one mounted `<Toaster />` rather than a value computed by a hook every screen calls, so
     * subscribing re-renders the toaster and nothing else.
     *
     * Reading it at enqueue time would instead mean every `toast()` call site passing a style, which
     * is exactly the rule-to-remember that `providers.tsx` exists to avoid.
     */
    const open = useDrawerStore((state) => state.open);

    // Sonner re-reads the offset on each render, so a notice raised before the drawer unfolded moves
    // with it rather than being left behind the strip.
    const bottom = (open ? DRAWER_HEIGHT + DRAWER_BLEEDING : DRAWER_BLEEDING) + TOAST_GAP;

    return (
        <Sonner
            theme="dark"
            className="toaster group"
            /*
             * Bottom centre, above the drawer, which is what the Wails app does and what the window's
             * shape asks for: the drawer runs the full width of the canvas, so the bottom right corner
             * sonner defaults to is the one place a notice is guaranteed to be over something.
             *
             * The bottom offset is derived rather than written as 72 or 200, so that changing either of
             * the drawer's two heights moves the notices with it instead of leaving them overlapping by
             * the difference. It clears the unfolded body and the folded header respectively, which are
             * the two numbers the reference pairs in `useNotify` for the reason its comment gives: change
             * one without the other and the notice either overlaps the drawer or floats above nothing.
             * Only `bottom` is given; the other three keep sonner's own 24px, which is the same gap this
             * measures from.
             *
             * `mobileOffset` repeats it because sonner switches to a separate set of variables below a
             * 600px viewport - a width this window can be resized to - and its mobile default is 16px,
             * which would drop a notice onto the drawer rather than above it.
             */
            position="bottom-center"
            offset={{ bottom }}
            mobileOffset={{ bottom }}
            icons={{
                success: <CircleCheckIcon className="size-4" />,
                info: <InfoIcon className="size-4" />,
                warning: <TriangleAlertIcon className="size-4" />,
                error: <OctagonXIcon className="size-4" />,
                loading: <Loader2Icon className="size-4 animate-spin" />,
            }}
            /*
             * `richColors` is what makes sonner read `data-type` at all: without it every notice is drawn
             * in `--normal-*`, so a refused file and a finished export are the same black card and the
             * only thing that says which is the sentence. The per-type variables it then reads
             * are pointed at this palette's own tokens, because sonner's built-in triples are its own
             * hues and would be the first colours in the application that came from somewhere else.
             *
             * Tinted dark rather than filled, in the proportions the setup dialog's failure header
             * already uses - `border-destructive/28 bg-destructive/12` - so an alert surface looks the
             * same wherever it appears. The mix is against `--popover` rather than `transparent` because
             * a notice floats over the canvas and the image on it; a translucent card would be read
             * through.
             */
            richColors
            style={
                {
                    "--normal-bg": "var(--popover)",
                    "--normal-text": "var(--popover-foreground)",
                    "--normal-border": "var(--border)",
                    "--border-radius": "var(--radius)",

                    "--warning-bg": "color-mix(in oklab, var(--warning) 12%, var(--popover))",
                    "--warning-border": "color-mix(in oklab, var(--warning) 28%, var(--popover))",
                    "--warning-text": "var(--warning)",

                    "--error-bg": "color-mix(in oklab, var(--destructive) 12%, var(--popover))",
                    "--error-border": "color-mix(in oklab, var(--destructive) 28%, var(--popover))",
                    "--error-text": "var(--destructive-bright)",

                    "--success-bg": "color-mix(in oklab, var(--success) 12%, var(--popover))",
                    "--success-border": "color-mix(in oklab, var(--success) 28%, var(--popover))",
                    "--success-text": "var(--success-bright)",

                    "--info-bg": "color-mix(in oklab, var(--primary) 12%, var(--popover))",
                    "--info-border": "color-mix(in oklab, var(--primary) 28%, var(--popover))",
                    "--info-text": "var(--primary)",
                } as CSSProperties
            }
            {...props}
        />
    );
};

export { Toaster };
