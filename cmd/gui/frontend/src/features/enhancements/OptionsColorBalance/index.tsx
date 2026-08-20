import { useMemo } from 'react';
import { Divider } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { IntensitySelector } from '@/features/enhancements/IntensitySelector';
import { ModelSelector, type ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { useOptionEnhancement } from '@/hooks';
import { modelLabel } from '@/i18n/format';
import { Rio } from '@/operations';

type OptionsColorBalanceProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

export const OptionsColorBalance = ({ anchorEl, open, onClose }: OptionsColorBalanceProps) => {
    const { t } = useTranslation();

    // Built here rather than at module scope: t() called at module-evaluation time would freeze the labels
    // and descriptions in whatever language was active on the first import and never follow a change.
    const options = useMemo<ModelSelectorOption[]>(
        () => [
            {
                value: 'rio_fp32',
                label: modelLabel(t, 'Rio', 'fp32'),
                description: t('enhancements.colorBalance.models.rio'),
            },
            { value: 'rio_fp16', label: modelLabel(t, 'Rio', 'fp16') },
        ],
        [t],
    );

    const { model, amount, onModelChange, onAmountChange } = useOptionEnhancement(
        'cb',
        (op) => (Number(op?.options.intensity) * 100).toString(),
        (nextModel, nextIntensity) => {
            const intensity = nextIntensity !== '' && nextIntensity !== '-' ? parseInt(nextIntensity, 10) / 100 : 0;
            const [name, precision] = nextModel.split('_');

            switch (name) {
                case 'rio':
                    return new Rio(intensity, precision);
            }
        },
    );

    return (
        <OptionsPopover title={t('enhancements.colorBalance.name')} anchorEl={anchorEl} open={open} onClose={onClose}>
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={model} onChange={onModelChange} />

                <Divider />

                <IntensitySelector value={amount} onChange={onAmountChange} />
            </div>
        </OptionsPopover>
    );
};
