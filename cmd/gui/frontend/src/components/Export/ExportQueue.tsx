import { Typography } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';

export const ExportQueue = ({ className }: TailwindProps) => {
    return (
        <div className={`${className} bg-amber-400 p-3`}>
            <Typography variant='subtitle2'>Queue (1)</Typography>
        </div>
    );
};
