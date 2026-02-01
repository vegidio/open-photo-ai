import { useMemo } from 'react';
import { List, ListSubheader } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps';
import { ExecutionProvider } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { SettingsItemSelect } from '@/components/molecules/SettingsItemSelect';
import { useSettingsStore } from '@/stores';

export const SettingsList = ({ className = '' }: TailwindProps) => {
    const frModel = useSettingsStore((state) => state.frModel);
    const setFrModel = useSettingsStore((state) => state.setFrModel);
    const laModel = useSettingsStore((state) => state.laModel);
    const setLaModel = useSettingsStore((state) => state.setLaModel);
    const upModel = useSettingsStore((state) => state.upModel);
    const setUpModel = useSettingsStore((state) => state.setUpModel);

    return (
        <List className={`${className} py-0 w-full`}>
            <ListSubheader className='bg-[#2b2b2b] text-[#f2f2f2]'>Application</ListSubheader>

            <ItemAiProcessor />

            <ListSubheader className='bg-[#2b2b2b] text-[#f2f2f2]'>Enhancements</ListSubheader>

            <SettingsItemSelect
                title='Face Recovery'
                description='The default Face Recovery model to use when adding this enhancement.'
                items={[
                    {
                        value: 'athens',
                        label: 'Athens',
                    },
                    {
                        value: 'santorini',
                        label: 'Santorini',
                    },
                ]}
                selected={frModel}
                onSelect={(value) => setFrModel(value)}
            />

            <SettingsItemSelect
                title='Light Adjustment'
                description='The default Light Adjustment model to use when adding this enhancement.'
                items={[
                    {
                        value: 'paris',
                        label: 'Paris',
                    },
                ]}
                selected={laModel}
                onSelect={(value) => setLaModel(value)}
            />

            <SettingsItemSelect
                title='Upscale'
                description='The default Upscale model to use when adding this enhancement.'
                items={[
                    {
                        value: 'tokyo',
                        label: 'Tokyo',
                    },
                    {
                        value: 'kyoto',
                        label: 'Kyoto',
                    },
                    {
                        value: 'saitama',
                        label: 'Saitama',
                    },
                ]}
                selected={upModel}
                onSelect={(value) => setUpModel(value)}
            />
        </List>
    );
};

const ItemAiProcessor = () => {
    const processorSelectItems = useSettingsStore((state) => state.processorSelectItems);
    const executionProvider = useSettingsStore((state) => state.executionProvider);
    const setExecutionProvider = useSettingsStore((state) => state.setExecutionProvider);

    const description = useMemo(() => {
        const base = 'Select the AI processor that will orchestrate the models.';

        switch (executionProvider) {
            case ExecutionProvider.ExecutionProviderAuto:
                return `${base} "Auto" will try to detect the best processor for your system.`;
            case ExecutionProvider.ExecutionProviderTensorRT:
                return `${base} TensorRT is very fast, but on the first run it will take some time to create the model graph; on subsequent runs it will be much faster.`;
            case ExecutionProvider.ExecutionProviderCUDA:
                return `${base} CUDA has strong performance and flexibility, good default if TensorRT is unavailable.`;
            case ExecutionProvider.ExecutionProviderDirectML:
                return `${base} DirectML is a good option when you don't have a dedicated GPU.`;
            case ExecutionProvider.ExecutionProviderCoreML:
                return `${base} CoreML is usually much faster than CPU, but it doesn't support all models.`;
            case ExecutionProvider.ExecutionProviderCPU:
                return `${base} CPU is the slowest option, but it supports all models.`;
            default:
                return base;
        }
    }, [executionProvider]);

    return (
        <SettingsItemSelect
            title='AI Processor'
            description={description}
            items={processorSelectItems}
            selected={executionProvider}
            onSelect={(value) => setExecutionProvider(value as ExecutionProvider)}
        />
    );
};
