import { Switch, Typography } from '@mui/material';
import type { TailwindProps } from '@/utils';
import { useControlStore } from '@/stores';

export const SidebarAutopilot = ({ className = '' }: TailwindProps) => {
    const autopilot = useControlStore((state) => state.autopilot);
    const toggle = useControlStore((state) => state.toggle);

    return (
        <div className={`flex justify-between items-center ${className}`}>
            <Typography variant='subtitle2' className='text-[#79e800]'>
                Autopilot
            </Typography>
            <Switch
                size='small'
                checked={autopilot}
                slotProps={{
                    thumb: {
                        className: autopilot ? '!bg-[#79e800]' : '',
                    },
                    track: {
                        className: autopilot ? '!bg-[#79e800]' : '',
                    },
                }}
                onClick={toggle}
            />
        </div>
    );
};
