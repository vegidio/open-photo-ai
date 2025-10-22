import { Switch, Typography } from '@mui/material';
import { useControlStore } from '@/stores';

type SidebarAutopilotProps = {
    className?: string;
};

export const SidebarAutopilot = ({ className = '' }: SidebarAutopilotProps) => {
    const autopilot = useControlStore((state) => state.autopilot);
    const setAutopilot = useControlStore((state) => state.setAutopilot);

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
                onClick={() => setAutopilot(!autopilot)}
            />
        </div>
    );
};
