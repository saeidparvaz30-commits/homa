import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { rollupOptions: { input: { main: "index.html", overlay: "overlay.html" } } },
  test: { environment: "jsdom", globals: true, setupFiles: "./src/test-setup.ts" },
});
