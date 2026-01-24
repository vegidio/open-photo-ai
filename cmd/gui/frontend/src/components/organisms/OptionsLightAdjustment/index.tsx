import { useEffect, useState } from 'react';
import { ClickAwayListener, Divider, Popover } from '@mui/material';
import { IntensitySelector } from '@/components/molecules/IntensitySelector';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
import { Paris } from '@/operations';
import { useEnhancementStore, useFileStore } from '@/stores';
import { EMPTY_OPERATIONS } from '@/utils/constants.ts';

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
    const file = useFileStore((state) => state.files[state.currentIndex]);
    const operations = useEnhancementStore((state) => state.enhancements.get(file) ?? EMPTY_OPERATIONS);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);

    const currentOp = operations.find((op) => op.id.startsWith('la'));
    const [model, setModel] = useState(`${currentOp?.options.name}_${currentOp?.options.precision}`);
    const [intensity, setIntensity] = useState((Number(currentOp?.options.intensity) * 100).toString());

    useEffect(() => {
        const numIntensity = intensity !== '' && intensity !== '-' ? parseInt(intensity, 10) / 100 : 0;
        const values = model.split('_');

        switch (values[0]) {
            case 'paris':
                replaceEnhancement(file, new Paris(numIntensity, values[1]));
                break;
        }
    }, [file, intensity, model, replaceEnhancement]);

    return (
        <Popover
            anchorEl={anchorEl}
            open={open}
            onClose={onClose}
            hideBackdrop={true}
            anchorOrigin={{
                vertical: 'center',
                horizontal: 'left',
            }}
            transformOrigin={{
                vertical: 'top',
                horizontal: 'right',
            }}
            className='pointer-events-none'
            slotProps={{
                paper: {
                    className: 'w-64 -ml-4 pointer-events-auto',
                },
            }}
        >
            <ClickAwayListener onClickAway={onClose}>
                <div className='flex flex-col'>
                    <ModalTitle title='Light Adjustment' onClose={onClose} />

                    <div className='flex flex-col mt-1 p-3 gap-4'>
                        <ModelSelector options={options} value={model} onChange={setModel} />

                        <Divider />

                        <IntensitySelector value={intensity} onChange={setIntensity} />
                    </div>
                </div>
            </ClickAwayListener>
        </Popover>
    );
};
