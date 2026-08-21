import type { TFunction } from 'i18next';

// The quality tier a precision maps onto. Kept here rather than inline at each call site so "fp32 means HD" is
// written once.
export const qualityLabel = (t: TFunction, precision: string): string =>
    t(precision === 'fp32' ? 'models.quality.hd' : 'models.quality.md');

// City names are proper nouns and stay untranslated; only the quality tier comes from the catalog. Splitting the label
// this way keeps ~30 near-duplicate entries ("Tokyo HD", "Tokyo MD", "Kyoto HD", ...) out of every catalog.
export const modelLabel = (t: TFunction, model: string, precision: string): string =>
    t('models.label', { model, quality: qualityLabel(t, precision) });
