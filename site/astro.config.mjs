import { defineConfig } from "astro/config";
import svelte from "@astrojs/svelte";
import mdx from "@astrojs/mdx";
import sitemap from "@astrojs/sitemap";
import tailwindcss from "@tailwindcss/vite";

// Precedence: explicit SITE_URL > Cloudflare-injected preview URL > production default.
// CF_PAGES_URL is exposed by both Cloudflare Pages and the newer Workers Builds
// (Pages-compatible env), so preview and production deploys get matching
// OG/canonical URLs without per-environment config.
const SITE_URL =
  process.env.SITE_URL ??
  process.env.CF_PAGES_URL ??
  "https://amigo-downloader.workers.dev";

// https://astro.build/config
export default defineConfig({
  site: SITE_URL,
  output: "static",
  integrations: [
    svelte(),
    mdx(),
    sitemap({
      i18n: {
        defaultLocale: "en",
        locales: { en: "en", de: "de" },
      },
    }),
  ],
  i18n: {
    defaultLocale: "en",
    locales: ["en", "de"],
    routing: {
      prefixDefaultLocale: false,
    },
  },
  vite: {
    plugins: [tailwindcss()],
  },
});
