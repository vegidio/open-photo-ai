import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
import { OptionsPopover } from '@/components/molecules/OptionsPopover';
import { useCurrentFile, useFileOperations } from '@/hooks';
import { Moscow, Novgorod, Petersburg } from '@/operations';
import { useEnhancementStore } from '@/stores';

type OptionsSharpenProps = {
    anchorEl: HTMLElement | null;
    open: boolean;
    onClose: () => void;
};

const options: ModelSelectorOption[] = [
    {
        value: 'moscow_fp32',
        label: 'Moscow High',
        description:
            'Use this model when blur comes from the camera being out of focus rather than from movement — e.g. portraits with a blurry background or foreground, macro photography gone soft, or any scene where a lens failed to focus on the right plane.',
    },
    { value: 'moscow_fp16', label: 'Moscow Std.' },
    {
        value: 'petersburg_fp32',
        label: 'Petersburg High',
        description:
            "Use this model when you need fast, lightweight motion deblurring and efficiency matters more than squeezing out every last bit of quality. It's well-suited for action footage and handheld camera shake, and it's a solid choice when running on limited hardware.",
    },
    { value: 'petersburg_fp16', label: 'Petersburg Std.' },
    {
        value: 'novgorod_fp32',
        label: 'Novgorod High',
        description:
            'Use this model when blur is caused by camera shake or fast-moving subjects — e.g. sports, handheld shots in low light, or any photo where something moved during exposure. It prioritizes maximum restoration quality over speed; good when results matter most.',
    },
    { value: 'novgorod_fp16', label: 'Novgorod Std.' },
];

export const OptionsSharpen = ({ anchorEl, open, onClose }: OptionsSharpenProps) => {
    const file = useCurrentFile();
    const operations = useFileOperations(file);
    const replaceEnhancement = useEnhancementStore((state) => state.replaceEnhancement);

    const currentOp = operations.find((op) => op.id.startsWith('sh'));
    if (!file || !currentOp) return null;

    const selectedModel = `${currentOp.options.name}_${currentOp.options.precision}`;

    const onModelChange = (value: string) => {
        if (!value) return;
        const values = value.split('_');

        switch (values[0]) {
            case 'novgorod':
                replaceEnhancement(file, new Novgorod(values[1]));
                break;

            case 'petersburg':
                replaceEnhancement(file, new Petersburg(values[1]));
                break;

            default:
                replaceEnhancement(file, new Moscow(values[1]));
                break;
        }
    };

    return (
        <OptionsPopover title='Sharpen' anchorEl={anchorEl} open={open} onClose={onClose}>
            <div className='flex flex-col mt-1 p-3 gap-4'>
                <ModelSelector options={options} value={selectedModel} onChange={onModelChange} />
            </div>
        </OptionsPopover>
    );
};
