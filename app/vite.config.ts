import { defineConfig } from "vite";

// Tauri expects a fixed dev port and serves the built files from app/dist.
export default defineConfig({
  root: "ui",
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: "safari15",
    // The settings window and the volume overlay (D-083).
    rollupOptions: { input: { main: "ui/index.html", hud: "ui/hud.html" } },
  },
});
