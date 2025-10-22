import { useTheme } from '@mui/material/styles';
import { SidebarAutopilot } from './SidebarAutopilot.tsx';
import { SidebarExport } from './SidebarExport.tsx';

type SidebarProps = {
    className?: string;
};

export const Sidebar = ({ className }: SidebarProps) => {
    const theme = useTheme();
    const primaryColor = theme.palette.primary.main;

    return (
        <div className={`flex flex-col pt-4 gap-4 bg-[${primaryColor}] ${className}`}>
            <SidebarAutopilot className='mx-4' />

            <div className='h-full' />

            <SidebarExport className='flex-1' />
        </div>
    );
};
