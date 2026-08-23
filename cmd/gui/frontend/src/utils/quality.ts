import { ImageFormat } from '@/bindings/github.com/vegidio/open-photo-ai/types';

// The formats whose encoders take a quality setting. The lossless ones (BMP, GIF, PNG, TIFF) ignore it, so they get
// no slider and no stored value.
export type QualityFormat =
    | ImageFormat.FormatAvif
    | ImageFormat.FormatHeic
    | ImageFormat.FormatJpeg
    | ImageFormat.FormatWebp;

// The user's chosen quality for each lossy format, as held by the settings store.
export type QualityChoices = Record<QualityFormat, number>;

export const QUALITY_FORMATS: readonly QualityFormat[] = [
    ImageFormat.FormatAvif,
    ImageFormat.FormatHeic,
    ImageFormat.FormatJpeg,
    ImageFormat.FormatWebp,
];

export const MIN_QUALITY = 1;
export const MAX_QUALITY = 100;

// What the lossless formats are encoded at. They ignore the value entirely; it exists only because SaveImage validates
// the argument it is given.
export const LOSSLESS_QUALITY = 100;

// The value each format starts at, and the mark drawn under its slider. These are exactly what utils.EncodeImage used
// to hardcode, so a user who never moves a slider gets byte-identical output to before the setting existed.
//
// They are deliberately not one shared number: the scales are not comparable across encoders, and 60 in libheif is a
// very different picture to 60 in libjpeg.
export const DEFAULT_QUALITY: QualityChoices = {
    [ImageFormat.FormatAvif]: 60,
    [ImageFormat.FormatHeic]: 60,
    [ImageFormat.FormatJpeg]: 90,
    [ImageFormat.FormatWebp]: 75,
};

export const clampQuality = (value: number) => Math.min(MAX_QUALITY, Math.max(MIN_QUALITY, Math.round(value)));

/**
 * Rebuilds a quality record from whatever was in storage, filling missing formats with their default and clamping the
 * rest into range.
 *
 * The store is persisted, so this value can arrive from an older build that had fewer formats, or from a hand-edited
 * localStorage entry. A 0 or NaN reaching the encoders is not a harmless bad setting: libavif/libheif/libwebp are
 * handed a non-nil options struct, so they take it as a real request rather than falling back to their own defaults,
 * and write out a garbage image.
 */
export const normalizeQuality = (value: unknown): QualityChoices => {
    const stored = (value ?? {}) as Partial<Record<QualityFormat, unknown>>;
    const result = { ...DEFAULT_QUALITY };

    for (const format of QUALITY_FORMATS) {
        const quality = Number(stored[format]);
        if (Number.isFinite(quality) && quality > 0) result[format] = clampQuality(quality);
    }

    return result;
};
