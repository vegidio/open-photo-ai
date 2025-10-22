type SidebarExportProps = {
    className?: string;
};

export const SidebarExport = ({ className = '' }: SidebarExportProps) => {
    return (
        <div>
            <button type='button' className={`${className} bg-[#009aff] hover:bg-[#007eff] text-[#f2f2f2] w-full h-12`}>
                Export
            </button>
        </div>
    );
};
