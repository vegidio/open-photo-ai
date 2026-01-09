import { useEffect, useMemo, useState } from 'react';
import { CircularProgress, IconButton, LinearProgress, TableCell, TableRow } from '@mui/material';
import { Events } from '@wailsio/runtime';
import { RiFolderImageLine } from 'react-icons/ri';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import { RevealInFileManager } from '@/bindings/gui/services/osservice.ts';
import { useExportStore } from '@/stores';
import { getExportInfo } from '@/utils/export.ts';
import { getImage } from '@/utils/image.ts';

type ExportQueueRowProps = {
    file: File;
    operations: Operation[];
};

export const ExportQueueRow = ({ file, operations }: ExportQueueRowProps) => {
    const format = useExportStore((state) => state.format);
    const prefix = useExportStore((state) => state.prefix);
    const suffix = useExportStore((state) => state.suffix);
    const location = useExportStore((state) => state.location);

    const [image, setImage] = useState<string>();
    const [state, setState] = useState('');
    const [progress, setProgress] = useState(0);
    const [newSize, setNewSize] = useState<string>();

    const { oldDims, newDims, oldSize, newExt, fileName, filePath } = useMemo(() => {
        const { fileName, filePath, ext } = getExportInfo(file, format, prefix, suffix, location);

        // Dimensions
        const scaleStr = operations.find((op) => op.id.startsWith('up'))?.options?.scale ?? '1';
        const scale = parseInt(scaleStr, 10);
        const oldDims = `${file.Dimensions[0]} x ${file.Dimensions[1]}`;
        const newDims = `${file.Dimensions[0] * scale} x ${file.Dimensions[1] * scale}`;

        // Size
        const oldSize =
            file.Size < 1_000_000 ? `${(file.Size / 1_000).toFixed(2)} KB` : `${(file.Size / 1_000_000).toFixed(2)} MB`;

        return { oldDims, newDims, oldSize, newExt: ext, fileName, filePath };
    }, [file, format, location, operations, prefix, suffix]);

    const getStatusText = (state: string): string => {
        switch (state) {
            case 'RUNNING':
                return 'Processing...';
            case 'ERROR':
                return 'Error';
            case 'COMPLETED':
                return 'Completed';
            default:
                return 'invisible';
        }
    };

    useEffect(() => {
        async function loadImage() {
            const imageData = await getImage(file, 100);
            setImage(imageData.url);
        }

        loadImage();
    }, [file]);

    useEffect(() => {
        const eventName = `app:export:${file.Hash}`;

        Events.On(eventName, (event) => {
            const [state, value] = event.data as [string, number];

            if (state === 'COMPLETED') {
                setNewSize(
                    value < 1_000_000 ? `${(value / 1_000).toFixed(2)} KB` : `${(value / 1_000_000).toFixed(2)} MB`,
                );
                setProgress(100);
            } else {
                setProgress(value * 100);
            }

            setState(state);
        });

        return () => Events.Off(eventName);
    }, [file.Hash]);

    return (
        <>
            <TableRow>
                {/* Image */}
                <TableCell rowSpan={2}>
                    <img alt='Preview' src={image} className='h-14 aspect-square object-cover' />
                </TableCell>

                {/* Filename & Dimensions */}
                <TableCell>
                    <div className='flex flex-col text-[13px] gap-1'>
                        <span>{fileName}</span>
                        <div>
                            <span className='text-[#b0b0b0]'>{oldDims}</span>
                            {oldDims !== newDims && <span> → {newDims}</span>}
                        </div>
                    </div>
                </TableCell>

                {/* Old & New Size */}
                <TableCell>
                    <div className='flex flex-col text-[13px] gap-1'>
                        <span className='invisible'>invisible</span>
                        <div>
                            <span className='text-[#b0b0b0]'>{oldSize}</span>
                            {newSize && <span> → {newSize}</span>}
                        </div>
                    </div>
                </TableCell>

                {/* Status & Extension */}
                <TableCell>
                    <div className='flex flex-col text-[13px] gap-1'>
                        <span className={`${state === '' ? 'invisible' : ''} text-[#009aff]`}>
                            {getStatusText(state)}
                        </span>
                        <div>
                            <span className='text-[#b0b0b0]'>{file.Extension.toUpperCase()}</span>
                            {file.Extension !== newExt && <span> → {newExt.toUpperCase()}</span>}
                        </div>
                    </div>
                </TableCell>

                {/* Loading & Open in File Manager */}
                <TableCell align='center'>
                    {state === 'COMPLETED' ? (
                        <IconButton size='small' onClick={() => RevealInFileManager(filePath)}>
                            <RiFolderImageLine />
                        </IconButton>
                    ) : state === 'RUNNING' ? (
                        <CircularProgress size={20} />
                    ) : null}
                </TableCell>
            </TableRow>

            <TableRow>
                {/* Progress Bar */}
                <TableCell colSpan={4} className='overflow-hidden'>
                    <LinearProgress variant='determinate' value={progress} />
                </TableCell>
            </TableRow>

            <TableRow className='h-4' />
        </>
    );
};
