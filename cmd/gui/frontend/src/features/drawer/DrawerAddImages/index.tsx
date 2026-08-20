import { Button } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { FiPlus } from 'react-icons/fi';
import { AnalyticsEvent, track } from '@/analytics';
import { DialogService } from '@/bindings/gui/services';
import { useFileStore } from '@/stores';

export const DrawerAddImages = () => {
    const { t } = useTranslation();
    const addFiles = useFileStore((state) => state.addFiles);

    const onBrowseClick = async () => {
        try {
            const files = await DialogService.OpenFileDialog(
                t('dialogs.native.selectImage'),
                t('dialogs.native.imagesFilter'),
            );
            addFiles(files);
            if (files.length > 0) track(AnalyticsEvent.FilesAdded, { count: files.length, source: 'browse' });
        } catch (e) {
            console.error(e);
        }
    };

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
            onClick={onBrowseClick}
        >
            {t('drawer.addImages')}
        </Button>
    );
};
