import { List } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { Enhancement } from '@/components/Enhancement';

export const SidebarEnhancements = ({ className = '' }: TailwindProps) => {
    return (
        <List className={`${className}`} dense>
            <Enhancement op={{ id: 'face', options: { precision: 'fp32' } }} />

            <Enhancement op={{ id: 'upscale', options: { mode: 'general', scale: '4', precision: 'fp32' } }} />
        </List>
    );
};
