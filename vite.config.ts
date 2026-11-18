import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // Don't watch the Rust build folder; Windows locks files in it mid-build.
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
