import { Divider } from '@mui/material';
import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
import { OptionsPopover } from '@/components/molecules/OptionsPopover';
import { ScaleSelector } from '@/components/molecules/ScaleSelector';
import { useOptionEnhancement } from '@/hooks';
import { Kyoto, Saitama, Tokyo } from '@/operations';

type OptionsUpscaleProps = {
    anchorEl: HTMLElement | null;
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
