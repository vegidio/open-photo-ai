import { ClickAwayListener, Popover } from '@mui/material';
import { ModalTitle } from '@/components/molecules/ModalTitle';
import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
import { Kyoto, Tokyo } from '@/operations';
import { useOptionsUpscaleStore } from '@/stores';

type OptionsUpscaleProps = {
    anchorEl: HTMLElement | null;
    open: boolean;
    onMenuClose: () => void;
};

const options: ModelSelectorOption[] = [
    { value: 'tokyo_fp32', label: 'Tokyo High' },
    { value: 'tokyo_fp16', label: 'Tokyo Std.' },
    { value: 'kyoto_fp32', label: 'Kyoto High' },
    { value: 'kyoto_fp16', label: 'Kyoto Std.' },
];

export const OptionsUpscale = ({ anchorEl, open, onMenuClose }: OptionsUpscaleProps) => {
    const model = useOptionsUpscaleStore((state) => state.model);
    const setModel = useOptionsUpscaleStore((state) => state.setModel);

    const selectedModel = `${model.options.name}_${model.options.precision}`;

    const onModelChange = (value: string) => {
        const values = value.split('_');

        switch (values[0]) {
            case 'tokyo':
                setModel(new Tokyo(4, values[1]));
                break;

            case 'kyoto':
                setModel(new Kyoto('general', 4, values[1]));
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
                    <ModalTitle title='Upscale' onClose={onMenuClose} />

                    <div className='mt-1 p-3'>
                        <ModelSelector options={options} value={selectedModel} onChange={onModelChange} />
                    </div>
                </div>
            </ClickAwayListener>
        </Popover>
    );
};
