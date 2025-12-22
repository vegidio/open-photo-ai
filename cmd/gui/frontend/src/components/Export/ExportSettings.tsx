import { useRef, useState } from 'react';
import { Button, Divider, MenuItem, Select, type SelectChangeEvent, TextField, Typography } from '@mui/material';
import type { CancellablePromise } from '@wailsio/runtime';
import type { Operation } from '@/operations';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import type { File } from '../../../bindings/gui/types';
import { DialogService } from '../../../bindings/gui/services';
import { Toggle } from '@/components/Toggle.tsx';
import { useExportStore } from '@/stores';
import { suggestEnhancement } from '@/utils/enhancement.ts';
import { exportImage } from '@/utils/export.ts';

type LocationType = 'hidden' | 'original' | 'browse';

type ExportSettingsProps = TailwindProps & {
    enhancements: Map<File, Operation[]>;
    onClose: () => void;
};

export const ExportSettings = ({ enhancements, onClose, className }: ExportSettingsProps) => {
    return (
        <div className={`${className} p-3 flex flex-col gap-4`}>
            <Typography variant='subtitle2'>Export Settings</Typography>

            <Filename />

            <Divider />

            <Location />

            <Divider />

            <Format />

            <div className='flex-1' />

            <Buttons enhancements={enhancements} onClose={onClose} />
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
                <MenuItem value='avif' className='text-sm'>
                    AVIF
                </MenuItem>
                <MenuItem value='bmp' className='text-sm'>
                    BMP
                </MenuItem>
                <MenuItem value='gif' className='text-sm'>
                    GIF
                </MenuItem>
                <MenuItem value='heic' className='text-sm'>
                    HEIC
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
                <MenuItem value='webp' className='text-sm'>
                    WEBP
                </MenuItem>
            </Select>
        </div>
    );
};

const Buttons = ({ enhancements, onClose }: ExportSettingsProps) => {
    const format = useExportStore((state) => state.format);
    const prefix = useExportStore((state) => state.prefix);
    const suffix = useExportStore((state) => state.suffix);
    const location = useExportStore((state) => state.location);
    const overwrite = useExportStore((state) => state.overwrite);

    const [state, setState] = useState<'idle' | 'processing' | 'completed'>('idle');
    const promiseRef = useRef<CancellablePromise<void> | null>(null);

    const handleCancel = () => {
        switch (state) {
            case 'idle':
            case 'completed':
                onClose();
                break;

            case 'processing':
                promiseRef.current?.cancel();
        }
    };

    const handleExport = async () => {
        setState('processing');

        for (const [file, operations] of enhancements.entries()) {
            // The list of operations for this file is empty; it means Autopilot added this file in the export list.
            // We need to check if there are any suitable operations to apply to the file.
            if (operations.length === 0) {
                const suggestions = await suggestEnhancement(file.Path);
                if (suggestions.length === 0) continue;
                operations.push(...suggestions);
            }

            promiseRef.current = exportImage(file, operations, overwrite, format, prefix, suffix, location);

            try {
                await promiseRef.current;
            } catch {
                setState('idle');
                return;
            }
        }

        setState('completed');
    };

    // Exporting

    return (
        <div className='flex gap-3'>
            <Button
                variant='contained'
                className='flex-1 bg-[#353535] hover:bg-[#171717] text-[#f2f2f2] normal-case font-normal'
                onClick={handleCancel}
            >
                {state === 'idle' ? 'Cancel' : state === 'processing' ? 'Abort' : 'Close'}
            </Button>

            <Button
                variant='contained'
                disabled={state === 'processing'}
                className='flex-1 bg-[#009aff] hover:bg-[#007eff] disabled:opacity-50 text-[#f2f2f2] normal-case font-normal'
                onClick={handleExport}
            >
                Save
            </Button>
        </div>
    );
};
