import { MenuItem, Select, type SelectChangeEvent, Typography } from '@mui/material';
import { useExportStore } from '@/stores';

export const ExportSettingsFormat = () => {
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
