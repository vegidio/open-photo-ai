import { useEffect, useMemo, useState } from 'react';
import {
    Box,
    IconButton,
    LinearProgress,
    Table,
    TableBody,
    TableCell,
    TableContainer,
    TableHead,
    TableRow,
    Typography,
} from '@mui/material';
import path from 'path-browserify';
import { RiFolderImageLine } from 'react-icons/ri';
import type { Operation } from '@/operations';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import type { DialogFile } from '../../../bindings/gui/types';
import { useEnhancementStore, useExportStore } from '@/stores';
import { getImage } from '@/utils/image.ts';

export const ExportQueue = ({ className }: TailwindProps) => {
    const enhancements = useEnhancementStore((state) => state.enhancements);

    return (
        <div className={`${className} p-3 flex flex-col gap-4`}>
            <Typography variant='subtitle2'>Queue ({enhancements.size})</Typography>

            <ImageList enhancements={enhancements} />
        </div>
    );
};

type ImageListProps = {
    enhancements: Map<DialogFile, Operation[]>;
};

const ImageList = ({ enhancements }: ImageListProps) => {
    return (
        <TableContainer component={Box}>
            <Table className='[&_td]:p-0 [&_td]:border-0 [&_th]:p-0 [&_th]:border-0'>
                <TableHead className='[&_th]:text-[#b0b0b0] [&_th]:text-[13px] [&_th]:font-normal'>
                    <TableRow>
                        <TableCell className='w-[72px]'>Image</TableCell>
                        <TableCell>Output</TableCell>
                        <TableCell className='w-44'>Size</TableCell>
                        <TableCell className='w-28'>Type</TableCell>
                        <TableCell className='w-10' />
                    </TableRow>
                </TableHead>

                <TableBody>
                    <TableRow className='h-4' />
                </TableBody>

                <TableBody>
                    {Array.from(enhancements.entries()).map(([file, operations]) => (
                        <ImageRow key={file.Hash} file={file} operations={operations} />
                    ))}
                </TableBody>
            </Table>
        </TableContainer>
    );
};

type ImageRowProps = {
    file: DialogFile;
    operations: Operation[];
};

const ImageRow = ({ file, operations }: ImageRowProps) => {
    const [image, setImage] = useState<string>();
    const format = useExportStore((state) => state.format);
    const prefix = useExportStore((state) => state.prefix);
    const suffix = useExportStore((state) => state.suffix);

    const { oldDims, newDims, oldSize, oldExt, newExt, output } = useMemo(() => {
        // Dimensions
        const scaleStr = operations.find((op) => op.id.startsWith('up'))?.options?.scale ?? '1';
        const scale = parseInt(scaleStr, 10);
        const oldDims = `${file.Dimensions[0]}x${file.Dimensions[1]}`;
        const newDims = `${file.Dimensions[0] * scale}x${file.Dimensions[1] * scale}`;

        // Size
        const oldSize =
            file.Size < 1_000_000 ? `${(file.Size / 1_000).toFixed(2)} KB` : `${(file.Size / 1_000_000).toFixed(2)} MB`;

        // Extension
        const oldExt = file.Extension;
        const newExt = format === 'preserve' ? file.Extension : format;

        // Output
        const baseName = path.basename(file.Path, path.extname(file.Path));
        const output = `${prefix}${baseName}${suffix}.${newExt}`;

        return { oldDims, newDims, oldSize, oldExt, newExt, output };
    }, [operations, file, format, prefix, suffix]);

    useEffect(() => {
        async function loadImage() {
            const imageData = await getImage(file, 100);
            setImage(imageData.url);
        }

        loadImage();
    }, [file]);

    return (
        <>
            <TableRow>
                <TableCell rowSpan={2}>
                    <img alt='Preview' src={image} className='h-14 aspect-square object-cover' />
                </TableCell>

                <TableCell className='flex flex-col text-[13px] gap-1'>
                    <span>{output}</span>
                    <div>
                        <span className='text-[#b0b0b0]'>{oldDims}</span>
                        {oldDims !== newDims && <span> → {newDims}</span>}
                    </div>
                </TableCell>

                <TableCell className='text-[13px]'>
                    <span className='text-[#b0b0b0]'>{oldSize}</span>
                    <span> → {oldSize}</span>
                </TableCell>

                <TableCell className='text-[13px]'>
                    <span className='text-[#b0b0b0]'>{oldExt.toUpperCase()}</span>
                    {oldExt !== newExt && <span> → {newExt.toUpperCase()}</span>}
                </TableCell>

                <TableCell align='center'>
                    <IconButton size='small' onClick={() => {}}>
                        <RiFolderImageLine />
                    </IconButton>
                </TableCell>
            </TableRow>

            <TableRow>
                <TableCell colSpan={4}>
                    <LinearProgress variant='determinate' value={50} />
                </TableCell>
            </TableRow>

            <TableRow className='h-4' />
        </>
    );
};
