import { useEffect, useState } from 'react';
import { Divider } from '@mui/material';
import { IntensitySelector } from '@/components/molecules/IntensitySelector';
import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
import { OptionsPopover } from '@/components/molecules/OptionsPopover';
import { Rio } from '@/operations';
import { useEnhancementStore, useFileStore } from '@/stores';
import { EMPTY_OPERATIONS } from '@/utils/constants.ts';

type OptionsColorBalanceProps = {
    anchorEl: HTMLElement | null;
    open: boolean;
    onClose: () => void;
};

const options: ModelSelectorOption[] = [
    { value: 'rio_fp32', label: 'Rio High' },
    { value: 'rio_fp16', label: 'Rio Std.' },
];

export const OptionsColorBalance = ({ anchorEl, open, onClose }: OptionsColorBalanceProps) => {
    const file = useFileStore((state) => state.files[state.currentIndex]);
    const operations = useEnhancementStore((state) => state.enhancements.get(file) ?? EMPTY_OPERATIONS);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);

    const currentOp = operations.find((op) => op.id.startsWith('cb'));
    const [model, setModel] = useState(`${currentOp?.options.name}_${currentOp?.options.precision}`);
    const [intensity, setIntensity] = useState((Number(currentOp?.options.intensity) * 100).toString());

    useEffect(() => {
        const numIntensity = intensity !== '' && intensity !== '-' ? parseInt(intensity, 10) / 100 : 0;
        const values = model.split('_');

        switch (values[0]) {
            case 'rio':
                replaceEnhancement(file, new Rio(numIntensity, values[1]));
                break;
        }
    }, [file, intensity, model, replaceEnhancement]);

    return (
        <OptionsPopover title='Color Balance' anchorEl={anchorEl} open={open} onClose={onClose}>
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={model} onChange={setModel} />

                <Divider />

                <IntensitySelector value={intensity} onChange={setIntensity} />
            </div>
        </OptionsPopover>
    );
};
