import { useState } from 'react';
import { Divider } from '@mui/material';
import { FaceSelector } from '@/components/molecules/FaceSelector';
import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
import { OptionsPopover } from '@/components/molecules/OptionsPopover';
import { FaceToggle } from '@/components/templates/FaceToggle';
import { useCurrentFile, useFileOperations } from '@/hooks';
import { Athens, Santorini } from '@/operations';
import { useEnhancementStore } from '@/stores';

type OptionsFaceRecoveryProps = {
    anchorEl: HTMLElement | null;
    open: boolean;
    onClose: () => void;
};

const options: ModelSelectorOption[] = [
    { value: 'athens_fp32', label: 'Athens High' },
    { value: 'athens_fp16', label: 'Athens Std.' },
    { value: 'santorini_fp32', label: 'Santorini High' },
    { value: 'santorini_fp16', label: 'Santorini Std.' },
];

export const OptionsFaceRecovery = ({ anchorEl, open, onClose }: OptionsFaceRecoveryProps) => {
    const file = useCurrentFile();
    const operations = useFileOperations(file);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);
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

                <FaceSelector onClick={() => setFacesOpen(true)} />
            </div>

            <FaceToggle open={facesOpen} onClose={() => setFacesOpen(false)} />
        </OptionsPopover>
    );
};
