import { Divider } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { IntensitySelector } from '@/features/enhancements/IntensitySelector';
import { ModelSelector } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { useModelOptions, useOptionEnhancement } from '@/hooks';
import { buildSelection } from '@/utils/enhancement';

type OptionsDenoiseProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

export const OptionsDenoise = ({ anchorEl, open, onClose }: OptionsDenoiseProps) => {
    const { t } = useTranslation();

    const options = useModelOptions('dn');

    const { model, amount, onModelChange, onAmountChange } = useOptionEnhancement(
        'dn',
        (nextModel, nextIntensity) =>
            buildSelection('dn', nextModel, nextIntensity !== '' ? parseInt(nextIntensity, 10) / 100 : 1),
        (op) => (Number(op?.options.intensity) * 100).toString(),
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
