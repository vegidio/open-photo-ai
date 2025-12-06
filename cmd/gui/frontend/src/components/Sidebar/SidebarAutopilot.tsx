import { Switch, Typography } from '@mui/material';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { useEnhancementStore } from '@/stores';

type SidebarAutopilotProps = TailwindProps & {
    disable?: boolean;
};

export const SidebarAutopilot = ({ disable = false, className = '' }: SidebarAutopilotProps) => {
    const autopilot = useEnhancementStore((state) => state.autopilot);
    const toggle = useEnhancementStore((state) => state.toggle);

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
