import { Divider } from '@mui/material';
import { ModelSelector, type ModelSelectorOption } from '@/features/enhancements/ModelSelector';
import { OptionsPopover } from '@/features/enhancements/OptionsPopover';
import { ScaleSelector } from '@/features/enhancements/ScaleSelector';
import { useOptionEnhancement } from '@/hooks';
import { Kyoto, Osaka, Saitama, Tokyo } from '@/operations';

type OptionsUpscaleProps = {
    anchorEl: HTMLElement | undefined;
    open: boolean;
    onClose: () => void;
};

const options: ModelSelectorOption[] = [
    {
        value: 'tokyo_fp32',
        label: 'Tokyo High',
        description:
            'Use this model when you want a natural upscale without exaggeration. It focuses on preserving the original look and fine structures instead of "inventing" new details, making it ideal when realism and faithfulness matter more than sharpness.',
    },
    { value: 'tokyo_fp16', label: 'Tokyo Std.' },
    {
        value: 'kyoto_fp32',
        label: 'Kyoto High',
        description:
            'Use this model for real-world photos (people, landscapes, products). It excels at restoring details while handling noise, blur, and compression artifacts. Ideal for practical applications where images are imperfect, and you want visually pleasing, robust results fast.',
    },
    { value: 'kyoto_fp16', label: 'Kyoto Std.' },
    {
        value: 'saitama_fp32',
        label: 'Saitama High',
        description:
            'Use this model for cartoon, drawings, line art, and digital illustrations. It preserves clean lines, flat colors, and stylized shading without introducing photo-like textures. Best when sharp edges and stylistic consistency matter more than realism.',
    },
    { value: 'saitama_fp16', label: 'Saitama Std.' },
    // Osaka is published only as fp16, so the High slot is shown for consistency with the other models but cannot be
    // selected. The description sits on the Std. entry rather than the High one, because a disabled MUI button gets
    // `pointer-events: none` and its tooltip would never open.
    { value: 'osaka_fp32', label: 'Osaka High', disabled: true },
    {
        value: 'osaka_fp16',
        label: 'Osaka Std.',
        description:
            'A diffusion model that rebuilds detail instead of interpolating it, giving the most dramatic results on soft, small, or heavily compressed photos. It is in a different league for cost: a 7.3 GB download on first use, roughly 8 GB of GPU memory, and far slower than the other models — minutes rather than seconds on a good GPU, and impractically slow without one. Only a half-precision build is published, so "High" is unavailable.',
    },
];

export const OptionsUpscale = ({ anchorEl, open, onClose }: OptionsUpscaleProps) => {
    const { model, amount, onModelChange, onAmountChange } = useOptionEnhancement(
        'up',
        (op) => op?.options.scale ?? '1',
        (nextModel, nextScale) => {
            if (nextScale === '') return;
            const scale = parseFloat(nextScale);
            const [name, precision] = nextModel.split('_');

            switch (name) {
                case 'tokyo':
                    return new Tokyo(scale, precision);
                case 'kyoto':
                    return new Kyoto(scale, precision);
                case 'saitama':
                    return new Saitama(scale, precision);
                case 'osaka':
                    return new Osaka(scale, precision);
            }
        },
    );

    return (
        <OptionsPopover title='Upscale' anchorEl={anchorEl} open={open} onClose={onClose}>
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={model} onChange={onModelChange} />

                <Divider />

                <ScaleSelector value={amount} onChange={onAmountChange} />
            </div>
        </OptionsPopover>
    );
};
