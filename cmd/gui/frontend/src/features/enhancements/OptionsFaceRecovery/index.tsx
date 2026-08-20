import { useMemo, useState } from 'react';
import { Divider } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { FaceSelector } from '@/features/enhancements/FaceSelector';
import { ModelSelector, type ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { FaceToggle } from '@/features/faces';
import { useCurrentFile, useFileDisabledFaces, useFileFaces, useFileOperations } from '@/hooks';
import { modelLabel } from '@/i18n/format';
import { Athens, Santorini } from '@/operations';
import { useEnhancementStore } from '@/stores';

type OptionsFaceRecoveryProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

export const OptionsFaceRecovery = ({ anchorEl, open, onClose }: OptionsFaceRecoveryProps) => {
    const { t } = useTranslation();

    // Built here rather than at module scope: t() called at module-evaluation time would freeze the labels
    // and descriptions in whatever language was active on the first import and never follow a change.
    const options = useMemo<ModelSelectorOption[]>(
        () => [
            {
                value: 'athens_fp32',
                label: modelLabel(t, 'Athens', 'fp32'),
                description: t('enhancements.faceRecovery.models.athens'),
            },
            { value: 'athens_fp16', label: modelLabel(t, 'Athens', 'fp16') },
            {
                value: 'santorini_fp32',
                label: modelLabel(t, 'Santorini', 'fp32'),
                description: t('enhancements.faceRecovery.models.santorini'),
            },
            { value: 'santorini_fp16', label: modelLabel(t, 'Santorini', 'fp16') },
        ],
        [t],
    );

    const file = useCurrentFile();
    const operations = useFileOperations(file);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);
    const selectedCount = useFileFaces(file).length - useFileDisabledFaces(file).size;
    const [facesOpen, setFacesOpen] = useState(false);

    const currentOp = operations.find((op) => op.id.startsWith('fr'));
    if (!file || !currentOp) return undefined;

    const selectedModel = `${currentOp.options.name}_${currentOp.options.precision}`;

    const onModelChange = (value: string) => {
        if (!value) return;
        const values = value.split('_');

        switch (values[0]) {
            case 'athens':
                replaceEnhancement(file, new Athens(values[1]));
                break;

            case 'santorini':
                replaceEnhancement(file, new Santorini(values[1]));
                break;
        }
    };

    return (
        <OptionsPopover
            title={t('enhancements.faceRecovery.name')}
            anchorEl={anchorEl}
            open={open}
            onClose={onClose}
            hideBackdrop={false}
        >
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={selectedModel} onChange={onModelChange} />

                <Divider />

                <FaceSelector selectedCount={selectedCount} onClick={() => setFacesOpen(true)} />
            </div>

            <FaceToggle file={file} open={facesOpen} onClose={() => setFacesOpen(false)} />
        </OptionsPopover>
    );
};
