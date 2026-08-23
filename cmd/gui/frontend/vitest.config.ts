import { defineConfig, mergeConfig } from 'vitest/config';
import viteConfig from './vite.config.ts';

// Merged with the app's own config rather than redeclared, so tests resolve `@/…` through exactly the same aliases the
// build uses. A second copy of that alias list would drift the first time one is added on only one side.
export default mergeConfig(
    viteConfig,
    defineConfig({
        test: {
            // Everything covered here is pure TypeScript — operation ids, output filenames, crop cache keys, locale
            // catalogs. Nothing renders, so there is no jsdom or browser provider to configure.
            environment: 'node',
            include: ['src/**/*.test.ts'],
            // export.ts / enhancement.ts import CancellablePromise from '@wailsio/runtime', whose system.js probes
            // for a WebView2/WKWebView bridge on import and console.warns when it finds none. Under `environment:
            // 'node'` there is no `window` at all, so it always warns - once per test file that reaches it. The
            // warning is correct and irrelevant: these tests exercise pure logic and never invoke the bridge.
            // Dropped here rather than by stubbing a fake `window.chrome.webview`, which would make the runtime
            // believe it has a live bridge and hide a real failure if one of these modules ever did call through.
            onConsoleLog(log) {
                if (log.includes('Browser Environment Detected')) return false;
            },
        },
    }),
);
