import { useMemo } from 'react';
import { Divider } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { IntensitySelector } from '@/features/enhancements/IntensitySelector';
import { ModelSelector, type ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { useOptionEnhancement } from '@/hooks';
import { modelLabel } from '@/i18n/format';
import { Moscow, Novgorod, Petersburg } from '@/operations';

type OptionsSharpenProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

export const OptionsSharpen = ({ anchorEl, open, onClose }: OptionsSharpenProps) => {
    const { t } = useTranslation();

    // Built here rather than at module scope: t() called at module-evaluation time would freeze the labels
    // and descriptions in whatever language was active on the first import and never follow a change.
    const options = useMemo<ModelSelectorOption[]>(
        () => [
            {
                value: 'moscow_fp32',
                label: modelLabel(t, 'Moscow', 'fp32'),
                description: t('enhancements.sharpen.models.moscow'),
            },
            { value: 'moscow_fp16', label: modelLabel(t, 'Moscow', 'fp16') },
            {
                value: 'petersburg_fp32',
                label: modelLabel(t, 'St. Petersburg', 'fp32'),
                description: t('enhancements.sharpen.models.petersburg'),
            },
            { value: 'petersburg_fp16', label: modelLabel(t, 'St. Petersburg', 'fp16') },
            {
                value: 'novgorod_fp32',
                label: modelLabel(t, 'Novgorod', 'fp32'),
                description: t('enhancements.sharpen.models.novgorod'),
            },
            { value: 'novgorod_fp16', label: modelLabel(t, 'Novgorod', 'fp16') },
        ],
        [t],
    );

    const { model, amount, onModelChange, onAmountChange } = useOptionEnhancement(
        'sh',
        (op) => (Number(op?.options.intensity) * 100).toString(),
        (nextModel, nextIntensity) => {
            const intensity = nextIntensity !== '' ? parseInt(nextIntensity, 10) / 100 : 1;
            const [name, precision] = nextModel.split('_');

            switch (name) {
                case 'novgorod':
                    return new Novgorod(intensity, precision);
                case 'petersburg':
                    return new Petersburg(intensity, precision);
                default:
                    return new Moscow(intensity, precision);
            }
        },
    );

    return (
        <OptionsPopover title={t('enhancements.sharpen.name')} anchorEl={anchorEl} open={open} onClose={onClose}>
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={model} onChange={onModelChange} />

                <Divider />

                <IntensitySelector
                    value={amount}
                    onChange={onAmountChange}
                    min={0}
                    max={300}
                    marks={[{ value: 100, label: '100' }]}
                />
            </div>
        </OptionsPopover>
    );
};
