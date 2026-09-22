import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  // Tauri expects a fixed dev port, fail if that port is not available.
  // 5183 chosen to avoid clashing with other projects on the default 5173.
  server: {
    port: 5183,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  // Env variables starting with TAURI_ are exposed to the frontend
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    target: "es2022",
    minify: "esbuild",
    sourcemap: false,
  },
  clearScreen: false,
});
