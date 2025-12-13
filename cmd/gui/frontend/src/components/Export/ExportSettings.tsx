import { useState } from 'react';
import { Button, Divider, MenuItem, Select, type SelectChangeEvent, TextField, Typography } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { DialogService } from '../../../bindings/gui/services';
import { Toggle } from '@/components/Toggle.tsx';
import { useEnhancementStore, useExportStore } from '@/stores';
import { exportImage } from '@/utils/export.ts';

type LocationType = 'hidden' | 'original' | 'browse';

type ExportSettingsProps = TailwindProps & {
    onClose: () => void;
};

export const ExportSettings = ({ onClose, className }: ExportSettingsProps) => {
    return (
        <div className={`${className} p-3 flex flex-col gap-4`}>
            <Typography variant='subtitle2'>Export Settings</Typography>

            <Filename />

            <Divider />

            <Location />

            <Divider />

            <Format />

            <div className='flex-1' />

            <Buttons onClose={onClose} />
        </div>
    );
};

const Filename = () => {
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

            <Typography variant='caption' className='text-[#ffcc00] mt-1.5'>
                When file location and extension are the same,
                {overwrite
                    ? ' your original file will be overwritten. This cannot be reverted.'
                    : ' new filenames will add a number instead of overwriting your original.'}
            </Typography>
        </div>
    );
};

const Location = () => {
    const location = useExportStore((state) => state.location);
    const setLocation = useExportStore((state) => state.setLocation);
    const [value, setValue] = useState<LocationType>(location ? 'hidden' : 'original');

    const handleChange = async (event: SelectChangeEvent) => {
        const newValue = event.target.value;

        if (newValue !== 'browse') {
            setValue(newValue as LocationType);
            setLocation(undefined);
            return;
        }

        try {
            const path = await DialogService.OpenDirDialog();

            if (path) {
                setValue('hidden');
                setLocation(path);
            } else if (!location) {
                setValue('original');
            }
        } catch (e) {
            console.error('Error choosing directory', e);
            setValue('original');
        }
    };

    return (
        <div className='flex flex-col gap-2'>
            <Typography variant='body2' className='text-[#b0b0b0]'>
                Save to
            </Typography>

            <Select
                value={value}
                onChange={handleChange}
                size='small'
                renderValue={() => (value === 'hidden' ? location : 'Original directory')}
                slotProps={{
                    input: {
                        className: 'text-sm',
                    },
                }}
            >
                {/* This hidden item is chosen when a directory is selected */}
                <MenuItem value='hidden' className='hidden' />

                <MenuItem value='original' className='text-sm'>
                    Original directory
                </MenuItem>
                <MenuItem value='browse' className='text-sm'>
                    Browse...
                </MenuItem>
            </Select>
        </div>
    );
};

const Format = () => {
    const format = useExportStore((state) => state.format);
    const setFormat = useExportStore((state) => state.setFormat);

    const handleChange = (event: SelectChangeEvent) => {
        setFormat(event.target.value);
    };

    return (
        <div className='flex flex-col gap-2'>
            <Typography variant='body2' className='text-[#b0b0b0]'>
                Format
            </Typography>

            <Select
                value={format}
                onChange={handleChange}
                size='small'
                slotProps={{
                    input: {
                        className: 'text-sm',
                    },
                }}
            >
                <MenuItem value='preserve' className='text-sm'>
                    Preserve
                </MenuItem>
                <MenuItem value='jpg' className='text-sm'>
                    JPG
                </MenuItem>
                <MenuItem value='png' className='text-sm'>
                    PNG
                </MenuItem>
                <MenuItem value='tiff' className='text-sm'>
                    TIFF
                </MenuItem>
            </Select>
        </div>
    );
};

const Buttons = ({ onClose }: ExportSettingsProps) => {
    const enhancements = useEnhancementStore((state) => state.enhancements);

    const handleExport = async () => {
        for (const [file, operations] of enhancements.entries()) {
            await exportImage(file, operations);
        }
    };

    return (
        <div className='flex gap-3'>
            <Button
                variant='contained'
                className='flex-1 bg-[#353535] hover:bg-[#171717] text-[#f2f2f2] normal-case font-normal'
                onClick={onClose}
            >
                Cancel
            </Button>
            <Button
                variant='contained'
                className='flex-1 bg-[#009aff] hover:bg-[#007eff] text-[#f2f2f2] normal-case font-normal'
                onClick={handleExport}
            >
                Save
            </Button>
        </div>
    );
};
