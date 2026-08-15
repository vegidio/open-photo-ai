import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import path from "node:path";

// `import.meta.dirname` rather than `__dirname`: Vite's native config loader (slated to become the
// default) can't provide the CommonJS globals, and warns about them from Vite 8.2 onwards.
const rootDir = import.meta.dirname;

// https://vitejs.dev/config/
export default defineConfig({
    plugins: [react(), tailwindcss()],
    // Bind the dev server to IPv4 127.0.0.1. Vite 8's default `localhost` host resolves to
    // IPv6 (::1) only, but the Wails dev proxy dials tcp4 127.0.0.1 — without this the webview
    // gets "connection refused" and stays blank. The port comes from the Taskfile's --port flag.
    server: {
        host: "127.0.0.1",
    },
    resolve: {
        alias: {
            "@/bindings": path.resolve(rootDir, "./bindings"),
            "@": path.resolve(rootDir, "./src"),
        },
    },
});
