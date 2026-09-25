// A constant on the one `DialogContent` rather than a class string written at each use, because the
// card is the thing the two states have most in common and a duplicated string is exactly what lets
// them drift - a `twMerge` miss on one of two copies of this makes one card narrower than the other.
// One card cannot disagree with itself.
//
// `sm:max-w-[35rem]` replaces shadcn's own `sm:max-w-lg` rather than fighting it - `twMerge` resolves
// the two because they carry the same modifier.
/**
 * The card the setup dialog is drawn on, in both of its states: one width, one radius, one border,
 * one shadow.
 */
export const SETUP_SURFACE =
    "overflow-hidden rounded-xl border-border bg-card p-0 shadow-[0_24px_60px_rgb(0_0_0/0.7)] sm:max-w-[35rem]";
