import type { Operation } from './Operation';

/**
 * Builders for the operation classes.
 *
 * The generated `id` is a contract in three places at once: the Go backend parses it (`cmd/gui/utils.IdsToOperations`),
 * the frontend image cache keys on it, and it is folded into the library's on-disk cache key. Changing a template
 * silently invalidates every cached result and breaks parsing on the Go side, so the three formats below are the single
 * definition of that string — the individual models only supply their prefix and name.
 */

type IntensityOp = new (intensity: number, precision: string) => Operation;
type ScaleOp = new (scale: number, precision: string) => Operation;
type PrecisionOp = new (precision: string) => Operation;

/** `<prefix>_<name>_<intensity>_<precision>` — denoise, sharpen, light adjustment, colour balance. */
export const intensityOp = (prefix: string, name: string): IntensityOp =>
    class {
        id: string;
        options: Record<string, string>;

        constructor(intensity: number, precision: string) {
            this.id = `${prefix}_${name}_${intensity}_${precision}`;
            this.options = { name, intensity: intensity.toString(), precision };
        }
    };

/** `<prefix>_<name>_<scale>x_<precision>` — upscale. */
export const scaleOp = (prefix: string, name: string): ScaleOp =>
    class {
        id: string;
        options: Record<string, string>;

        constructor(scale: number, precision: string) {
            this.id = `${prefix}_${name}_${scale}x_${precision}`;
            this.options = { name, scale: scale.toString(), precision };
        }
    };

/** `<prefix>_<name>_<precision>` — face recovery, whose faces are detected separately and passed alongside. */
export const precisionOp = (prefix: string, name: string): PrecisionOp =>
    class {
        id: string;
        options: Record<string, string>;

        constructor(precision: string) {
            this.id = `${prefix}_${name}_${precision}`;
            this.options = { name, precision };
        }
    };
