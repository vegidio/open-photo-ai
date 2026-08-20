import { Typography } from '@mui/material';
import { Select, type SelectItem } from '@/components/atoms/Select';
import { useExportStore } from '@/stores';

const items: SelectItem[] = [
    { value: 'preserve', label: 'Preserve' },
    { value: 'avif', label: 'AVIF' },
    { value: 'bmp', label: 'BMP' },
    { value: 'gif', label: 'GIF' },
    { value: 'heic', label: 'HEIC' },
    { value: 'jpg', label: 'JPG' },
    { value: 'png', label: 'PNG' },
    { value: 'tiff', label: 'TIFF' },
    { value: 'webp', label: 'WEBP' },
];

export const ExportSettingsFormat = () => {
    const format = useExportStore((state) => state.format);
    const setFormat = useExportStore((state) => state.setFormat);

    return (
        <div className='flex flex-col gap-2'>
            <Typography variant='body2' className='text-[#b0b0b0]'>
                Format
            </Typography>

            <Select items={items} value={format} onValueChange={setFormat} />
        </div>
    );
};
