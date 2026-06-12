import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { AddEnhancement } from '@/features/enhancements/AddEnhancement';
import { SidebarAutopilot } from '@/features/sidebar/SidebarAutopilot';
import { SidebarEnhancements } from '@/features/sidebar/SidebarEnhancements';
import { SidebarExport } from '@/features/sidebar/SidebarExport';
import { SidebarImage } from '@/features/sidebar/SidebarImage';
import { useFileStore } from '@/stores';

export const Sidebar = ({ className }: TailwindProps) => {
    const fileLength = useFileStore((state) => state.files.length);

    return (
        <div className={`flex flex-col bg-[#272727] border-t border-t-[#171717] border-solid ${className}`}>
            <SidebarImage />

            <div className='flex flex-col bg-black p-6 gap-5'>
                <SidebarAutopilot />
                <AddEnhancement disabled={fileLength === 0} />
            </div>

            <SidebarEnhancements className='py-4 mr-0.5' />

            <div className='flex-1' />

            <SidebarExport className='flex-1' />
        </div>
    );
};
