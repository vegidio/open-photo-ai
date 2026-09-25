import type { ReactNode } from "react";
import { Particles } from "@/components/ui/particles";
import { useSettingsStore } from "@/stores/settings";

/**
 * The dotted surface: a 1px `--surface-dot` dot every 48px over `--background`.
 *
 * Was `DOTTED_CANVAS` in `lib/constants.ts`, back when the canvas had one surface and nothing chose
 * it. Unchanged in what it draws - it is now one of two answers the canvas chooses between, and
 * separately the *only* answer the Crop/Rotate dialog draws.
 *
 * **Exported for that second reader**, which takes the value rather than the choice: the dialog is
 * dotted always, as screen 18 draws it, so it reads this directly instead of going through
 * {@link useCanvasSurface}. That is what keeps "the dotted surface" one string rather than two that
 * can drift - which is the same reason this stopped being a constant in another file.
 */
export const DOTTED_SURFACE =
    "bg-background bg-[radial-gradient(var(--color-surface-dot)_1px,transparent_1px)] bg-size-[3rem_3rem]";

/**
 * The particle surface, which is the same `--background` with nothing drawn on it by CSS at all.
 *
 * The design sets `background-image:none` here rather than layering the field over the dots, which
 * is what makes the two options exclusive: the colour underneath is identical either way, and the
 * two differ in exactly what is drawn over it.
 */
const BARE = "bg-background";

/**
 * The design's own parameters for the field, carried over rather than upstream's 100 / 0.4.
 *
 * `SPEED` is the one value here that is not a divergence - it is the drift upstream hardcoded, named
 * so that all four of the field's knobs are read and tuned in one place. It is CSS pixels per frame
 * rather than per second, which {@link Particles} explains; raising it much makes the field compete
 * with the photograph it is drawn behind, which is the same ground on which mouse magnetism was cut.
 */
const QUANTITY = 120;
const COLOR = "#ffffff";
const SIZE = 0.6;
const SPEED = 0.2;

/**
 * What the canvas is made of: the classes its own element carries, and the field drawn inside it.
 *
 * **One caller, and the branch still lives here.** This anticipated a second reader in the
 * Crop/Rotate dialog, and that dialog turned out not to want the *branch*: screen 18 draws its
 * surface dotted whatever the canvas is set to, so it reads {@link DOTTED_SURFACE} directly. What
 * the two regions share is therefore the dotted surface's definition, not the question of which
 * surface to draw - so the value is exported and the choice is not.
 *
 * It stays in `lib/` rather than moving into `features/preview/` because that exported value has a
 * reader in another feature, and a `use` reaching from `features/crop/` into `features/preview/` is
 * the front-end form of the cross-model import the project layout forbids.
 *
 * **Read from the applied value, not the preferred one, so it applies on Save.** `stores/canvas.ts`
 * says why those are two different things: the settings rows write their store as a control is
 * touched, so a canvas subscribed to `settings.background` would follow the radio live and Cancel
 * could only change it back afterwards. Reading what the dialog published on Save is what makes the
 * canvas keep its surface throughout a draft that is cancelled.
 *
 * Returned as a pair rather than as one wrapping component because the element it describes already
 * exists at each call site, with its own layout, its own ref and its own handlers on it. Wrapping
 * that element would mean either replacing it or adding a layer between it and its children.
 */
export const useCanvasSurface = (): { className: string; field: ReactNode } => {
    /*
     * Read straight off the settings store, which holds only preferences that are in force: a
     * background the user picked but has not saved lives in the dialog's own draft and is invisible
     * here. This used to read a second store holding the applied copy, which existed only because
     * the settings store held drafts - see `stores/settings.ts`.
     */
    const background = useSettingsStore((state) => state.background);

    return {
        className: background === "dotted" ? DOTTED_SURFACE : BARE,
        field:
            background === "particles" ? (
                <Particles quantity={QUANTITY} color={COLOR} size={SIZE} speed={SPEED} />
            ) : undefined,
    };
};
