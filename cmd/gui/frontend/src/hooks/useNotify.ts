import { useCallback, useMemo } from 'react';
import { type OptionsObject, useSnackbar } from 'notistack';
import { useDrawerStore } from '@/stores';

// How far a snackbar sits above the bottom edge, clearing the drawer when it is open and the status bar when it is
// not. Named because the two are a pair: changing one without the other makes the snackbar overlap or float.
const SNACKBAR_MARGIN_DRAWER_OPEN = '180px';
const SNACKBAR_MARGIN_DRAWER_CLOSED = '52px';

export const useNotify = () => {
    const { enqueueSnackbar, closeSnackbar } = useSnackbar();

    // The drawer state is read at call time via getState() rather than subscribed to with a selector. Subscribing made
    // every consumer of this hook - App, Preview, SidebarEnhancements - re-render on each drawer toggle purely to
    // recompute a margin they were not displaying, and it gave the hook a new identity on every one of those renders.
    // The margin only matters at the moment a snackbar is enqueued, which is exactly when getState() runs.
    //
    // Memoizing is what makes the result usable as a dependency: notistack's enqueueSnackbar is stable, so this
    // callback is too. Callers can list it in their own effect and useCallback dependencies honestly instead of
    // suppressing the exhaustive-deps rule to stop an unstable reference from re-running their effects.
    const enqueueSnackbarWithDefaults = useCallback(
        (message: string, options?: OptionsObject) => {
            const defaultOptions: OptionsObject = {
                variant: 'default',
                preventDuplicate: true,
                style: {
                    marginBottom: useDrawerStore.getState().open
                        ? SNACKBAR_MARGIN_DRAWER_OPEN
                        : SNACKBAR_MARGIN_DRAWER_CLOSED,
                },
            };

            return enqueueSnackbar(message, { ...defaultOptions, ...options });
        },
        [enqueueSnackbar],
    );

    // The wrapper object is memoized as well, so a caller that keeps the whole result rather than destructuring it
    // gets a stable reference too.
    return useMemo(
        () => ({ enqueueSnackbar: enqueueSnackbarWithDefaults, closeSnackbar }),
        [enqueueSnackbarWithDefaults, closeSnackbar],
    );
};
