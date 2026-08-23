import { useRef, useState } from 'react';
import { Button } from '@mui/material';
import { CancelError, type CancellablePromise, Events } from '@wailsio/runtime';
import { useTranslation } from 'react-i18next';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import { AnalyticsEvent, track } from '@/analytics';
import { useExportStore, useSettingsStore } from '@/stores';
import { suggestEnhancement } from '@/utils/enhancement.ts';
import { getErrorMessage } from '@/utils/errors.ts';
import { exportImage } from '@/utils/export.ts';
import { QUALITY_FORMATS, type QualityChoices } from '@/utils/quality.ts';

type ExportSettingsButtonsProps = {
    enhancements: Map<File, Operation[]>;
    quality: QualityChoices;
    onClose: () => void;
};

export const ExportSettingsButtons = ({ enhancements, quality, onClose }: ExportSettingsButtonsProps) => {
    const { t } = useTranslation();
    const format = useExportStore((state) => state.format);
    const prefix = useExportStore((state) => state.prefix);
    const suffix = useExportStore((state) => state.suffix);
    const location = useExportStore((state) => state.location);
    const overwrite = useExportStore((state) => state.overwrite);
    const resetKey = useExportStore((state) => state.resetKey);
    const ep = useSettingsStore((state) => state.executionProvider);
    const models = useSettingsStore((state) => state.models);
    const setQuality = useSettingsStore((state) => state.setQuality);

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

    // Exports every file in turn, reporting whether it got through all of them. Returns early on the first failure —
    // the file's error state has already been emitted by then.
    const exportAll = async (committed: QualityChoices): Promise<boolean> => {
        for (const [file, fileOperations] of enhancements.entries()) {
            let operations = fileOperations;

            try {
                // The list of operations for this file is empty; it means Autopilot added this file in the export
                // list. We need to check if there are any suitable operations to apply to the file.
                if (operations.length === 0) {
                    suggestRef.current = suggestEnhancement(file, models);

                    const suggestions = await suggestRef.current;

                    if (suggestions.length === 0) continue;

                    // A new array rather than a push: this one comes out of a memoised Map in SidebarExport, so
                    // mutating it would make that memo stateful - after one run the "no operations, ask for
                    // suggestions" branch above would never fire again.
                    operations = [...operations, ...suggestions];
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
                    quality: committed,
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

                return false;
            }
        }

        return true;
    };

    const handleExport = async () => {
        if (state === 'completed') {
            resetKey();
            return;
        }

        // Where the Export dialog's draft becomes the remembered value - the point of "last quality used". Committed
        // for every format, not just the visible one: the draft only ever differs from the store where the user
        // changed something, so the rest are no-ops.
        for (const qualityFormat of QUALITY_FORMATS) setQuality(qualityFormat, quality[qualityFormat]);

        // Read back rather than reusing the draft, so the bytes written are encoded with exactly the values that were
        // persisted, clamping included.
        const committed = useSettingsStore.getState().quality;

        setState('processing');
        track(AnalyticsEvent.ExportStarted, { count: enhancements.size, format });

        const completed = await exportAll(committed);

        setState(completed ? 'completed' : 'idle');
    };

    return (
        <div className='flex gap-3'>
            <Button
                variant='contained'
                className='flex-1 bg-[#353535] hover:bg-[#171717] text-[#f2f2f2] normal-case font-normal'
                onClick={handleCancel}
            >
                {state === 'idle'
                    ? t('common.cancel')
                    : state === 'processing'
                      ? t('export.settings.abort')
                      : t('common.close')}
            </Button>

            <Button
                variant='contained'
                disabled={state === 'processing'}
                className={`flex-1 ${state === 'completed' ? 'bg-[#353535] hover:bg-[#171717]' : 'bg-[#009aff] hover:bg-[#007eff]'} disabled:opacity-50 text-[#f2f2f2] normal-case font-normal`}
                onClick={handleExport}
            >
                {state === 'completed' ? t('export.settings.exportAgain') : t('common.save')}
            </Button>
        </div>
    );
};
