import { SimpleTreeView, TreeItem } from '@mui/x-tree-view';
import type { TailwindProps } from '@/utils/TailwindProps';

export const SettingsMenu = ({ className = '' }: TailwindProps) => {
    return (
        <SimpleTreeView
            className={`${className}`}
            expandedItems={['app', 'enhancements']}
            sx={{
                '& .MuiTreeItem-label': {
                    fontSize: '0.875rem', // text-sm equivalent
                    color: '#b0b0b0',
                },
            }}
        >
            <TreeItem
                itemId='app'
                label='Application'
                slotProps={{
                    label: {
                        className: 'font-bold text-[#f2f2f2]',
                    },
                }}
            >
                <TreeItem itemId='app_processor' label='AI Processor' />
            </TreeItem>

            <TreeItem
                itemId='enhancements'
                label='Enhancements'
                slotProps={{
                    label: {
                        className: 'font-bold text-[#f2f2f2]',
                    },
                }}
            >
                <TreeItem itemId='enh_face' label='Face Recovery' />
                <TreeItem itemId='enh_light' label='Light Adjustment' />
                <TreeItem itemId='enh_upscale' label='Upscale' />
            </TreeItem>
        </SimpleTreeView>
    );
};
