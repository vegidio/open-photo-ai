import { useMemo } from 'react';
import { Divider } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ModelSelector, type ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { ScaleSelector } from '@/features/enhancements/ScaleSelector';
import { useOptionEnhancement } from '@/hooks';
import { modelLabel } from '@/i18n/format';
import { Kyoto, Osaka, Saitama, Tokyo } from '@/operations';

type OptionsUpscaleProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

export const OptionsUpscale = ({ anchorEl, open, onClose }: OptionsUpscaleProps) => {
    const { t } = useTranslation();

    // Built here rather than at module scope: t() called at module-evaluation time would freeze the labels and
    // descriptions in whatever language was active on the first import and never follow a change.
    const options = useMemo<ModelSelectorOption[]>(
        () => [
            {
                value: 'tokyo_fp32',
                label: modelLabel(t, 'Tokyo', 'fp32'),
                description: t('enhancements.upscale.models.tokyo'),
            },
            { value: 'tokyo_fp16', label: modelLabel(t, 'Tokyo', 'fp16') },
            {
                value: 'kyoto_fp32',
                label: modelLabel(t, 'Kyoto', 'fp32'),
                description: t('enhancements.upscale.models.kyoto'),
            },
            { value: 'kyoto_fp16', label: modelLabel(t, 'Kyoto', 'fp16') },
            {
                value: 'saitama_fp32',
                label: modelLabel(t, 'Saitama', 'fp32'),
                description: t('enhancements.upscale.models.saitama'),
            },
            { value: 'saitama_fp16', label: modelLabel(t, 'Saitama', 'fp16') },
            // Osaka is published only as fp16, so the HD slot is shown for consistency with the other models but
            // cannot be selected. The description sits on the MD entry rather than the HD one, because a disabled MUI
            // button gets `pointer-events: none` and its tooltip would never open.
            { value: 'osaka_fp32', label: modelLabel(t, 'Osaka', 'fp32'), disabled: true },
            {
                value: 'osaka_fp16',
                label: modelLabel(t, 'Osaka', 'fp16'),
                description: t('enhancements.upscale.models.osaka'),
            },
        ],
        [t],
    );

    const { model, amount, onModelChange, onAmountChange } = useOptionEnhancement(
        'up',
        (op) => op?.options.scale ?? '1',
        (nextModel, nextScale) => {
            if (nextScale === '') return;
            const scale = parseFloat(nextScale);
            const [name, precision] = nextModel.split('_');

            switch (name) {
                case 'tokyo':
                    return new Tokyo(scale, precision);
                case 'kyoto':
                    return new Kyoto(scale, precision);
                case 'saitama':
                    return new Saitama(scale, precision);
                case 'osaka':
                    return new Osaka(scale, precision);
            }
        },
    );

    return (
        <OptionsPopover title={t('enhancements.upscale.name')} anchorEl={anchorEl} open={open} onClose={onClose}>
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={model} onChange={onModelChange} />

                <Divider />

                <ScaleSelector value={amount} onChange={onAmountChange} />
            </div>
        </OptionsPopover>
    );
};
