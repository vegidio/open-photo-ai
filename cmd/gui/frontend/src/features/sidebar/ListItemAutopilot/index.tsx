import { CircularProgress, ListItem, ListItemIcon, ListItemText } from '@mui/material';
import { useTranslation } from 'react-i18next';

export const ListItemAutopilot = () => {
    const { t } = useTranslation();
    return (
        <ListItem disablePadding className='py-2 px-4'>
            <ListItemIcon className='ml-6 min-w-10'>
                {/** biome-ignore lint/a11y/noSvgWithoutTitle: N/A */}
                <svg width={0} height={0}>
                    <defs>
                        <linearGradient id='my_gradient' x1='0%' y1='0%' x2='0%' y2='100%'>
                            <stop offset='0%' stopColor='#79e800' />
                            <stop offset='100%' stopColor='#1cb5e0' />
                        </linearGradient>
                    </defs>
                </svg>

                <CircularProgress size={24} sx={{ 'svg circle': { stroke: 'url(#my_gradient)' } }} />
            </ListItemIcon>

            <ListItemText
                primary={t('sidebar.analysing')}
                slotProps={{
                    primary: {
                        className: 'text-[#009aff] font-bold',
                    },
                }}
            />
        </ListItem>
    );
};
