import { useEffect, useState } from 'react';
import { CancelError, type CancellablePromise, Events } from '@wailsio/runtime';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { AnalyticsEvent, track } from '@/analytics';
import { formatList, mpBucket } from '@/analytics/buckets.ts';
import { EnhancementProgress } from '@/features/preview/EnhancementProgress';
import { PreviewEmpty } from '@/features/preview/PreviewEmpty';
import { PreviewImage } from '@/features/preview/PreviewImage';
import { useCurrentFile, useFileCrop, useFileDisabledFaces, useFileOperations, useNotify } from '@/hooks';
import i18n from '@/i18n';
import { useDrawerStore, useFileStore, useImageStore, useSettingsStore } from '@/stores';
import { DOTTED_BACKGROUND } from '@/utils/constants.ts';
import { getErrorMessage, userFriendlyErrorKey } from '@/utils/errors.ts';
import { getEnhancedImage, getImage, type ImageData } from '@/utils/image.ts';

export const Preview = ({ className = '' }: TailwindProps) => {
    const { enqueueSnackbar } = useNotify();

    const setOpen = useDrawerStore((state) => state.setOpen);

    const filesLength = useFileStore((state) => state.files.length);
    const addFiles = useFileStore((state) => state.addFiles);
    const currentFile = useCurrentFile();

    const setOriginalImage = useImageStore((state) => state.setOriginalImage);
    const setEnhancedImage = useImageStore((state) => state.setEnhancedImage);

    const operations = useFileOperations(currentFile);
    // Re-run the preview when the user toggles which faces are enhanced (the Set ref changes on toggle).
    const disabledFaces = useFileDisabledFaces(currentFile);
    // Re-run the preview when the user applies/clears a crop (the original is fetched cropped, enhanced reads it too).
    const crop = useFileCrop(currentFile);

    const ep = useSettingsStore((state) => state.executionProvider);

    const [isRunning, setIsRunning] = useState(false);

    // biome-ignore lint/correctness/useExhaustiveDependencies: enqueueSnackbar
    useEffect(() => {
        let p: CancellablePromise<ImageData>;
        let isCancelled = false;

        async function loadPreview() {
            if (currentFile) {
                const originalImage = await getImage(currentFile, 0, crop);

                // `p?.cancel()` in the cleanup can't help here: on this path `p` doesn't exist yet, so a file switch
                // while the original is still loading would let the abandoned file's image land in the store on top
                // of the newer one — and then start a full enhancement run for it.
                if (isCancelled) return;

                setOriginalImage(originalImage);
                setEnhancedImage(originalImage);

                if (operations.length > 0) {
                    setIsRunning(true);

                    const opIds = operations.map((op) => op.id);
                    const startedAt = performance.now();
                    p = getEnhancedImage(currentFile, ep, ...opIds);

                    try {
                        const enhancedImage = await p;
                        if (isCancelled) return;

                        setEnhancedImage(enhancedImage);

                        // Reported as a preview render, which is what it is: this effect re-runs on a crop change, a
                        // face toggle or a processor change, and `getEnhancedImage` resolves from its own cache
                        // without touching the backend. The duration therefore covers cache hits too - which is
                        // useful, since "how often is a preview instant?" is a real question - but it is not a
                        // measure of inference time. See `export_completed` for that.
                        track(AnalyticsEvent.PreviewRendered, {
                            operation_count: opIds.length,
                            duration_ms: Math.round(performance.now() - startedAt),
                            mp_bucket: mpBucket(enhancedImage.width, enhancedImage.height),
                            ep,
                        });
                    } catch (e) {
                        // A run this effect has already abandoned reports nothing: it may fail precisely *because* it
                        // was cancelled, and a snackbar about a file the user has navigated away from is noise.
                        if (!isCancelled && !(e instanceof CancelError)) {
                            console.error('Failed to process the image', e);
                            track(AnalyticsEvent.ProcessFailed, { reason: getErrorMessage(e) });
                            enqueueSnackbar(i18n.t(userFriendlyErrorKey(e, 'errors.enhanceFailed')), {
                                variant: 'error',
                            });
                        }
                    } finally {
                        if (!isCancelled) setIsRunning(false);
                    }
                } else {
                    setIsRunning(false);
                }
            } else {
                setOriginalImage(undefined);
                setEnhancedImage(undefined);
            }
        }

        loadPreview();

        return () => {
            isCancelled = true;
            p?.cancel();
        };
        // `ep` belongs here: it selects which model the backend builds the preview on, so a preview produced by the
        // old provider is stale as soon as the user picks a different one and has to be re-run.
    }, [operations, currentFile, disabledFaces, crop, ep, setEnhancedImage, setOriginalImage]);

    useEffect(() => {
        if (filesLength > 1) setOpen(true);
    }, [filesLength, setOpen]);

    // Native file drops (any state) arrive via the Wails `app:FilesDropped` event; the drop zone is the
    // always-mounted `#preview` div below, so this works whether or not an image is already loaded.
    useEffect(() => {
        // Returns the per-listener unsubscribe; `Events.Off` is global across every listener for the name.
        return Events.On('app:FilesDropped', (event) => {
            addFiles(event.data);
            if (event.data?.length > 0) {
                track(AnalyticsEvent.FilesAdded, {
                    count: event.data.length,
                    source: 'drop',
                    formats: formatList(event.data.map((file) => file.Extension)),
                });
            }
        });
    }, [addFiles]);

    return (
        <div
            id='preview'
            data-file-drop-target
            className={`flex items-center justify-center ${DOTTED_BACKGROUND} ${className}`}
        >
            {isRunning && <EnhancementProgress />}
            {filesLength === 0 ? <PreviewEmpty /> : <PreviewImage />}
        </div>
    );
};
