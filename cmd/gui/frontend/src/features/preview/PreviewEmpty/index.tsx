import { Typography } from '@mui/material';
import { Trans, useTranslation } from 'react-i18next';
import { MdFolderOpen } from 'react-icons/md';
import { Button } from '@/components/atoms/Button';
import { useFileManager } from '@/hooks';

export const PreviewEmpty = () => {
    const { t } = useTranslation();
    const { openFiles } = useFileManager();

    return (
        <div className='flex flex-col items-center justify-center size-full'>
            <MdFolderOpen className='size-20 text-[#009aff]' />

            <div className='flex flex-col text-center gap-3 mb-4 bg-[#171717]'>
                {/* The line break is part of the centred two-line layout, so it sits in the catalog where a
                    translator can move or drop it — <br/> is one of i18next's default kept nodes. */}
                <Typography className='text-[#f2f2f2]'>
                    <Trans i18nKey='preview.empty.title' />
                </Typography>

                <Typography variant='subtitle2' className='text-[#979797]'>
                    {t('common.or')}
                </Typography>
            </div>

            <Button onClick={() => openFiles('empty_preview')}>{t('preview.empty.browse')}</Button>
        </div>
    );
};
