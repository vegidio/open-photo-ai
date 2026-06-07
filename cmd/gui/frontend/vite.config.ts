import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import path from "node:path";

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
            "@/bindings": path.resolve(__dirname, "./bindings"),
            "@": path.resolve(__dirname, "./src"),
        },
    },
});
