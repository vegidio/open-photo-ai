import { Divider, List } from '@mui/material';
import type { Operation } from '@/operations';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { Enhancement } from '@/components/Enhancement';
import { useEnhancementStore, useFileStore } from '@/stores';

const EMPTY_OPERATIONS: Operation[] = [];

export const SidebarEnhancements = ({ className = '' }: TailwindProps) => {
    const file = useFileStore((state) => state.files[state.currentIndex]);
    const operations = useEnhancementStore((state) => state.enhancements.get(file) ?? EMPTY_OPERATIONS);

    return (
        <List className={`${className}`} dense>
            {operations.map((op) => (
                <div key={op.id}>
                    <Enhancement op={op} />
                    <Divider className='border-[#363636] mx-0.5' />
                </div>
            ))}
        </List>
    );
};
