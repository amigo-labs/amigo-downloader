import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [svelte(), tailwindcss()],
  build: {
    cssCodeSplit: true,
    // No production sourcemaps: the dist is embedded into the server binary,
    // so shipping maps would only bloat it.
    sourcemap: false,
    rollupOptions: {
      output: {
        // Keep Svelte's runtime in its own long-lived vendor chunk so app
        // changes don't bust its browser cache.
        manualChunks(id) {
          if (id.includes("node_modules/svelte")) return "vendor-svelte";
          return undefined;
        },
      },
    },
  },
  server: {
    proxy: {
      "/api": "http://localhost:1516",
    },
  },
});
