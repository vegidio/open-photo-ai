import { LinearProgress, Typography } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';

type DependencyProgressProps = TailwindProps & {
    name: string;
    value: number;
};

export const DownloadProgress = ({ name, value, className = '' }: DependencyProgressProps) => {
    return (
        <div className={`${className} flex flex-row gap-2 items-center`}>
            <Typography variant='body2' className='w-28'>
                {name}
            </Typography>

            <div className='flex flex-row flex-1 items-center'>
                <LinearProgress variant='determinate' value={value} className='flex-1' />
                <Typography variant='caption' align='right' className='text-[#b0b0b0] w-10'>
                    {value.toFixed(0)}%
                </Typography>
            </div>
        </div>
    );
};
