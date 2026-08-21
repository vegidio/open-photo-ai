import { useState } from 'react';
import { Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { Button } from '@/components/atoms/Button';
import { Icon } from '@/components/atoms/Icon';
import { CropRotate } from '@/features/crop';
import { useImageStore } from '@/stores';

export const SidebarImage = () => {
    const { t } = useTranslation();
    const originalImage = useImageStore((state) => state.originalImage);
    const viewport = useImageStore((state) => state.viewport);
    const [cropOpen, setCropOpen] = useState(false);

    return (
        <div className='h-36 flex items-center justify-center relative'>
            {/* Padded rather than flush to the sidebar edges: the placeholder is a full sentence, and in a language
                whose wording is longer than English it renders edge-to-edge on one line — or clips — without room to
                wrap. Constraining the width here forces the wrap and keeps the image below at full sidebar width. */}
            {!originalImage && (
                <Typography className='px-6 text-center text-[#545454] text-sm'>{t('sidebar.noPreview')}</Typography>
            )}

            {originalImage && (
                <div className='relative'>
                    <img alt={t('sidebar.zoomCropAlt')} src={originalImage.url} className='block max-h-36 max-w-full' />
                    {viewport && (
                        <div
                            className='absolute box-border border-2 border-white pointer-events-none'
                            style={{
                                left: `${viewport.x * 100}%`,
                                top: `${viewport.y * 100}%`,
                                width: `${viewport.width * 100}%`,
                                height: `${viewport.height * 100}%`,
                            }}
                        />
                    )}
                </div>
            )}

            {originalImage && (
                <Button
                    option='secondary'
                    onClick={() => setCropOpen(true)}
                    className='min-w-0 absolute bottom-3 right-3 h-8 aspect-square p-2'
                >
                    <Icon option='crop' className='size-full' />
                </Button>
            )}

            <CropRotate open={cropOpen} onClose={() => setCropOpen(false)} />
        </div>
    );
};
