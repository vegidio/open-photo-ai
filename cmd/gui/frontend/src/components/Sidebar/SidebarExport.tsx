import { useMemo, useState } from 'react';
import { Button } from '@mui/material';
import { PiExport } from 'react-icons/pi';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { Export } from '@/components/Export';
import { useEnhancementStore, useExportStore, useFileStore } from '@/stores';
import { getExportEligible } from '@/utils/export.ts';

export const SidebarExport = ({ className = '' }: TailwindProps) => {
    const selectedFiles = useFileStore((state) => state.selectedFiles);
    const enhancements = useEnhancementStore((state) => state.enhancements);
    const autopilot = useEnhancementStore((state) => state.autopilot);
    const exportKey = useExportStore((state) => state.key);

    const [openExport, setOpenExport] = useState(false);

    const exportEligible = useMemo(() => {
        return getExportEligible(selectedFiles, enhancements, autopilot);
    }, [autopilot, enhancements, selectedFiles]);

    return (
        <div>
            <Button
                variant='contained'
                startIcon={<PiExport className='text-[#019aff]' />}
                className={`${className} bg-[#353535] hover:bg-[#171717] disabled:opacity-30 text-[#f2f2f2] normal-case font-normal rounded-none w-full h-12`}
                disabled={exportEligible.size === 0}
                onClick={() => setOpenExport(true)}
            >
                Export image
            </Button>

            {openExport && (
                <Export
                    key={exportKey}
                    enhancements={exportEligible}
                    open={true}
                    onClose={() => setOpenExport(false)}
                />
            )}
        </div>
    );
};
