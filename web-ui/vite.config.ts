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
      // `ws: true` so the /api/v1/ws WebSocket upgrade is proxied too;
      // without it the live connection status / updates silently fail under
      // `vite dev` (production is unaffected — the server serves both).
      "/api": {
        target: "http://localhost:1516",
        ws: true,
      },
    },
  },
});
