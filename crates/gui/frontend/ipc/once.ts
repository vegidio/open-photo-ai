/**
 * A fetch that happens once and is kept, and whose rejection is not - for a value the Rust side builds
 * once per process and cannot change while it runs.
 *
 * **The promise is kept, not its value.** That is what makes two callers during the first fetch one
 * `invoke` rather than two: the second awaits the first's promise instead of starting its own.
 *
 * **A rejection is cleared before it is rethrown**, so a failed fetch stays retryable.
 *
 * `forget` is for tests alone: nothing in the application has a reason to drop a value that cannot
 * change, but a test that did not reset it would observe the previous test's fetch.
 */
export const once = <T>(fetch: () => Promise<T>) => {
    // Written once here rather than in each caller, because the rule that is easy to get wrong is not the
    // caching - it is what happens to a rejection.
    let fetched: Promise<T> | undefined;

    return {
        get: () => {
            fetched ??= fetch().catch((error: unknown) => {
                // A kept rejection would make one failed call permanent for the life of the window, which for
                // a value that cannot change is the wrong way round: the answer is always there to be had. It
                // is the same rule Rust's `Setup` follows, for the same reason.
                fetched = undefined;

                throw error;
            });

            return fetched;
        },

        forget: () => {
            fetched = undefined;
        },
    };
};
