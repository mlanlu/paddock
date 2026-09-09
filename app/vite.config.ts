import { defineConfig } from "vite";

// Tauri serves this over a fixed port in dev and expects the build in dist/.
// `clearScreen: false` keeps cargo's output visible behind vite's.
export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: "es2022", outDir: "dist", emptyOutDir: true },
});
