import { useOnce } from "@/hooks/useOnce";
import { catalogue, type FamilyEntry } from "@/ipc/catalogue";

/**
 * The catalogue, once it has arrived.
 *
 * An `invoke`, so there is a render before it resolves - during which every chooser built from it
 * draws nothing. That is the honest state: what a family offers is the library's answer, and until
 * it has answered there is nothing to offer. `catalogue()` keeps the promise for the life of the
 * process, so every consumer mounting at once is one call rather than one each.
 *
 * In `hooks/` rather than beside the settings rows it was written for, because the add menu is its
 * second caller: a `use` reaching from one feature into another is a promotion that has not happened
 * yet.
 */
export const useCatalogue = (): FamilyEntry[] => useOnce(catalogue, [] as FamilyEntry[]);

/**
 * What one family publishes, or `undefined` while the catalogue has not arrived or does not name it.
 *
 * A function over an already-read catalogue rather than a hook that reads one: all three callers
 * need it somewhere a hook cannot go - twice inside a `.map()` over the enhancements on screen, once
 * inside the add menu's click handler - so a `useFamilyEntry` would be uncallable at every site that
 * wants it, and each would go on spelling out the `find`.
 */
export const familyEntry = (families: FamilyEntry[], family: FamilyEntry["family"]): FamilyEntry | undefined =>
    families.find((published) => published.family === family);
