import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Shared by the browser build (Web Bluetooth) and the Tauri app (app/src-tauri).
const tauriHost = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: "./",
  resolve: { alias: { "@": path.resolve(import.meta.dirname, "./src") } },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: tauriHost || false,
    hmr: tauriHost ? { protocol: "ws", host: tauriHost, port: 1421 } : undefined,
  },
});
