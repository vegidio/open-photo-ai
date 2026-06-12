import { useState } from 'react';
import { Divider } from '@mui/material';
import { FaceSelector } from '@/features/enhancements/FaceSelector';
import { ModelSelector, type ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { FaceToggle } from '@/features/faces';
import { useCurrentFile, useFileDisabledFaces, useFileFaces, useFileOperations } from '@/hooks';
import { Athens, Santorini } from '@/operations';
import { useEnhancementStore } from '@/stores';

type OptionsFaceRecoveryProps = {
    anchorEl: HTMLElement | null;
    open: boolean;
    onClose: () => void;
};

const options: ModelSelectorOption[] = [
    {
        value: 'athens_fp32',
        label: 'Athens High',
        description:
            'Use this model when identity fidelity matters most. It lets you preserve facial structure while restoring details, even on heavily degraded faces. Best when you want restoration without changing the person.',
    },
    { value: 'athens_fp16', label: 'Athens Std.' },
    {
        value: 'santorini_fp32',
        label: 'Santorini High',
        description:
            'Use this model when you want aggressive, fast enhancement and can tolerate identity drift. It produces sharp, visually pleasing faces on moderate degradation, but may hallucinate features and alter identity on very low-quality inputs.',
    },
    { value: 'santorini_fp16', label: 'Santorini Std.' },
];

export const OptionsFaceRecovery = ({ anchorEl, open, onClose }: OptionsFaceRecoveryProps) => {
    const file = useCurrentFile();
    const operations = useFileOperations(file);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);
    const selectedCount = useFileFaces(file).length - useFileDisabledFaces(file).size;
    const [facesOpen, setFacesOpen] = useState(false);

    const currentOp = operations.find((op) => op.id.startsWith('fr'));
    if (!file || !currentOp) return null;

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
        <OptionsPopover title='Face Recovery' anchorEl={anchorEl} open={open} onClose={onClose} hideBackdrop={false}>
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={selectedModel} onChange={onModelChange} />

                <Divider />

                <FaceSelector selectedCount={selectedCount} onClick={() => setFacesOpen(true)} />
            </div>

            <FaceToggle file={file} open={facesOpen} onClose={() => setFacesOpen(false)} />
        </OptionsPopover>
    );
};
