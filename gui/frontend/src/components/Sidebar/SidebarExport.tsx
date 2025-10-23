import type { TailwindProps } from '@/utils/TailwindProps.ts';

export const SidebarExport = ({ className = '' }: TailwindProps) => {
    return (
        <div>
            <button type='button' className={`${className} bg-[#009aff] hover:bg-[#007eff] text-[#f2f2f2] w-full h-12`}>
                Export
            </button>
        </div>
    );
};
