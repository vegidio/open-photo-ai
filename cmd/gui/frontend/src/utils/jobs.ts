import { useJobStore } from '@/stores/jobs.ts';

/**
 * Runs `fn` while counting it as an inference job in flight, so the UI can tell whether a model is busy.
 *
 * This wraps the backend call itself rather than the UI action that triggers it. Everything that reaches a model goes
 * through `getEnhancedImage`, `exportImage`, `detectFaces` or `suggestEnhancement`, so counting in those four places
 * is what keeps a new call site from silently escaping the count — and keeps the cached paths, which never cross into
 * Go, out of it. Nested calls (an enhancement that first detects faces) simply take the counter to 2.
 */
export const withJob = async <T>(fn: () => Promise<T>): Promise<T> => {
    const { beginJob, endJob } = useJobStore.getState();
    beginJob();

    try {
        return await fn();
    } finally {
        endJob();
    }
};
