import { Divider } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { IntensitySelector } from '@/features/enhancements/IntensitySelector';
import { ModelSelector } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { useModelOptions, useOptionEnhancement } from '@/hooks';
import { buildSelection } from '@/utils/enhancement';

type OptionsLightAdjustmentProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

export const OptionsLightAdjustment = ({ anchorEl, open, onClose }: OptionsLightAdjustmentProps) => {
    const { t } = useTranslation();

    const options = useModelOptions('la');

    const { model, amount, onModelChange, onAmountChange } = useOptionEnhancement(
        'la',
        (nextModel, nextIntensity) =>
            buildSelection(
                'la',
                nextModel,
                nextIntensity !== '' && nextIntensity !== '-' ? parseInt(nextIntensity, 10) / 100 : 0,
            ),
        (op) => (Number(op?.options.intensity) * 100).toString(),
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
