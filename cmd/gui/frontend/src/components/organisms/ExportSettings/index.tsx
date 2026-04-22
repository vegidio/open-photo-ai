import { Divider, Typography } from '@mui/material';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { ExportSettingsButtons } from '@/components/organisms/ExportSettingsButtons';
import { ExportSettingsFilename } from '@/components/organisms/ExportSettingsFilename';
import { ExportSettingsFormat } from '@/components/organisms/ExportSettingsFormat';
import { ExportSettingsLocation } from '@/components/organisms/ExportSettingsLocation';

export type ExportSettingsProps = TailwindProps & {
    enhancements: Map<File, Operation[]>;
    onClose: () => void;
};

export const ExportSettings = ({ enhancements, onClose, className }: ExportSettingsProps) => {
    return (
        <div className={`${className} p-3 flex flex-col gap-4`}>
            <Typography variant='subtitle2'>Export Settings</Typography>

            <ExportSettingsFilename />

            <Divider />

            <ExportSettingsLocation />

            <Divider />

            <ExportSettingsFormat />

            <div className='flex-1' />

            <ExportSettingsButtons enhancements={enhancements} onClose={onClose} />
        </div>
    );
};
