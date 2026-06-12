import { Divider } from '@mui/material';
import { IntensitySelector } from '@/features/enhancements/IntensitySelector';
import { ModelSelector, type ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { useOptionEnhancement } from '@/hooks';
import { Paris } from '@/operations';

type OptionsLightAdjustmentProps = {
    anchorEl: HTMLElement | null;
    open: boolean;
    onClose: () => void;
};

const options: ModelSelectorOption[] = [
    {
        value: 'paris_fp32',
        label: 'Paris High',
        description:
            "Use this model when working with images affected by poor or uneven lighting, such as night scenes, backlit photos, shadows, or overexposed areas. It's useful when you need to enhance visibility and contrast so that images look clearer.",
    },
    { value: 'paris_fp16', label: 'Paris Std.' },
];

export const OptionsLightAdjustment = ({ anchorEl, open, onClose }: OptionsLightAdjustmentProps) => {
    const { model, amount, onModelChange, onAmountChange } = useOptionEnhancement(
        'la',
        (op) => (Number(op?.options.intensity) * 100).toString(),
        (nextModel, nextIntensity) => {
            const intensity = nextIntensity !== '' && nextIntensity !== '-' ? parseInt(nextIntensity, 10) / 100 : 0;
            const [name, precision] = nextModel.split('_');

            switch (name) {
                case 'paris':
                    return new Paris(intensity, precision);
            }
        },
    );

    return (
        <OptionsPopover title='Light Adjustment' anchorEl={anchorEl} open={open} onClose={onClose}>
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={model} onChange={onModelChange} />

                <Divider />

                <IntensitySelector value={amount} onChange={onAmountChange} />
            </div>
        </OptionsPopover>
    );
};
