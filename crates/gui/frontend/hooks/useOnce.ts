import { useEffect, useState } from "react";

/**
 * What `fetch` answers, once it has - `initial` for the render before it has.
 *
 * For a value fetched through `ipc/once.ts`: an `invoke` answered once and kept for the life of the process, so
 * every consumer mounting at once is one call rather than one each. An answer landing after the consumer unmounted
 * is dropped rather than set.
 *
 * No error branch, as in `Navbar`'s version number: a rejection here means the IPC bridge is broken, and a rejected
 * promise in the console says so better than an empty control that claims a reason.
 */
export const useOnce = <T, I = undefined>(fetch: () => Promise<T>, initial?: I): T | I => {
    const [value, setValue] = useState<T | I>(initial as I);

    useEffect(() => {
        let live = true;

        fetch().then((answered) => live && setValue(answered));

        return () => {
            live = false;
        };
    }, [fetch]);

    return value;
};
