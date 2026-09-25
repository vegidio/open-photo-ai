import { create } from "zustand";
import { persist } from "zustand/middleware";

// The reference's own three values, spelled its way: `side` rather than `sideBySide`. The store below
// reads back what the Wails application wrote, so a renamed value would rehydrate as a mode this one
// does not have. The order is the design's, and it is presentation - which is why the control maps
// over this rather than listing them again.
/** The three ways the canvas draws the current image, in the order the control offers them. */
export const PREVIEW_MODES = ["full", "side", "split"] as const;

export type PreviewMode = (typeof PREVIEW_MODES)[number];

// The shape `stores/settings.ts` uses for its processors, and needed for the same reason: the control
// reports a plain string - including an empty one when the user presses the mode that is already
// chosen - and a comparison must always be one of the three.
/** Whether a string is one of the three. */
export const isPreviewMode = (value: string): value is PreviewMode =>
    (PREVIEW_MODES as readonly string[]).includes(value);

type PreviewStore = {
    // Named `previewMode` inside a store already called preview, which reads as a stutter and is
    // deliberate: it is the field name the Wails app's `useAppStore` persisted under the key below,
    // and a shorter name here would be a stored choice silently read back as the default. The whole
    // point of sharing the key is that an upgrading user keeps what they chose.
    /** Which of the three comparisons the canvas draws. */
    previewMode: PreviewMode;

    setPreviewMode: (mode: PreviewMode) => void;
};

// Its own store rather than a field on the file store, because it is not about a file: it outlives
// every image being closed, and it outlives the application being quit - which nothing in the file
// store does.
//
// Side by side is the reference's default because it is the mode that shows both halves of what the
// application is for.
/**
 * How the user reads a comparison, and nothing else. Remembered across restarts, and side by side by
 * default, the reference's default.
 */
export const usePreviewStore = create<PreviewStore>()(
    persist(
        (set) => ({
            previewMode: "side",

            setPreviewMode: (mode: PreviewMode) => set({ previewMode: mode }),
        }),
        {
            // The Wails app's `useAppStore` key, unchanged and for the reason `enhancements-storage`
            // is unchanged: a user upgrading from it keeps the comparison they chose. That store held
            // only this one field, so what is read back is the whole of what it wrote.
            name: "app-storage",
        },
    ),
);
