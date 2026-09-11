import { defineConfig } from "vite";

export default defineConfig({
  // Rust build outputs must not reload the frontend or interrupt active UI jobs.
  server: { watch: { ignored: ["**/src-tauri/**"] } },
});
