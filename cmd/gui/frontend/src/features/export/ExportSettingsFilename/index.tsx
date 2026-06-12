import { TextField, Typography } from '@mui/material';
import { Toggle } from '@/components/atoms/Toggle';
import { useExportStore } from '@/stores';

export const ExportSettingsFilename = () => {
    const prefix = useExportStore((state) => state.prefix);
    const setPrefix = useExportStore((state) => state.setPrefix);
    const suffix = useExportStore((state) => state.suffix);
    const setSuffix = useExportStore((state) => state.setSuffix);
    const overwrite = useExportStore((state) => state.overwrite);
    const setOverwrite = useExportStore((state) => state.setOverwrite);

    return (
        <div className='flex flex-col'>
            <Typography variant='body2' className='text-[#b0b0b0]'>
                Filename
            </Typography>

            <TextField
                label='Prefix'
                variant='outlined'
                size='small'
                margin='dense'
                value={prefix}
                onChange={(e) => setPrefix(e.target.value)}
                slotProps={{
                    input: {
                        className: 'text-sm',
                    },
                    inputLabel: {
                        className: 'text-sm',
                    },
                    htmlInput: {
                        autoCapitalize: 'off',
                        autoCorrect: 'off',
                    },
                }}
            />

            <TextField
                label='Suffix'
                variant='outlined'
                size='small'
                margin='dense'
                value={suffix}
                onChange={(e) => setSuffix(e.target.value)}
                slotProps={{
                    input: {
                        className: 'text-sm',
                    },
                    inputLabel: {
                        className: 'text-sm',
                    },
                    htmlInput: {
                        autoCapitalize: 'off',
                        autoCorrect: 'off',
                    },
                }}
            />

            <Toggle
                label={
                    <Typography variant='body2' className='text-[#b0b0b0]'>
                        Allow overwriting?
                    </Typography>
                }
                initialValue={overwrite}
                color='#009aff'
                onChange={(value) => setOverwrite(value)}
                className='mt-1'
            />

            <Typography variant='caption' className={`${overwrite ? 'text-[#ffcc00]' : ''} mt-1.5`}>
                When file location and extension are the same,
                {overwrite
                    ? ' your original file will be overwritten. This cannot be reverted.'
                    : ' new filenames will add a number instead of overwriting your original.'}
            </Typography>
        </div>
    );
};
