/**
 * Bucketing for analytics properties derived from the user's own images.
 *
 * Image dimensions are sent bucketed rather than raw: an exact pixel count is a weak fingerprint, and the question the
 * data answers - "how does processing time scale with image size?" - needs a handful of bands, not thousands of
 * distinct values that no dashboard can group.
 *
 * Durations are the opposite case and are deliberately NOT bucketed anywhere: Aptabase aggregates numeric properties
 * itself, and bucketing client-side would throw away the p95, which is the only interesting statistic about a run that
 * spent four minutes compiling a TensorRT engine.
 */

/** The megapixel bands, ordered. Chosen around real camera output: phone, full-frame, high-megapixel, medium format. */
export const MP_BUCKETS = ['<2', '2-6', '6-12', '12-24', '24-50', '50+'] as const;

export type MpBucket = (typeof MP_BUCKETS)[number];

/**
 * Buckets an image's pixel count into a megapixel band.
 *
 * A non-positive or non-finite dimension yields the smallest band rather than throwing: this runs on the analytics
 * path, where a bad value must never be able to break the call site that reports it.
 */
export const mpBucket = (width: number, height: number): MpBucket => {
    if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) return '<2';

    const mp = (width * height) / 1_000_000;

    if (mp < 2) return '<2';
    if (mp < 6) return '2-6';
    if (mp < 12) return '6-12';
    if (mp < 24) return '12-24';
    if (mp < 50) return '24-50';

    return '50+';
};

/**
 * Summarises the file types in one import as a sorted, de-duplicated, comma-joined list of lowercase extensions.
 *
 * Sorted and de-duplicated so that "jpg then raw" and "raw then jpg" are the same value on a dashboard, and so the
 * cardinality stays bounded by the set of supported formats rather than by the order files happen to arrive in.
 * Extensions only - never a file name, which would be user content.
 */
export const formatList = (extensions: readonly string[]): string =>
    [...new Set(extensions.map((ext) => ext.replace(/^\./, '').toLowerCase()).filter(Boolean))].sort().join(',');
