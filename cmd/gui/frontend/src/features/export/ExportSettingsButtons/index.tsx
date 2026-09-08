import { useEffect, useRef } from 'react';
import { Button } from '@mui/material';
import { CancelError, type CancellablePromise, Events } from '@wailsio/runtime';
import { useTranslation } from 'react-i18next';
import type { File } from '@/bindings/gui/types';
import type { Operation } from '@/operations';
import { AnalyticsEvent, track } from '@/analytics';
import { useExportStore, useSettingsStore } from '@/stores';
import { suggestEnhancement } from '@/utils/enhancement.ts';
import { getErrorMessage } from '@/utils/errors.ts';
import { exportImage, resolveQualityFormat } from '@/utils/export.ts';
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

    // In the store rather than local component state, because the dialog above reads it too - it refuses to close
    // while a batch is in flight. One place holds it, so the button labels and the dialog's close policy cannot
    // disagree about whether an export is running.
    const state = useExportStore((current) => current.runState);
    const setState = useExportStore((current) => current.setRunState);
    const ep = useSettingsStore((state) => state.executionProvider);
    const models = useSettingsStore((state) => state.models);
    const setQuality = useSettingsStore((state) => state.setQuality);

    const suggestRef = useRef<CancellablePromise<Operation[]> | undefined>(undefined);
    const exportRef = useRef<CancellablePromise<void> | undefined>(undefined);

    // Both refs are cancelled here as well as by Abort, because Abort is not the only way out of this component. The
    // dialog closes on Escape and unmounts the whole subtree, and without this the batch carried on writing files the
    // user believed they had stopped - emitting per-file events at listeners that no longer exist and finishing on a
    // setState against an unmounted component.
    //
    // The run state is reset here as well, and has to be: it now outlives this component. A dialog closed mid-export
    // would otherwise leave the store reading 'processing' forever, and since that is exactly what holds the dialog
    // shut, it could never be opened and closed again.
    //
    // Empty deps: this must run on unmount only. Cancelling a settled promise is a no-op, so there is nothing to
    // guard against a batch that already finished.
    useEffect(
        () => () => {
            suggestRef.current?.cancel();
            exportRef.current?.cancel();
            useExportStore.getState().setRunState('idle');
        },
        [],
    );

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

    // Exports every file in turn, reporting how many it got through and whether it finished. Returns early on the
    // first failure — the file's error state has already been emitted by then.
    //
    // The count is carried out rather than derived from the per-file events: those are emitted by the queue rows, so
    // the batch has no other way to say how far it got before it stopped.
    const exportAll = async (committed: QualityChoices): Promise<{ exported: number; completed: boolean }> => {
        let exported = 0;

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
                exported++;
            } catch (e) {
                if (e instanceof CancelError) {
                    Events.Emit('app:export', { hash: file.Hash, state: 'IDLE', value: 0, durationMs: 0 });
                } else {
                    const msg = getErrorMessage(e);
                    const tag = msg.includes('[download]') ? 'ERROR_DOWNLOAD' : 'ERROR';
                    Events.Emit('app:export', { hash: file.Hash, state: tag, value: 0, durationMs: 0 });
                }

                return { exported, completed: false };
            }
        }

        return { exported, completed: true };
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

        // `file_count`, not `count`: this is the number of files the batch will attempt, which is what the per-file
        // `export_completed` events should add up to. The old name sat next to a per-file `count` on other events and
        // read as though the two were comparable.
        const qualityFormat = resolveQualityFormat(enhancements.keys(), format);

        track(AnalyticsEvent.ExportBatchStarted, {
            file_count: enhancements.size,
            format,
            // Only meaningful when every file in the queue resolves to the same lossy encoder; under "preserve" with a
            // mixed queue each file uses its own stored value and no single number is honest.
            quality: qualityFormat ? committed[qualityFormat] : 0,
            ep,
        });

        const startedAt = performance.now();
        const { exported, completed } = await exportAll(committed);

        track(AnalyticsEvent.ExportBatchFinished, {
            file_count: enhancements.size,
            exported,
            completed,
            duration_ms: Math.round(performance.now() - startedAt),
        });

        setState(completed ? 'completed' : 'idle');
    };

    return (
        <div className='flex gap-3'>
            <Button
                variant='contained'
                className='flex-1 bg-surface-overlay hover:bg-surface-base text-content-primary normal-case font-normal'
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
                className={`flex-1 ${state === 'completed' ? 'bg-surface-overlay hover:bg-surface-base' : 'bg-brand hover:bg-brand-hover'} disabled:opacity-50 text-content-primary normal-case font-normal`}
                onClick={handleExport}
            >
                {state === 'completed' ? t('export.settings.exportAgain') : t('common.save')}
            </Button>
        </div>
    );
};
