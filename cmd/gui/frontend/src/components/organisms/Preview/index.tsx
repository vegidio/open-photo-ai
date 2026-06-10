import { useEffect, useState } from 'react';
import { CancelError, type CancellablePromise, Events } from '@wailsio/runtime';
import type { TailwindProps } from '@/utils/TailwindProps.ts';
import { EnhancementProgress } from '@/components/organisms/EnhancementProgress';
import { PreviewEmpty } from '@/components/organisms/PreviewEmpty';
import { PreviewImage } from '@/components/organisms/PreviewImage';
import { useCurrentFile, useFileDisabledFaces, useFileOperations, useNotify } from '@/hooks';
import { useDrawerStore, useFileStore, useImageStore, useSettingsStore } from '@/stores';
import { DOTTED_BACKGROUND } from '@/utils/constants.ts';
import { userFriendlyErrorMessage } from '@/utils/errors.ts';
import { getEnhancedImage, getImage, type ImageData } from '@/utils/image.ts';

export const Preview = ({ className = '' }: TailwindProps) => {
    const { enqueueSnackbar } = useNotify();

    // DrawerStore
    const setOpen = useDrawerStore((state) => state.setOpen);

    // FileListStore
    const filesLength = useFileStore((state) => state.files.length);
    const addFiles = useFileStore((state) => state.addFiles);
    const currentFile = useCurrentFile();

    // ImageStore
    const setOriginalImage = useImageStore((state) => state.setOriginalImage);
    const setEnhancedImage = useImageStore((state) => state.setEnhancedImage);

    // EnhancementStore
    const operations = useFileOperations(currentFile);
    // Re-run the preview when the user toggles which faces are enhanced (the Set ref changes on toggle).
    const disabledFaces = useFileDisabledFaces(currentFile);

    const ep = useSettingsStore((state) => state.executionProvider);

    const [isRunning, setIsRunning] = useState(false);

    // biome-ignore lint/correctness/useExhaustiveDependencies: enqueueSnackbar
    useEffect(() => {
        let p: CancellablePromise<ImageData>;
        let isCancelled = false;

        async function loadPreview() {
            if (currentFile) {
                const originalImage = await getImage(currentFile, 0);
                setOriginalImage(originalImage);
                setEnhancedImage(originalImage);

                if (operations.length > 0) {
                    setIsRunning(true);

                    const opIds = operations.map((op) => op.id);
                    p = getEnhancedImage(currentFile, ep, ...opIds);

                    try {
                        const enhancedImage = await p;
                        setEnhancedImage(enhancedImage);
                    } catch (e) {
                        if (!(e instanceof CancelError)) {
                            const msg = userFriendlyErrorMessage(e, 'Something went wrong. Failed to enhance image.');
                            enqueueSnackbar(msg, { variant: 'error' });
                        }
                    } finally {
                        if (!isCancelled) setIsRunning(false);
                    }
                } else {
                    // When there are no operations, use the original image
                    setIsRunning(false);
                }
            } else {
                setOriginalImage(undefined);
                setEnhancedImage(undefined);
            }
        }

        loadPreview();

        return () => {
            // Cancel any pending request to preview the enhanced image
            isCancelled = true;
            p?.cancel();
        };
    }, [operations, currentFile, disabledFaces, setEnhancedImage, setOriginalImage]);

    useEffect(() => {
        if (filesLength > 1) setOpen(true);
    }, [filesLength, setOpen]);

    // Native file drops (any state) arrive via the Wails `app:FilesDropped` event; the drop zone is the
    // always-mounted `#preview` div below, so this works whether or not an image is already loaded.
    useEffect(() => {
        Events.On('app:FilesDropped', (event) => {
            addFiles(event.data);
        });

        return () => Events.Off('app:FilesDropped');
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
