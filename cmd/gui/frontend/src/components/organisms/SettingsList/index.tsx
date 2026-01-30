import { List, ListSubheader } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps';
import { SettingsItemSelect } from '@/components/molecules/SettingsItemSelect';
import { useSettingsStore } from '@/stores';

export const SettingsList = ({ className = '' }: TailwindProps) => {
    const processor = useSettingsStore((state) => state.processor);
    const setProcessor = useSettingsStore((state) => state.setProcessor);
    const frModel = useSettingsStore((state) => state.frModel);
    const setFrModel = useSettingsStore((state) => state.setFrModel);
    const laModel = useSettingsStore((state) => state.laModel);
    const setLaModel = useSettingsStore((state) => state.setLaModel);
    const upModel = useSettingsStore((state) => state.upModel);
    const setUpModel = useSettingsStore((state) => state.setUpModel);

    return (
        <List className={`${className} py-0 w-full`}>
            <ListSubheader className='bg-[#2b2b2b] text-[#f2f2f2]'>Application</ListSubheader>

            <SettingsItemSelect
                title='AI Processor'
                description='Select the AI processor to use for the enhancements.'
                items={[
                    {
                        value: 'tensorrt',
                        label: 'TensorRT',
                    },
                    {
                        value: 'cuda',
                        label: 'CUDA',
                    },
                    {
                        value: 'cpu',
                        label: 'CPU',
                    },
                ]}
                selected={processor}
                onSelect={(value) => setProcessor(value)}
            />

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
