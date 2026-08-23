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
        },
    }),
);
