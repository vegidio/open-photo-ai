import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
import { OptionsPopover } from '@/components/molecules/OptionsPopover';
import { useCurrentFile, useFileOperations } from '@/hooks';
import { Gothenburg, Malmo, Stockholm, Uppsala } from '@/operations';
import { useEnhancementStore } from '@/stores';

type OptionsDenoiseProps = {
    anchorEl: HTMLElement | null;
    open: boolean;
    onClose: () => void;
};

const options: ModelSelectorOption[] = [
    { value: 'stockholm_fp32', label: 'Stockholm High' },
    { value: 'stockholm_fp16', label: 'Stockholm Std.' },
    { value: 'gothenburg_fp32', label: 'Gothenburg Hg.' },
    { value: 'gothenburg_fp16', label: 'Gothenburg Std.' },
    { value: 'malmo_fp32', label: 'Malmö High' },
    { value: 'malmo_fp16', label: 'Malmö Std.' },
    { value: 'uppsala_fp32', label: 'Uppsala High' },
    { value: 'uppsala_fp16', label: 'Uppsala Std.' },
];

export const OptionsDenoise = ({ anchorEl, open, onClose }: OptionsDenoiseProps) => {
    const file = useCurrentFile();
    const operations = useFileOperations(file);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);

    const currentOp = operations.find((op) => op.id.startsWith('dn'));
    if (!file || !currentOp) return null;

    const selectedModel = `${currentOp.options.name}_${currentOp.options.precision}`;

    const onModelChange = (value: string) => {
        if (!value) return;
        const values = value.split('_');

        switch (values[0]) {
            case 'gothenburg':
                replaceEnhancement(file, new Gothenburg(values[1]));
                break;

            case 'malmo':
                replaceEnhancement(file, new Malmo(values[1]));
                break;

            case 'uppsala':
                replaceEnhancement(file, new Uppsala(values[1]));
                break;

            default:
                replaceEnhancement(file, new Stockholm(values[1]));
                break;
        }
    };

    return (
        <OptionsPopover title='Denoise' anchorEl={anchorEl} open={open} onClose={onClose}>
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={selectedModel} onChange={onModelChange} />
            </div>
        </OptionsPopover>
    );
};
