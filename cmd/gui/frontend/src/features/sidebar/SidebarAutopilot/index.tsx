import { Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { Toggle } from '@/components/atoms/Toggle';
import { useEnhancementStore } from '@/stores';

type SidebarAutopilotProps = TailwindProps & {
    disabled?: boolean;
};

export const SidebarAutopilot = ({ className = '' }: SidebarAutopilotProps) => {
    const { t } = useTranslation();
    const autopilot = useEnhancementStore((state) => state.autopilot);
    const toggle = useEnhancementStore((state) => state.toggle);

    return (
        <Toggle
            label={
                <Typography variant='subtitle2' className='text-[#79e800]'>
                    {t('sidebar.autopilot')}
                </Typography>
            }
            initialValue={autopilot}
            color='#79e800'
            onChange={toggle}
            className={className}
        />
    );
};
