import { Box, Table, TableBody, TableCell, TableContainer, TableHead, TableRow, Typography } from '@mui/material';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { ExportQueueRow } from './ExportQueueRow.tsx';

type ExportQueueProps = TailwindProps & {
    enhancements: Map<File, Operation[]>;
};

export const ExportQueue = ({ enhancements, className }: ExportQueueProps) => {
    return (
        <div className={`${className} p-3 flex flex-col gap-4`}>
            <Typography variant='subtitle2'>Queue ({enhancements.size})</Typography>

            <ImageList enhancements={enhancements} />
        </div>
    );
};

type ImageListProps = {
    enhancements: Map<File, Operation[]>;
};

const ImageList = ({ enhancements }: ImageListProps) => {
    const padding = enhancements.size > 7 ? 1 : 0;

    return (
        <TableContainer
            component={Box}
            className={padding === 0 ? 'scrollbar-none' : 'scrollbar-thin'}
            sx={{ paddingRight: padding }}
        >
            <Table stickyHeader className='[&_td]:p-0 [&_td]:border-0 [&_th]:p-0 [&_th]:border-0'>
                <TableHead className='[&_th]:text-[#b0b0b0] [&_th]:text-[13px] [&_th]:font-normal [&_th]:bg-[#212121]'>
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
                        <ExportQueueRow key={file.Hash} file={file} operations={operations} />
                    ))}
                </TableBody>
            </Table>
        </TableContainer>
    );
};
