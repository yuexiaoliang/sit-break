import { defineConfig } from "vite";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL(".", import.meta.url));

// https://vite.dev/config/
export default defineConfig(() => ({
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
