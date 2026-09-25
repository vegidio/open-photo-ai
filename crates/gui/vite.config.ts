import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
// From `vitest/config` rather than `vite`: it re-exports Vite's own `defineConfig` widened with the
// `test` key, so the build config below is unaffected and the tests inherit the alias and the React
// plugin rather than needing a second config that can drift from this one.
import { defineConfig } from "vitest/config";

// Set by `tauri dev` when developing against a device on the network.
const host = process.env.TAURI_DEV_HOST;

// No React Compiler. `@vitejs/plugin-react` v6 does the JSX transform in oxc with no Babel pass, so
// the inline `react({ babel })` form most guides show is silently a no-op - it belongs with real
// components to compile, added deliberately rather than pre-emptively.
//
// Tailwind 4 is wired as a Vite plugin rather than through PostCSS: v4 replaces the
// `postcss.config.js` + `tailwind.config.js` pair with `@import "tailwindcss"` plus `@theme` in the
// sheet itself, and `@tailwindcss/vite` is the first-party integration for that.
// There is deliberately no `tailwind.config.js` in this crate - the theme lives in
// `frontend/style.css`.
//
// https://vite.dev/config/
export default defineConfig({
    plugins: [react(), tailwindcss()],

    // The runtime half of the `@/*` alias declared in tsconfig.json. Vite does not read
    // `compilerOptions.paths`, so leaving this out breaks the build while `tsc` still passes.
    resolve: {
        alias: {
            "@": fileURLToPath(new URL("./frontend", import.meta.url)),
        },
    },

    // Keeps Vite from obscuring Rust errors.
    clearScreen: false,

    // Tauri expects a fixed port; fail rather than silently pick another.
    server: {
        port: 1420,
        strictPort: true,
        host: host || false,
        // Omitted entirely rather than set to `undefined`: `hmr` is declared `boolean | HmrOptions`,
        // and `exactOptionalPropertyTypes` means an explicit `undefined` is not the same thing as an
        // absent key.
        ...(host && { hmr: { protocol: "ws", host, port: 1421 } }),
        watch: {
            // `crates/gui/src` is this crate's Rust and Cargo has its own rebuild loop; watching it here
            // double-triggers.
            ignored: ["**/src/**"],
        },
    },

    test: {
        // jsdom rather than Vitest's browser mode: nothing under test depends on real layout or
        // real input dispatch, so a browser download would buy nothing.
        environment: "jsdom",
        setupFiles: ["./frontend/test/setup.ts"],
        include: ["frontend/**/*.test.{ts,tsx}"],
        // No `globals`. Tests import `describe`/`it`/`expect`/`vi` by name, which is what
        // `verbatimModuleSyntax` wants and saves a `types` entry here that would have to be kept in
        // step with this.
        globals: false,

        // Every spy reset between tests, rather than each file hand-rolling a `clearAllMocks` in its
        // own `beforeEach` - a habit only ever noticed by the file that forgets.
        clearMocks: true,
        restoreMocks: true,
        unstubGlobals: true,
    },
});
