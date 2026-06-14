// getErrorMessage extracts a raw error string from an unknown thrown value.
export const getErrorMessage = (error: unknown): string => (error instanceof Error ? error.message : String(error));

// userFriendlyErrorMessage maps a backend error into a short message safe to show in a toast.
// `fallback` is used when nothing more specific matches.
export const userFriendlyErrorMessage = (error: unknown, fallback: string): string => {
    const msg = getErrorMessage(error);

    if (msg.includes('[download]')) {
        return 'Failed to download AI model. Check your internet connection and try again.';
    }

    return fallback;
};
