import { useMemo } from 'react';
import { Divider } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { IntensitySelector } from '@/features/enhancements/IntensitySelector';
import { ModelSelector, type ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { useOptionEnhancement } from '@/hooks';
import { modelLabel } from '@/i18n/format';
import { Gothenburg, Malmo, Stockholm } from '@/operations';

type OptionsDenoiseProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

export const OptionsDenoise = ({ anchorEl, open, onClose }: OptionsDenoiseProps) => {
    const { t } = useTranslation();

    // Built here rather than at module scope: t() called at module-evaluation time would freeze the labels
    // and descriptions in whatever language was active on the first import and never follow a change.
    const options = useMemo<ModelSelectorOption[]>(
        () => [
            {
                value: 'stockholm_fp32',
                label: modelLabel(t, 'Stockholm', 'fp32'),
                description: t('enhancements.denoise.models.stockholm'),
            },
            { value: 'stockholm_fp16', label: modelLabel(t, 'Stockholm', 'fp16') },
            {
                value: 'gothenburg_fp32',
                label: modelLabel(t, 'Gothenburg', 'fp32'),
                description: t('enhancements.denoise.models.gothenburg'),
            },
            { value: 'gothenburg_fp16', label: modelLabel(t, 'Gothenburg', 'fp16') },
            {
                value: 'malmo_fp32',
                label: modelLabel(t, 'Malmö', 'fp32'),
                description: t('enhancements.denoise.models.malmo'),
            },
            { value: 'malmo_fp16', label: modelLabel(t, 'Malmö', 'fp16') },
        ],
        [t],
    );

    const { model, amount, onModelChange, onAmountChange } = useOptionEnhancement(
        'dn',
        (op) => (Number(op?.options.intensity) * 100).toString(),
        (nextModel, nextIntensity) => {
            const intensity = nextIntensity !== '' ? parseInt(nextIntensity, 10) / 100 : 1;
            const [name, precision] = nextModel.split('_');

            switch (name) {
                case 'malmo':
                    return new Malmo(intensity, precision);
                case 'gothenburg':
                    return new Gothenburg(intensity, precision);
                default:
                    return new Stockholm(intensity, precision);
            }
        },
    );

    return (
        <OptionsPopover title={t('enhancements.denoise.name')} anchorEl={anchorEl} open={open} onClose={onClose}>
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
