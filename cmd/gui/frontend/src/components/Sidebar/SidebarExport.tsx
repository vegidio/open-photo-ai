import { useState } from 'react';
import { Button } from '@mui/material';
import { PiExport } from 'react-icons/pi';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { Export } from '@/components/Export';

export const SidebarExport = ({ className = '' }: TailwindProps) => {
    const [openExport, setOpenExport] = useState(false);

    return (
        <div>
            <Button
                variant='contained'
                startIcon={<PiExport />}
                className={`${className} bg-[#353535] hover:bg-[#171717] text-[#f2f2f2] normal-case font-normal rounded-none w-full h-12`}
                onClick={() => setOpenExport(true)}
            >
                Export image
            </Button>

            <Export open={openExport} onClose={() => setOpenExport(false)} />
        </div>
    );
};
