import type { ParseKeys } from 'i18next';

export const getErrorMessage = (error: unknown): string => (error instanceof Error ? error.message : String(error));

// userFriendlyErrorKey maps a backend error to a catalog key safe to show in a toast. `fallback` is used when
// nothing more specific matches.
//
// It returns a key rather than a rendered string so this stays a pure function with no runtime i18n dependency
// (`import type` only): the caller owns the t() call, and therefore the language at the moment the toast is raised.
export const userFriendlyErrorKey = (error: unknown, fallback: ParseKeys): ParseKeys => {
    const msg = getErrorMessage(error);

    if (msg.includes('[download]')) {
        return 'errors.modelDownload';
    }

    return fallback;
};
