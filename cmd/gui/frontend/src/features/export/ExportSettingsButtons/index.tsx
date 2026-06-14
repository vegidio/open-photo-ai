import { useRef, useState } from 'react';
import { Button } from '@mui/material';
import { CancelError, type CancellablePromise, Events } from '@wailsio/runtime';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import { AnalyticsEvent, track } from '@/analytics';
import { useExportStore, useSettingsStore } from '@/stores';
import { suggestEnhancement } from '@/utils/enhancement.ts';
import { getErrorMessage } from '@/utils/errors.ts';
import { exportImage } from '@/utils/export.ts';

type ExportSettingsButtonsProps = {
    enhancements: Map<File, Operation[]>;
    onClose: () => void;
};

export const ExportSettingsButtons = ({ enhancements, onClose }: ExportSettingsButtonsProps) => {
    const format = useExportStore((state) => state.format);
    const prefix = useExportStore((state) => state.prefix);
    const suffix = useExportStore((state) => state.suffix);
    const location = useExportStore((state) => state.location);
    const overwrite = useExportStore((state) => state.overwrite);
    const resetKey = useExportStore((state) => state.resetKey);
    const ep = useSettingsStore((state) => state.executionProvider);
    const dnModel = useSettingsStore((state) => state.dnModel);
    const frModel = useSettingsStore((state) => state.frModel);
    const laModel = useSettingsStore((state) => state.laModel);
    const cbModel = useSettingsStore((state) => state.cbModel);
    const upModel = useSettingsStore((state) => state.upModel);
    const shModel = useSettingsStore((state) => state.shModel);

    const [state, setState] = useState<'idle' | 'processing' | 'completed'>('idle');
    const suggestRef = useRef<CancellablePromise<Operation[]> | undefined>(undefined);
    const exportRef = useRef<CancellablePromise<void> | undefined>(undefined);

    const handleCancel = () => {
        switch (state) {
            case 'idle':
            case 'completed':
                onClose();
                break;

            case 'processing':
                suggestRef.current?.cancel();
                exportRef.current?.cancel();
        }
    };

    const handleExport = async () => {
        if (state === 'completed') {
            resetKey();
            return;
        }

        setState('processing');
        track(AnalyticsEvent.ExportStarted, { count: enhancements.size, format });

        for (const [file, operations] of enhancements.entries()) {
            try {
                // The list of operations for this file is empty; it means Autopilot added this file in the export list.
                // We need to check if there are any suitable operations to apply to the file.
                if (operations.length === 0) {
                    suggestRef.current = suggestEnhancement(file, {
                        dn: dnModel,
                        fr: frModel,
                        la: laModel,
                        cb: cbModel,
                        up: upModel,
                        sh: shModel,
                    });

                    const suggestions = await suggestRef.current;

                    if (suggestions.length === 0) continue;
                    operations.push(...suggestions);
                }

                exportRef.current = exportImage({
                    file,
                    ep,
                    operations,
                    overwrite,
                    format,
                    prefix,
                    suffix,
                    location,
                });
                await exportRef.current;
            } catch (e) {
                if (e instanceof CancelError) {
                    Events.Emit('app:export', { hash: file.Hash, state: 'IDLE', value: 0 });
                } else {
                    const msg = getErrorMessage(e);
                    const tag = msg.includes('[download]') ? 'ERROR_DOWNLOAD' : 'ERROR';
                    Events.Emit('app:export', { hash: file.Hash, state: tag, value: 0 });
                }

                setState('idle');
                return;
            }
        }

        setState('completed');
    };

    // Exporting

    return (
        <div className='flex gap-3'>
            <Button
                variant='contained'
                className='flex-1 bg-[#353535] hover:bg-[#171717] text-[#f2f2f2] normal-case font-normal'
                onClick={handleCancel}
            >
                {state === 'idle' ? 'Cancel' : state === 'processing' ? 'Abort' : 'Close'}
            </Button>

            <Button
                variant='contained'
                disabled={state === 'processing'}
                className={`flex-1 ${state === 'completed' ? 'bg-[#353535] hover:bg-[#171717]' : 'bg-[#009aff] hover:bg-[#007eff]'} disabled:opacity-50 text-[#f2f2f2] normal-case font-normal`}
                onClick={handleExport}
            >
                {state === 'completed' ? 'Export again' : 'Save'}
            </Button>
        </div>
    );
};
