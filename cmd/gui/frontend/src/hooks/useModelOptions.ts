import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { modelLabel } from '@/i18n/format';
import { type EnhancementType, modelTiers } from '@/utils/enhancement';

/**
 * The model choices an `Options…` popover offers, built from the enhancement registry.
 *
 * Every model appears as an HD/SD pair, with the description on the HD entry. That rule used to be restated once per
 * model in each of the seven popovers; here it is written once, so a new model shows up in its popover as soon as it
 * is added to `ENHANCEMENTS`.
 *
 * The pairs themselves come from `modelTiers`, shared with the Settings picker: the two model pickers in the app must
 * offer the same models at the same precisions, and the precision behind each tier is not the same for every model —
 * see `modelPrecisions`.
 *
 * The options are built inside the hook rather than at module scope because `t()` called at module-evaluation time
 * would freeze the labels and descriptions in whatever language was active on the first import.
 */
export const useModelOptions = (type: EnhancementType): ModelSelectorOption[] => {
    const { t } = useTranslation();

    return useMemo(
        () =>
            modelTiers(type).map<ModelSelectorOption>(({ value, label, tier, descriptionKey }) => ({
                value,
                label: modelLabel(t, label, tier),
                // Only on the HD entry: the description is about the model, and repeating it on both halves of the
                // pair would put two info icons on every row.
                ...(tier === 'hd' ? { description: t(descriptionKey) } : {}),
            })),
        [t, type],
    );
};
