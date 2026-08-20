import { useMemo } from 'react';
import { Divider } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { IntensitySelector } from '@/features/enhancements/IntensitySelector';
import { ModelSelector, type ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { useOptionEnhancement } from '@/hooks';
import { modelLabel } from '@/i18n/format';
import { Paris } from '@/operations';

type OptionsLightAdjustmentProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

export const OptionsLightAdjustment = ({ anchorEl, open, onClose }: OptionsLightAdjustmentProps) => {
    const { t } = useTranslation();

    // Built here rather than at module scope: t() called at module-evaluation time would freeze the labels
    // and descriptions in whatever language was active on the first import and never follow a change.
    const options = useMemo<ModelSelectorOption[]>(
        () => [
            {
                value: 'paris_fp32',
                label: modelLabel(t, 'Paris', 'fp32'),
                description: t('enhancements.lightAdjustment.models.paris'),
            },
            { value: 'paris_fp16', label: modelLabel(t, 'Paris', 'fp16') },
        ],
        [t],
    );

    const { model, amount, onModelChange, onAmountChange } = useOptionEnhancement(
        'la',
        (op) => (Number(op?.options.intensity) * 100).toString(),
        (nextModel, nextIntensity) => {
            const intensity = nextIntensity !== '' && nextIntensity !== '-' ? parseInt(nextIntensity, 10) / 100 : 0;
            const [name, precision] = nextModel.split('_');

            switch (name) {
                case 'paris':
                    return new Paris(intensity, precision);
            }
        },
    );

    return (
        <OptionsPopover
            title={t('enhancements.lightAdjustment.name')}
            anchorEl={anchorEl}
            open={open}
            onClose={onClose}
        >
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={model} onChange={onModelChange} />

                <Divider />

                <IntensitySelector value={amount} onChange={onAmountChange} />
            </div>
        </OptionsPopover>
    );
};
