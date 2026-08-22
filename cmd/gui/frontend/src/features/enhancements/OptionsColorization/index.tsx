import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { ModelSelector, type ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { useCurrentFile, useFileOperations } from '@/hooks';
import { modelLabel } from '@/i18n/format';
import { Delhi, Jaipur, Mumbai } from '@/operations';
import { useEnhancementStore } from '@/stores';

type OptionsColorizationProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

export const OptionsColorization = ({ anchorEl, open, onClose }: OptionsColorizationProps) => {
    const { t } = useTranslation();

    // Built here rather than at module scope: t() called at module-evaluation time would freeze the labels
    // and descriptions in whatever language was active on the first import and never follow a change.
    const options = useMemo<ModelSelectorOption[]>(
        () => [
            {
                value: 'delhi_fp32',
                label: modelLabel(t, 'Delhi', 'fp32'),
                description: t('enhancements.colorization.models.delhi'),
            },
            { value: 'delhi_fp16', label: modelLabel(t, 'Delhi', 'fp16') },
            {
                value: 'mumbai_fp32',
                label: modelLabel(t, 'Mumbai', 'fp32'),
                description: t('enhancements.colorization.models.mumbai'),
            },
            { value: 'mumbai_fp16', label: modelLabel(t, 'Mumbai', 'fp16') },
            {
                value: 'jaipur_fp32',
                label: modelLabel(t, 'Jaipur', 'fp32'),
                description: t('enhancements.colorization.models.jaipur'),
            },
            { value: 'jaipur_fp16', label: modelLabel(t, 'Jaipur', 'fp16') },
        ],
        [t],
    );

    const file = useCurrentFile();
    const operations = useFileOperations(file);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);

    const currentOp = operations.find((op) => op.id.startsWith('cl'));
    if (!file || !currentOp) return undefined;

    const selectedModel = `${currentOp.options.name}_${currentOp.options.precision}`;

    const onModelChange = (value: string) => {
        if (!value) return;
        const values = value.split('_');

        switch (values[0]) {
            case 'delhi':
                replaceEnhancement(file, new Delhi(values[1]));
                break;

            case 'mumbai':
                replaceEnhancement(file, new Mumbai(values[1]));
                break;

            case 'jaipur':
                replaceEnhancement(file, new Jaipur(values[1]));
                break;
        }
    };

    return (
        <OptionsPopover
            title={t('enhancements.colorization.name')}
            anchorEl={anchorEl}
            open={open}
            onClose={onClose}
            hideBackdrop={false}
        >
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={selectedModel} onChange={onModelChange} />
            </div>
        </OptionsPopover>
    );
};
