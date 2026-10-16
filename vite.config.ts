import { defineConfig } from "vite";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";

const root = fileURLToPath(new URL(".", import.meta.url));

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: false,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    // Mantine 的最低 WebKit 基线；CSS 能力同样需要系统 WebView 支持。
    target: ["safari15.4", "chrome108"],
    rollupOptions: {
      input: {
        widget: resolve(root, "widget.html"),
        reminder: resolve(root, "reminder.html"),
        settings: resolve(root, "settings.html"),
        panel: resolve(root, "panel.html"),
      },
    },
  },
}));
