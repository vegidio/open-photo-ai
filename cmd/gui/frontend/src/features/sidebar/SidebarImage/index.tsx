import { useState } from 'react';
import { Typography } from '@mui/material';
import { Button } from '@/components/atoms/Button';
import { Icon } from '@/components/atoms/Icon';
import { CropRotate } from '@/features/crop';
import { useImageStore } from '@/stores';

export const SidebarImage = () => {
    const originalImage = useImageStore((state) => state.originalImage);
    const viewport = useImageStore((state) => state.viewport);
    const [cropOpen, setCropOpen] = useState(false);

    return (
        <div className='h-36 flex items-center justify-center relative'>
            {!originalImage && <Typography className='text-[#545454] text-sm'>No preview available</Typography>}

            {originalImage && (
                <div className='relative'>
                    <img alt='Zoom & Crop' src={originalImage.url} className='block max-h-36 max-w-full' />
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
