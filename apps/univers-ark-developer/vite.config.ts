import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

export default defineConfig(() => {
  const tauriDevHost = process.env.TAURI_DEV_HOST;

  return {
    plugins: [react(), tailwindcss()],
    resolve: {
      alias: {
        "@": path.resolve(__dirname, "./src"),
      },
    },
    clearScreen: false,
    server: {
      host: tauriDevHost ?? "127.0.0.1",
      port: 1536,
      strictPort: true,
      hmr: tauriDevHost
        ? {
            host: tauriDevHost,
            port: 1537,
            protocol: "ws",
          }
        : undefined,
    },
    preview: {
      host: tauriDevHost ?? "127.0.0.1",
      port: 1536,
      strictPort: true,
    },
  };
});
