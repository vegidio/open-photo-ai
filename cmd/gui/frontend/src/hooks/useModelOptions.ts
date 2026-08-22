import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { modelLabel } from '@/i18n/format';
import { ENHANCEMENTS, type EnhancementType } from '@/utils/enhancement';

/**
 * The model choices an `Options…` popover offers, built from the enhancement registry.
 *
 * Every model appears as an HD/MD pair, with the description on the selectable entry of the pair. That rule used to be
 * restated once per model in each of the seven popovers; here it is written once, so a new model shows up in its
 * popover as soon as it is added to `ENHANCEMENTS`.
 *
 * The options are built inside the hook rather than at module scope because `t()` called at module-evaluation time
 * would freeze the labels and descriptions in whatever language was active on the first import.
 */
export const useModelOptions = (type: EnhancementType): ModelSelectorOption[] => {
    const { t } = useTranslation();

    return useMemo(
        () =>
            ENHANCEMENTS[type].models.flatMap<ModelSelectorOption>(({ id, label, descriptionKey, fp16Only }) => [
                {
                    value: `${id}_fp32`,
                    label: modelLabel(t, label, 'fp32'),
                    // A model with no fp32 build still shows the slot, so the selector keeps the same shape, but it
                    // cannot be chosen. Its description moves to the MD entry, because a disabled MUI button gets
                    // `pointer-events: none` and its tooltip would never open.
                    disabled: fp16Only,
                    description: fp16Only ? undefined : t(descriptionKey),
                },
                {
                    value: `${id}_fp16`,
                    label: modelLabel(t, label, 'fp16'),
                    description: fp16Only ? t(descriptionKey) : undefined,
                },
            ]),
        [t, type],
    );
};
