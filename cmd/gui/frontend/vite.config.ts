import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import path from "node:path";

// https://vitejs.dev/config/
export default defineConfig({
    plugins: [react(), tailwindcss()],
    resolve: {
        alias: {
            "@/bindings": path.resolve(__dirname, "./bindings"),
            "@": path.resolve(__dirname, "./src"),
        },
    },
});
