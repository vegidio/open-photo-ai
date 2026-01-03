import { ClickAwayListener, Popover } from '@mui/material';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
import { Athens, Santorini } from '@/operations';
import { useEnhancementStore, useFileStore } from '@/stores';
import { EMPTY_OPERATIONS } from '@/utils/constants.ts';

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
    const file = useFileStore((state) => state.files[state.currentIndex]);
    const operations = useEnhancementStore((state) => state.enhancements.get(file) ?? EMPTY_OPERATIONS);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);

    const currentOp = operations.find((op) => op.id.startsWith('fr'));
    if (!currentOp) return null;

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
                    <ModalTitle title='Face Recovery' onClose={onClose} />

                    <div className='mt-1 p-3'>
                        <ModelSelector options={options} value={selectedModel} onChange={onModelChange} />
                    </div>
                </div>
            </ClickAwayListener>
        </Popover>
    );
};
