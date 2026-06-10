import { useState } from 'react';
import { Typography } from '@mui/material';
import { IconButton } from '@/components/atoms/IconButton';
import { CropRotate } from '@/components/templates/CropRotate';
import { useImageStore } from '@/stores';

export const SidebarImage = () => {
    const originalImage = useImageStore((state) => state.originalImage);
    const viewport = useImageStore((state) => state.viewport);
    const [cropOpen, setCropOpen] = useState(false);

    return (
        <div className='h-36 flex items-center justify-center relative'>
            {!originalImage && <Typography className='text-[#545454]'>No preview available</Typography>}

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
                <IconButton
                    option='crop'
                    size='small'
                    className='absolute bottom-1 right-1 text-[#9e9e9e]'
                    onClick={() => setCropOpen(true)}
                />
            )}

            <CropRotate open={cropOpen} onClose={() => setCropOpen(false)} />
        </div>
    );
};
