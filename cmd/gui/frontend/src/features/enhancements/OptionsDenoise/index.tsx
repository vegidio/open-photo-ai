import { Divider } from '@mui/material';
import { IntensitySelector } from '@/features/enhancements/IntensitySelector';
import { ModelSelector, type ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { useOptionEnhancement } from '@/hooks';
import { Gothenburg, Malmo, Stockholm } from '@/operations';

type OptionsDenoiseProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

const options: ModelSelectorOption[] = [
    {
        value: 'stockholm_fp32',
        label: 'Stockholm High',
        description:
            "Use this model when you need fast, high-quality denoising of real sensor noise and computational efficiency matters. It's a good choice when throughput and resource constraints are real concerns, keeping inference times low without sacrificing quality.",
    },
    { value: 'stockholm_fp16', label: 'Stockholm Std.' },
    {
        value: 'gothenburg_fp32',
        label: 'Gothenburg High',
        description:
            'Use this model when your photos contain real-world sensor noise, the kind produced by shooting in low light or at high ISO with a smartphone or DSLR. It handles complex noise patterns that cameras produce, making it the right choice for photography.',
    },
    { value: 'gothenburg_fp16', label: 'Gothenburg Std.' },
    {
        value: 'malmo_fp32',
        label: 'Malmö High',
        description:
            'Use this model to remove rain streaks from outdoor images, whether captured in light drizzle or heavy downpour. It handles rain of varying scale, density, and direction, restoring fine details behind streaks. A good choice when weather artifacts obscure the scene.',
    },
    { value: 'malmo_fp16', label: 'Malmö Std.' },
];

export const OptionsDenoise = ({ anchorEl, open, onClose }: OptionsDenoiseProps) => {
    const { model, amount, onModelChange, onAmountChange } = useOptionEnhancement(
        'dn',
        (op) => (Number(op?.options.intensity) * 100).toString(),
        (nextModel, nextIntensity) => {
            const intensity = nextIntensity !== '' ? parseInt(nextIntensity, 10) / 100 : 1;
            const [name, precision] = nextModel.split('_');

            switch (name) {
                case 'malmo':
                    return new Malmo(intensity, precision);
                case 'gothenburg':
                    return new Gothenburg(intensity, precision);
                default:
                    return new Stockholm(intensity, precision);
            }
        },
    );

    return (
        <OptionsPopover title='Denoise' anchorEl={anchorEl} open={open} onClose={onClose}>
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={model} onChange={onModelChange} />

                <Divider />

                <IntensitySelector
                    value={amount}
                    onChange={onAmountChange}
                    min={0}
                    max={300}
                    marks={[{ value: 100, label: '100' }]}
                />
            </div>
        </OptionsPopover>
    );
};
