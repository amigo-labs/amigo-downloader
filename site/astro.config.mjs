import { defineConfig } from "astro/config";
import svelte from "@astrojs/svelte";
import mdx from "@astrojs/mdx";
import sitemap from "@astrojs/sitemap";
import tailwindcss from "@tailwindcss/vite";

// Precedence: explicit SITE_URL > Cloudflare Pages preview URL > production default.
// CF_PAGES_URL is set automatically by Cloudflare Pages (classic) for both preview
// and production builds, so OG/canonical URLs match the actual deploy URL.
const SITE_URL =
  process.env.SITE_URL ??
  process.env.CF_PAGES_URL ??
  "https://amigo-downloader.pages.dev";

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
