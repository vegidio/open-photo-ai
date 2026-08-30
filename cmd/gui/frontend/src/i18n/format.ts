import type { TFunction } from 'i18next';
import type { QualityTier } from '@/utils/enhancement';

// The catalog string for a quality tier.
//
// It takes a tier rather than a precision on purpose. Which precision backs which tier is a per-model fact, not a
// global one — Osaka's HD tier is its fp16 build, because no fp32 build of it exists — so that decision lives in
// `qualityTier`, next to the registry that knows it. What is left here is only the formatting.
export const qualityLabel = (t: TFunction, tier: QualityTier): string => t(`models.quality.${tier}`);

// City names are proper nouns and stay untranslated; only the quality tier comes from the catalog. Splitting the label
// this way keeps ~30 near-duplicate entries ("Tokyo HD", "Tokyo SD", "Kyoto HD", ...) out of every catalog.
export const modelLabel = (t: TFunction, model: string, tier: QualityTier): string =>
    t('models.label', { model, quality: qualityLabel(t, tier) });
