import { Divider, List } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { Enhancement } from '@/components/Enhancement';
import { useEnhancementStore } from '@/stores';

export const SidebarEnhancements = ({ className = '' }: TailwindProps) => {
    const operations = useEnhancementStore((state) => state.operations);

    return (
        <List className={`${className}`} dense>
            {operations.map((op) => (
                <div key={op.id}>
                    <Enhancement op={op} />
                    <Divider variant='middle' className='border-[#363636]' />
                </div>
            ))}
        </List>
    );
};
