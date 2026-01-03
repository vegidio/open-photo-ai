import { ClickAwayListener, Popover } from '@mui/material';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
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
    if (!currentOp) return null;

    const selectedModel = `${currentOp.options.name}_${currentOp.options.precision}`;

    const onModelChange = (value: string) => {
        const values = value.split('_');

        switch (values[0]) {
            case 'tokyo':
                replaceEnhancement(file, new Tokyo(4, values[1]));
                break;

            case 'kyoto':
                replaceEnhancement(file, new Kyoto('general', 4, values[1]));
                break;
        }
    };

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

                    <div className='mt-1 p-3'>
                        <ModelSelector options={options} value={selectedModel} onChange={onModelChange} />
                    </div>
                </div>
            </ClickAwayListener>
        </Popover>
    );
};
