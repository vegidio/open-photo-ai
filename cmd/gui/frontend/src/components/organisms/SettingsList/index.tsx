import { forwardRef, useImperativeHandle, useMemo, useRef } from 'react';
import { List, ListSubheader } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps';
import { ExecutionProvider } from '@/bindings/github.com/vegidio/open-photo-ai/types';
import { SettingsItemSelect } from '@/components/molecules/SettingsItemSelect';
import { useSettingsStore } from '@/stores';
import { os } from '@/utils/constants';

export type SettingsListHandle = {
    scrollToSection: (itemId: string) => void;
};

export const SettingsList = forwardRef<SettingsListHandle, TailwindProps>(({ className = '' }, ref) => {
    const containerRef = useRef<HTMLUListElement>(null);

    const frModel = useSettingsStore((state) => state.frModel);
    const setFrModel = useSettingsStore((state) => state.setFrModel);
    const laModel = useSettingsStore((state) => state.laModel);
    const setLaModel = useSettingsStore((state) => state.setLaModel);
    const cbModel = useSettingsStore((state) => state.cbModel);
    const setCbModel = useSettingsStore((state) => state.setCbModel);
    const upModel = useSettingsStore((state) => state.upModel);
    const setUpModel = useSettingsStore((state) => state.setUpModel);

    useImperativeHandle(ref, () => ({
        scrollToSection: (itemId: string) => {
            const target = containerRef.current?.querySelector(`#${CSS.escape(itemId)}`);
            target?.scrollIntoView({ behavior: 'smooth', block: 'start' });
        },
    }));

    return (
        <List ref={containerRef} className={`${className} py-0 w-full`}>
            <ListSubheader id='app' className='bg-[#2b2b2b] text-[#f2f2f2]'>
                Application
            </ListSubheader>

            <ItemAiProcessor id='app_processor' />

            <ListSubheader id='enhancements' className='bg-[#2b2b2b] text-[#f2f2f2]'>
                Enhancements
            </ListSubheader>

            <SettingsItemSelect
                id='enh_face'
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
                id='enh_light'
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
                id='enh_color'
                title='Color Balance'
                description='The default Color Balance model to use when adding this enhancement.'
                items={[
                    {
                        value: 'rio',
                        label: 'Rio',
                    },
                ]}
                selected={cbModel}
                onSelect={(value) => setCbModel(value)}
            />

            <SettingsItemSelect
                id='enh_upscale'
                title='Upscale'
                description='The default Upscale model to use when adding this enhancement.'
                items={[
                    {
                        value: 'tokyo',
                        label: 'Tokyo',
                        disabled: os === 'darwin',
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
});

type ItemAiProcessorProps = {
    id?: string;
};

const ItemAiProcessor = ({ id }: ItemAiProcessorProps) => {
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
            id={id}
            title='AI Processor'
            description={description}
            items={processorSelectItems}
            selected={executionProvider}
            onSelect={(value) => setExecutionProvider(value as ExecutionProvider)}
        />
    );
};
