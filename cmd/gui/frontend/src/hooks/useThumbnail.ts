import { type RefCallback, useCallback, useEffect, useRef, useState } from 'react';
import type { File } from '@/bindings/gui/types';
import { getImage } from '@/utils/image.ts';

type Thumbnail = {
    // Object URL for the decoded thumbnail, or undefined until it has loaded.
    url: string | undefined;

    // Attach to the element the thumbnail belongs to. Until that element scrolls into view, no request is made.
    ref: RefCallback<Element>;
};

/**
 * Loads a file's thumbnail at `size`, once the element it belongs to is actually near the viewport.
 *
 * Both the drawer strip and the export queue had their own copy of this effect, identically, and neither guarded
 * against the component unmounting mid-await — so switching files quickly called setState on a component that was
 * already gone.
 *
 * The IntersectionObserver is the reason this is a hook rather than a shared function. Every item used to fire its
 * own backend round-trip on mount, so dropping a folder of 500 photos issued 500 IPC calls at once for a strip that
 * shows about ten. Decoding is deferred until an item is close to being seen; `rootMargin` keeps it a screen ahead so
 * scrolling still feels instant. getImage caches by hash and size, so an item that scrolls out and back is free.
 */
export const useThumbnail = (file: File, size: number): Thumbnail => {
    const [url, setUrl] = useState<string>();
    const [visible, setVisible] = useState(false);

    const observerRef = useRef<IntersectionObserver>(undefined);

    const ref = useCallback<RefCallback<Element>>((node) => {
        observerRef.current?.disconnect();

        if (!node) return;

        // No IntersectionObserver (a test environment, an old webview) means load straight away rather than never.
        if (typeof IntersectionObserver === 'undefined') {
            setVisible(true);
            return;
        }

        const observer = new IntersectionObserver(
            (entries) => {
                if (!entries.some((entry) => entry.isIntersecting)) return;

                // One-way: once it has been seen the thumbnail stays loaded, so scrolling back and forth does not
                // tear it down and rebuild it.
                setVisible(true);
                observer.disconnect();
            },
            { rootMargin: '200px' },
        );

        observer.observe(node);
        observerRef.current = observer;

        return () => observer.disconnect();
    }, []);

    useEffect(() => () => observerRef.current?.disconnect(), []);

    useEffect(() => {
        if (!visible) return;

        let cancelled = false;

        (async () => {
            try {
                const imageData = await getImage(file, size);
                if (!cancelled) setUrl(imageData.url);
            } catch {
                // A thumbnail that cannot be decoded leaves the placeholder in place; the file itself still lists.
            }
        })();

        return () => {
            cancelled = true;
        };
    }, [file, size, visible]);

    return { url, ref };
};
