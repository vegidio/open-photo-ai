import { useMemo, useState } from 'react';
import { Button } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PiExport } from 'react-icons/pi';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { Export } from '@/features/export';
import { useEnhancementStore, useExportStore, useFileStore } from '@/stores';
import { getExportEligible } from '@/utils/export.ts';

export const SidebarExport = ({ className = '' }: TailwindProps) => {
    const { t } = useTranslation();
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
                startIcon={<PiExport className='text-brand' />}
                className={`${className} bg-surface-overlay hover:bg-surface-base disabled:opacity-30 text-content-primary normal-case font-normal rounded-none w-full h-12`}
                disabled={exportEligible.size === 0}
                onClick={() => setOpenExport(true)}
            >
                {t('sidebar.exportImage')}
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
