import { Divider } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { SidebarAutopilot } from './SidebarAutopilot.tsx';
import { SidebarExport } from './SidebarExport.tsx';
import { AddEnhancement } from '@/components/Enhancement/AddEnhancement.tsx';
import { SidebarEnhancements } from '@/components/Sidebar/SidebarEnhancements.tsx';

export const Sidebar = ({ className }: TailwindProps) => {
    return (
        <div className={`flex flex-col gap-4 bg-[#272727] ${className}`}>
            <div className='flex flex-col bg-black p-6 gap-5'>
                <SidebarAutopilot />
                <AddEnhancement />
            </div>

            <SidebarEnhancements className='py-0 mr-0.5' />

            <div className='h-full' />

            <SidebarExport className='flex-1' />
        </div>
    );
};
