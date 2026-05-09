import { useEffect, useState } from 'react';
import { Divider } from '@mui/material';
import { IntensitySelector } from '@/components/molecules/IntensitySelector';
import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
import { OptionsPopover } from '@/components/molecules/OptionsPopover';
import { useCurrentFile, useFileOperations } from '@/hooks';
import { Paris } from '@/operations';
import { useEnhancementStore } from '@/stores';

type OptionsLightAdjustmentProps = {
    anchorEl: HTMLElement | null;
    open: boolean;
    onClose: () => void;
};

const options: ModelSelectorOption[] = [
    { value: 'paris_fp32', label: 'Paris High' },
    { value: 'paris_fp16', label: 'Paris Std.' },
];

export const OptionsLightAdjustment = ({ anchorEl, open, onClose }: OptionsLightAdjustmentProps) => {
    const file = useCurrentFile();
    const operations = useFileOperations(file);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);

    const currentOp = operations.find((op) => op.id.startsWith('la'));
    const [model, setModel] = useState(`${currentOp?.options.name}_${currentOp?.options.precision}`);
    const [intensity, setIntensity] = useState((Number(currentOp?.options.intensity) * 100).toString());

    useEffect(() => {
        if (!file) return;

        const numIntensity = intensity !== '' && intensity !== '-' ? parseInt(intensity, 10) / 100 : 0;
        const values = model.split('_');

        switch (values[0]) {
            case 'paris':
                replaceEnhancement(file, new Paris(numIntensity, values[1]));
                break;
        }
    }, [file, intensity, model, replaceEnhancement]);

    return (
        <OptionsPopover title='Light Adjustment' anchorEl={anchorEl} open={open} onClose={onClose}>
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={model} onChange={setModel} />

                <Divider />

                <IntensitySelector value={intensity} onChange={setIntensity} />
            </div>
        </OptionsPopover>
    );
};
