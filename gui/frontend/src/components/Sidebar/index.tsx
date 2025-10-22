import type { TailwindProps } from '@/utils';
import { SidebarAutopilot } from './SidebarAutopilot.tsx';
import { SidebarExport } from './SidebarExport.tsx';

export const Sidebar = ({ className }: TailwindProps) => {
    return (
        <div className={`flex flex-col pt-4 gap-4 bg-[#272727] ${className}`}>
            <SidebarAutopilot className='mx-4' />

            <div className='h-full' />

            <SidebarExport className='flex-1' />
        </div>
    );
};
