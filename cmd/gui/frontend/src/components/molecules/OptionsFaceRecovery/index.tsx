import { ClickAwayListener, Popover } from '@mui/material';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
import { Athens, Santorini } from '@/operations';
import { useOptionsFaceRecoveryStore } from '@/stores';

type OptionsFaceRecoveryProps = {
    anchorEl: HTMLElement | null;
    open: boolean;
    onMenuClose: () => void;
};

const options: ModelSelectorOption[] = [
    { value: 'athens_fp32', label: 'Athens High' },
    { value: 'athens_fp16', label: 'Athens Std.' },
    { value: 'santorini_fp32', label: 'Santorini High' },
    { value: 'santorini_fp16', label: 'Santorini Std.' },
];

export const OptionsFaceRecovery = ({ anchorEl, open, onMenuClose }: OptionsFaceRecoveryProps) => {
    const model = useOptionsFaceRecoveryStore((state) => state.model);
    const setModel = useOptionsFaceRecoveryStore((state) => state.setModel);

    const selectedModel = `${model.options.name}_${model.options.precision}`;

    const onModelChange = (value: string) => {
        const values = value.split('_');

        switch (values[0]) {
            case 'athens':
                setModel(new Athens(values[1]));
                break;

            case 'santorini':
                setModel(new Santorini(values[1]));
                break;
        }
    };

    return (
        <Popover
            anchorEl={anchorEl}
            open={open}
            onClose={onMenuClose}
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
            <ClickAwayListener onClickAway={onMenuClose}>
                <div className='flex flex-col'>
                    <ModalTitle title='Face Recovery' onClose={onMenuClose} />

                    <div className='mt-1 p-3'>
                        <ModelSelector options={options} value={selectedModel} onChange={onModelChange} />
                    </div>
                </div>
            </ClickAwayListener>
        </Popover>
    );
};
