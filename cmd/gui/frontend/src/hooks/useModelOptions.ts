import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { modelLabel } from '@/i18n/format';
import { ENHANCEMENTS, type EnhancementType, modelPrecisions } from '@/utils/enhancement';

/**
 * The model choices an `Options…` popover offers, built from the enhancement registry.
 *
 * Every model appears as an HD/SD pair, with the description on the HD entry. That rule used to be restated once per
 * model in each of the seven popovers; here it is written once, so a new model shows up in its popover as soon as it
 * is added to `ENHANCEMENTS`.
 *
 * The precision behind each half of the pair comes from the registry rather than being written here, because the two
 * tiers do not map onto the same precisions for every model — see `modelPrecisions`.
 *
 * The options are built inside the hook rather than at module scope because `t()` called at module-evaluation time
 * would freeze the labels and descriptions in whatever language was active on the first import.
 */
export const useModelOptions = (type: EnhancementType): ModelSelectorOption[] => {
    const { t } = useTranslation();

    return useMemo(
        () =>
            ENHANCEMENTS[type].models.flatMap<ModelSelectorOption>(({ id, label, descriptionKey }) => {
                const precisions = modelPrecisions(type, id);

                return [
                    {
                        value: `${id}_${precisions.hd}`,
                        label: modelLabel(t, label, 'hd'),
                        description: t(descriptionKey),
                    },
                    {
                        value: `${id}_${precisions.md}`,
                        label: modelLabel(t, label, 'md'),
                    },
                ];
            }),
        [t, type],
    );
};
