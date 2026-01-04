import { useEffect, useState } from 'react';
import { ClickAwayListener, Divider, Popover } from '@mui/material';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
import { ScaleSelector } from '@/components/molecules/ScaleSelector';
import { Kyoto, Tokyo } from '@/operations';
import { useEnhancementStore, useFileStore } from '@/stores';
import { EMPTY_OPERATIONS, os } from '@/utils/constants.ts';

type OptionsUpscaleProps = {
    anchorEl: HTMLElement | null;
    open: boolean;
    onClose: () => void;
};

const options: ModelSelectorOption[] = [
    { value: 'tokyo_fp32', label: 'Tokyo High', disabled: os === 'darwin' },
    { value: 'tokyo_fp16', label: 'Tokyo Std.', disabled: os === 'darwin' },
    { value: 'kyoto_fp32', label: 'Kyoto High' },
    { value: 'kyoto_fp16', label: 'Kyoto Std.' },
];

export const OptionsUpscale = ({ anchorEl, open, onClose }: OptionsUpscaleProps) => {
    const file = useFileStore((state) => state.files[state.currentIndex]);
    const operations = useEnhancementStore((state) => state.enhancements.get(file) ?? EMPTY_OPERATIONS);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);

    const currentOp = operations.find((op) => op.id.startsWith('up'));
    const [model, setModel] = useState(`${currentOp?.options.name}_${currentOp?.options.precision}`);
    const [scale, setScale] = useState(parseInt(currentOp?.options.scale ?? '1', 10));

    useEffect(() => {
        console.log(model, scale);
        const values = model.split('_');

        switch (values[0]) {
            case 'tokyo':
                replaceEnhancement(file, new Tokyo(scale, values[1]));
                break;

            case 'kyoto':
                replaceEnhancement(file, new Kyoto('general', scale, values[1]));
                break;
        }
    }, [file, replaceEnhancement, model, scale]);

    return (
        <Popover
            anchorEl={anchorEl}
            open={open}
            onClose={onClose}
            anchorOrigin={{
                vertical: 'center',
                horizontal: 'left',
            }}
            transformOrigin={{
                vertical: 'top',
                horizontal: 'right',
            }}
            slotProps={{
                paper: {
                    className: 'w-64 -ml-4',
                },
                root: {
                    // className: 'pointer-events-none',
                },
            }}
        >
            <ClickAwayListener onClickAway={onClose}>
                <div className='flex flex-col'>
                    <ModalTitle title='Upscale' onClose={onClose} />

                    <div className='flex flex-col mt-1 p-3 gap-4'>
                        <ModelSelector options={options} value={model} onChange={setModel} />

                        <Divider />

                        <ScaleSelector value={scale} onChange={setScale} />
                    </div>
                </div>
            </ClickAwayListener>
        </Popover>
    );
};
