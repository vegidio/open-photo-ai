import { useState } from 'react';
import { Button } from '@mui/material';
import { PiExport } from 'react-icons/pi';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { Export } from '@/components/Export';
import { useFileStore } from '@/stores';

export const SidebarExport = ({ className = '' }: TailwindProps) => {
    const selectedFilesCount = useFileStore((state) => state.selectedFiles.length);
    const [openExport, setOpenExport] = useState(false);

    return (
        <div>
            <Button
                variant='contained'
                startIcon={<PiExport className='text-[#019aff]' />}
                className={`${className} bg-[#353535] hover:bg-[#171717] disabled:opacity-30 text-[#f2f2f2] normal-case font-normal rounded-none w-full h-12`}
                disabled={selectedFilesCount === 0}
                onClick={() => setOpenExport(true)}
            >
                Export image
            </Button>

            {openExport && <Export open={true} onClose={() => setOpenExport(false)} />}
        </div>
    );
};
