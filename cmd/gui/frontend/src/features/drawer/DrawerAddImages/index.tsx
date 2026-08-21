import { Button } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { FiPlus } from 'react-icons/fi';
import { useFileManager } from '@/hooks';

export const DrawerAddImages = () => {
    const { t } = useTranslation();
    const { openFiles } = useFileManager();

    return (
        <Button
            variant='text'
            disableRipple
            startIcon={<FiPlus className='size-6 stroke-1' />}
            className='normal-case text-white font-normal'
            sx={{
                '&:hover': {
                    backgroundColor: 'transparent',
                },
            }}
            onClick={() => openFiles('browse')}
        >
            {t('drawer.addImages')}
        </Button>
    );
};
