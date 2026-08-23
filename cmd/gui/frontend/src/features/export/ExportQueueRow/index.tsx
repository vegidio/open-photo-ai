import { useEffect, useMemo, useState } from 'react';
import { CircularProgress, IconButton, LinearProgress, TableCell, TableRow } from '@mui/material';
import { Events } from '@wailsio/runtime';
import { useTranslation } from 'react-i18next';
import { RiFolderImageLine } from 'react-icons/ri';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import { AnalyticsEvent, track } from '@/analytics';
import { mpBucket } from '@/analytics/buckets.ts';
import { RevealInFileManager } from '@/bindings/gui/services/osservice.ts';
import { ExportQueueState } from '@/features/export/ExportQueueState';
import { useFileCrop, useThumbnail } from '@/hooks';
import { useExportStore, useSettingsStore } from '@/stores';
import { upscaleFactor } from '@/utils/enhancement.ts';
import { getExportInfo } from '@/utils/export.ts';
import { formatBytes } from '@/utils/format.ts';
import { cropDimensions } from '@/utils/image.ts';

type ExportQueueRowProps = {
    file: File;
    operations: Operation[];
};

export const ExportQueueRow = ({ file, operations }: ExportQueueRowProps) => {
    const { t } = useTranslation();
    const format = useExportStore((state) => state.format);
    const prefix = useExportStore((state) => state.prefix);
    const suffix = useExportStore((state) => state.suffix);
    const location = useExportStore((state) => state.location);
    const crop = useFileCrop(file);

    const { url: image, ref: thumbnailRef } = useThumbnail(file, 100);
    const [state, setState] = useState('IDLE');
    const [progress, setProgress] = useState(0);
    const [newSize, setNewSize] = useState<string>();

    const { oldDims, newDims, oldSize, newExt, fileName, filePath } = useMemo(() => {
        const { fileName, filePath, ext } = getExportInfo(file, format, prefix, suffix, location);

        // Dimensions — a crop changes the source size (crop box is post-rotation = the cropped image's size).
        // The scale is read through the shared helper so this row and the navbar cannot disagree about it, which they
        // did while one parsed it as an int and the other as a float.
        const scale = upscaleFactor(operations);
        const [width, height] = cropDimensions(file, crop);
        const oldDims = `${width} x ${height}`;
        const newDims = `${(width * scale).toFixed(0)} x ${(height * scale).toFixed(0)}`;

        const oldSize = formatBytes(file.Size);

        return { oldDims, newDims, oldSize, newExt: ext, fileName, filePath };
    }, [file, format, location, operations, prefix, suffix, crop]);

    // Derived before the effect so the listener closes over a stable string rather than over `file`, whose identity
    // the effect deliberately does not track - it keys on the hash, and a hash fixes the dimensions.
    const mpBand = mpBucket(file.Dimensions[0], file.Dimensions[1]);
    const operationCount = operations.length;

    useEffect(() => {
        return Events.On('app:export', (event) => {
            const { hash, state, value, durationMs } = event.data;
            if (hash !== file.Hash) return;

            if (state === 'COMPLETED') {
                setNewSize(formatBytes(value));
                setProgress(100);

                // The one honest "an image was enhanced and kept" event, so it carries the properties worth having:
                // the duration is measured in Go (inference + encode + write), not across the IPC boundary, and the
                // size band is what makes durations comparable between a phone photo and a medium-format scan.
                track(AnalyticsEvent.ExportCompleted, {
                    format: useExportStore.getState().format,
                    operation_count: operationCount,
                    duration_ms: durationMs,
                    // The source size, which is what determines how much work the models did. An upscale's output is
                    // larger, but it is a fixed multiple of this.
                    mp_bucket: mpBand,
                    ep: useSettingsStore.getState().executionProvider,
                });
            } else {
                setProgress(value * 100);
            }

            if (state === 'ERROR' || state === 'ERROR_DOWNLOAD') {
                track(AnalyticsEvent.ExportFailed, {
                    reason: state,
                    format: useExportStore.getState().format,
                    ep: useSettingsStore.getState().executionProvider,
                });
            }

            setState(state);
        });
    }, [file.Hash, mpBand, operationCount]);

    return (
        <>
            <TableRow>
                {/* Image */}
                <TableCell rowSpan={2}>
                    <img
                        ref={thumbnailRef}
                        alt={t('common.previewAlt')}
                        src={image}
                        className='h-14 aspect-square object-cover'
                    />
                </TableCell>

                {/* Filename & Dimensions */}
                <TableCell>
                    <div className='flex flex-col text-[13px] gap-1'>
                        <span>{fileName}</span>
                        <div>
                            <span className='text-[#b0b0b0]'>{oldDims}</span>
                            {/* biome-ignore lint/style/noJsxLiterals: symbol, not translatable copy */}
                            {oldDims !== newDims && <span> → {newDims}</span>}
                        </div>
                    </div>
                </TableCell>

                {/* Old & New Size */}
                <TableCell>
                    <div className='flex flex-col text-[13px] gap-1'>
                        {/* biome-ignore lint/style/noJsxLiterals: never rendered — a spacer that reserves the row's first line */}
                        <span className='invisible'>invisible</span>
                        <div>
                            <span className='text-[#b0b0b0]'>{oldSize}</span>
                            {/* biome-ignore lint/style/noJsxLiterals: symbol, not translatable copy */}
                            {newSize && <span> → {newSize}</span>}
                        </div>
                    </div>
                </TableCell>

                {/* Status & Extension */}
                <TableCell>
                    <div className='flex flex-col text-[13px] gap-1'>
                        <ExportQueueState state={state} />
                        <div>
                            <span className='text-[#b0b0b0]'>{file.Extension.toUpperCase()}</span>
                            {/* biome-ignore lint/style/noJsxLiterals: symbol, not translatable copy */}
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
                    ) : undefined}
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
