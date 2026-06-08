import { useEffect, useState } from 'react';
import { Divider } from '@mui/material';
import { IntensitySelector } from '@/components/molecules/IntensitySelector';
import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
import { OptionsPopover } from '@/components/molecules/OptionsPopover';
import { useCurrentFile, useFileOperations } from '@/hooks';
import { Gothenburg, Malmo, Stockholm } from '@/operations';
import { useEnhancementStore } from '@/stores';

type OptionsDenoiseProps = {
    anchorEl: HTMLElement | null;
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
    const file = useCurrentFile();
    const operations = useFileOperations(file);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);

    const currentOp = operations.find((op) => op.id.startsWith('dn'));
    const [model, setModel] = useState(`${currentOp?.options.name}_${currentOp?.options.precision}`);
    const [intensity, setIntensity] = useState((Number(currentOp?.options.intensity) * 100).toString());

    useEffect(() => {
        if (!file) return;

        const numIntensity = intensity !== '' ? parseInt(intensity, 10) / 100 : 1;
        const values = model.split('_');

        switch (values[0]) {
            case 'malmo':
                replaceEnhancement(file, new Malmo(numIntensity, values[1]));
                break;

            case 'gothenburg':
                replaceEnhancement(file, new Gothenburg(numIntensity, values[1]));
                break;

            default:
                replaceEnhancement(file, new Stockholm(numIntensity, values[1]));
                break;
        }
    }, [file, intensity, model, replaceEnhancement]);

    return (
        <OptionsPopover title='Denoise' anchorEl={anchorEl} open={open} onClose={onClose}>
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={model} onChange={setModel} />

                <Divider />

                <IntensitySelector
                    value={intensity}
                    onChange={setIntensity}
                    min={0}
                    max={300}
                    marks={[{ value: 100, label: '100' }]}
                />
            </div>
        </OptionsPopover>
    );
};
