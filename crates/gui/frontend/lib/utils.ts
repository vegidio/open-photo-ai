import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * Joins class names, then resolves Tailwind conflicts left-to-right.
 *
 * Every component shadcn generates imports this, which is the reason it exists here rather than
 * each one composing `clsx` itself. The `twMerge` half is the part that matters: `clsx` alone would
 * emit `px-2 px-4` and leave the winner to stylesheet order, so a caller could not reliably
 * override a variant's padding by passing `className`.
 */
export const cn = (...inputs: ClassValue[]) => twMerge(clsx(inputs));

/** `value` brought inside `min` and `max`, either of which may be absent - a range not yet published. */
export const clampTo = (value: number, min?: number, max?: number) => {
    const lowered = max === undefined ? value : Math.min(value, max);

    return min === undefined ? lowered : Math.max(lowered, min);
};

/**
 * A file's size as the export queue draws it: kilobytes or megabytes, to two places, untranslated.
 *
 * Binary units, as the file managers on two of the three platforms count them, so the size a row reports is the one
 * the user finds beside the file. Not `features/setup/size.ts`'s `formatSize`, which spells a download's published
 * size in whole decimal megabytes, as the archive listings do.
 */
export const formatBytes = (bytes: number) =>
    bytes >= 1024 * 1024 ? `${(bytes / (1024 * 1024)).toFixed(2)} MB` : `${(bytes / 1024).toFixed(2)} KB`;
