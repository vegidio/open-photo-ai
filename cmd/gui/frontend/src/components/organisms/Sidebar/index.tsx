import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { AddEnhancement } from '@/components/organisms/AddEnhancement';
import { SidebarAutopilot } from '@/components/organisms/SidebarAutopilot';
import { SidebarEnhancements } from '@/components/organisms/SidebarEnhancements';
import { SidebarExport } from '@/components/organisms/SidebarExport';
import { SidebarImage } from '@/components/organisms/SidebarImage';
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
