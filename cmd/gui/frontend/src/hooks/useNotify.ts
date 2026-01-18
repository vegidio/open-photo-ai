import { type OptionsObject, useSnackbar } from 'notistack';
import { useDrawerStore } from '@/stores';

export const useNotify = () => {
    const open = useDrawerStore((state) => state.open);
    const { enqueueSnackbar, closeSnackbar } = useSnackbar();

    const enqueueSnackbarWithDefaults = (message: string, options?: OptionsObject) => {
        const defaultOptions: OptionsObject = {
            variant: 'default',
            preventDuplicate: true,
            style: { marginBottom: open ? '180px' : '52px' },
        };

        return enqueueSnackbar(message, { ...defaultOptions, ...options });
    };

    return {
        enqueueSnackbar: enqueueSnackbarWithDefaults,
        closeSnackbar,
    };
};
