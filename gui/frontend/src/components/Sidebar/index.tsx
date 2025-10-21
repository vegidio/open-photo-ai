import { SidebarExport } from './SidebarExport.tsx';

type SidebarProps = {
    className?: string;
};

export const Sidebar = ({ className }: SidebarProps) => {
    return (
        <div className={`flex flex-col ${className}`}>
            <SidebarExport className="flex-1" />
        </div>
    );
};
