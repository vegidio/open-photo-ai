import { ModelSelector, type ModelSelectorOption } from '@/components/molecules/ModelSelector';
import { OptionsPopover } from '@/components/molecules/OptionsPopover';
import { useCurrentFile, useFileOperations } from '@/hooks';
import { Moscow, Novgorod } from '@/operations';
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
            'Use this model for portraits or close-up photos where the background (or foreground) is unintentionally out of focus. It recovers sharpness lost due to a narrow depth of field or an incorrectly set focus point on the camera.',
    },
    { value: 'moscow_fp16', label: 'Moscow Std.' },
    {
        value: 'novgorod_fp32',
        label: 'Novgorod High',
        description:
            'Use this model when blur comes from camera shake or fast-moving subjects: handheld long exposures, action sports, or any scene where something moved during the shot. It reconstructs the sharp image behind the streak.',
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
