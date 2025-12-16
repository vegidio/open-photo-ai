import { type DragEvent, useEffect, useState } from 'react';
import { Button, Typography } from '@mui/material';
import { Events } from '@wailsio/runtime';
import { MdFolderOpen } from 'react-icons/md';
import type { File } from '../../../bindings/gui/types';
import { DialogService } from '../../../bindings/gui/services';
import { useFileStore } from '@/stores';

export const PreviewEmpty = () => {
    const addFiles = useFileStore((state) => state.addFiles);
    const [isDragging, setIsDragging] = useState(false);

    const onBrowseClick = async () => {
        try {
            const files = await DialogService.OpenFileDialog();
            addFiles(files);
        } catch (e) {
            console.error(e);
        }
    };

    const onDragEnter = (e: DragEvent) => {
        e.preventDefault();
        setIsDragging(true);
    };

    const onDragOver = (e: DragEvent) => {
        e.preventDefault();
    };

    const onDragLeave = (e: DragEvent) => {
        e.preventDefault();
        setIsDragging(false);
    };

    useEffect(() => {
        Events.On('app:FilesDropped', (event) => {
            const files = event.data as File[];
            addFiles(files);
        });

        return () => Events.Off('app:FilesDropped');
    }, [addFiles]);

    return (
        // biome-ignore lint/a11y/noStaticElementInteractions: N/A
        <div
            data-wails-dropzone
            onDragEnter={onDragEnter}
            onDragOver={onDragOver}
            onDragLeave={onDragLeave}
            className={`flex flex-col items-center justify-center size-full ${isDragging ? 'border-3 border-blue-500' : ''}`}
        >
            <MdFolderOpen className='size-20 text-[#009aff]' />

            <div className='flex flex-col text-center gap-3 mb-4 bg-[#171717]'>
                <Typography className='text-[#f2f2f2]'>
                    Drag and drop images
                    <br />
                    to start editing them
                </Typography>

                <Typography variant='subtitle2' className='text-[#979797]'>
                    OR
                </Typography>
            </div>

            <Button
                variant='contained'
                className='bg-[#009aff] hover:bg-[#007eff] text-[#f2f2f2] normal-case font-normal'
                onClick={onBrowseClick}
            >
                Browse images
            </Button>
        </div>
    );
};
