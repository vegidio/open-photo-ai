import { useTranslation } from 'react-i18next';
import { ModelSelector } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { useModelOptions, useOptionEnhancement } from '@/hooks';
import { buildSelection } from '@/utils/enhancement';

type OptionsColorizationProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

export const OptionsColorization = ({ anchorEl, open, onClose }: OptionsColorizationProps) => {
    const { t } = useTranslation();

    const options = useModelOptions('cl');

    // Colorization has no per-run amount, so only the model half of the hook is used.
    const { model, onModelChange } = useOptionEnhancement('cl', (nextModel) => buildSelection('cl', nextModel));

    return (
        <OptionsPopover
            title={t('enhancements.colorization.name')}
            anchorEl={anchorEl}
            open={open}
            onClose={onClose}
            hideBackdrop={false}
        >
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={model} onChange={onModelChange} />
            </div>
        </OptionsPopover>
    );
};
