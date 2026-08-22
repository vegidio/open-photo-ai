import { Divider } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ModelSelector } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { ScaleSelector } from '@/features/enhancements/ScaleSelector';
import { useModelOptions, useOptionEnhancement } from '@/hooks';
import { buildSelection } from '@/utils/enhancement';

type OptionsUpscaleProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

export const OptionsUpscale = ({ anchorEl, open, onClose }: OptionsUpscaleProps) => {
    const { t } = useTranslation();

    const options = useModelOptions('up');

    const { model, amount, onModelChange, onAmountChange } = useOptionEnhancement(
        'up',
        (nextModel, nextScale) =>
            nextScale === '' ? undefined : buildSelection('up', nextModel, parseFloat(nextScale)),
        (op) => op?.options.scale ?? '1',
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
