import { Divider } from '@mui/material';
import { IntensitySelector } from '@/features/enhancements/IntensitySelector';
import { ModelSelector, type ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { useOptionEnhancement } from '@/hooks';
import { Rio } from '@/operations';

type OptionsColorBalanceProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

const options: ModelSelectorOption[] = [
    {
        value: 'rio_fp32',
        label: 'Rio High',
        description:
            "Use this model when your photos look too orange, too blue, or just have an off, unnatural tint, like indoor shots under warm lamps, cloudy outdoor scenes, or pictures taken in mixed lighting conditions where the colors simply don't look natural.",
    },
    { value: 'rio_fp16', label: 'Rio Std.' },
];

export const OptionsColorBalance = ({ anchorEl, open, onClose }: OptionsColorBalanceProps) => {
    const { model, amount, onModelChange, onAmountChange } = useOptionEnhancement(
        'cb',
        (op) => (Number(op?.options.intensity) * 100).toString(),
        (nextModel, nextIntensity) => {
            const intensity = nextIntensity !== '' && nextIntensity !== '-' ? parseInt(nextIntensity, 10) / 100 : 0;
            const [name, precision] = nextModel.split('_');

            switch (name) {
                case 'rio':
                    return new Rio(intensity, precision);
            }
        },
    );

    return (
        <OptionsPopover title='Color Balance' anchorEl={anchorEl} open={open} onClose={onClose}>
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={model} onChange={onModelChange} />

                <Divider />

                <IntensitySelector value={amount} onChange={onAmountChange} />
            </div>
        </OptionsPopover>
    );
};
