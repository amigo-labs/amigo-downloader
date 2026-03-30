# Cyberpunk Arcade Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform amigo-downloader web-ui into a Cyberpunk Arcade design with glassmorphism, neon colors, collapsible sidebar, master-detail layout, and fix all 30 audit issues.

**Architecture:** Rewrite the CSS foundation first (tokens, glass, neon palette), then rebuild the app shell (sidebar, footer, detail panel), then migrate each component with audit fixes baked in. Single feature branch, 6 phases, frequent commits.

**Tech Stack:** Svelte 5, Tailwind CSS v4, CSS Custom Properties, Google Fonts (Rajdhani + Share Tech Mono)

**Spec:** `docs/specs/2026-03-30-cyberpunk-redesign.md`

---

## File Structure

### New Files
| File | Responsibility |
|------|---------------|
| `web-ui/src/components/Icon.svelte` | SVG icon component with aria-label support |
| `web-ui/src/components/DetailPanel.svelte` | Right-side master-detail panel |
| `web-ui/src/components/Footer.svelte` | Status bar with sparkline + stats |
| `web-ui/src/components/SkeletonCard.svelte` | Loading skeleton placeholder |
| `web-ui/src/components/settings/SettingsRssFeeds.svelte` | RSS feeds moved from page to settings sub-component |
| `web-ui/public/amigo-logo.png` | Logo asset (copy of amigo-downloader.png) |

### Modified Files (major changes)
| File | What changes |
|------|-------------|
| `web-ui/src/app.css` | Complete rewrite — neon tokens, glass system, scan-lines, reduced-motion |
| `web-ui/src/App.svelte` | New 3-column layout, collapsible sidebar, footer, protocol filter |
| `web-ui/src/lib/stores.ts` | New stores (sidebar, selectedDownload, protocol), deduplicate Download type |
| `web-ui/src/lib/api.ts` | Re-export Download from stores, no logic changes |
| `web-ui/index.html` | New fonts, updated manifest theme-color |
| `web-ui/src/main.ts` | No changes needed (fonts loaded via index.html) |

### Modified Files (component migration)
Every `.svelte` file in `components/` and `pages/` gets neon/glass styling + relevant audit fixes.

### Deleted Files
| File | Reason |
|------|--------|
| `web-ui/src/pages/UsenetDownloads.svelte` | Merged into Downloads via protocol filter |
| `web-ui/src/pages/RssFeeds.svelte` | Moved to `SettingsRssFeeds.svelte` |

---

## Phase 1: Foundation

### Task 1.1: Copy Logo Asset

**Files:**
- Create: `web-ui/public/amigo-logo.png`

- [ ] **Step 1: Copy logo to public directory**

```bash
cp "d:/github/amigo-downloader/amigo-downloader.png" "d:/github/amigo-downloader/web-ui/public/amigo-logo.png"
```

- [ ] **Step 2: Verify file exists**

```bash
ls -la web-ui/public/amigo-logo.png
```
Expected: File exists, non-zero size.

- [ ] **Step 3: Commit**

```bash
git add web-ui/public/amigo-logo.png
git commit -m "chore: add cyberpunk logo to web-ui public assets"
```

---

### Task 1.2: Update index.html — Fonts & Meta

**Files:**
- Modify: `web-ui/index.html`

- [ ] **Step 1: Replace font imports and update theme-color**

Replace the entire `<head>` content of `web-ui/index.html`:

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>amigo-downloader</title>

    <!-- PWA -->
    <link rel="manifest" href="/manifest.json" />
    <meta name="theme-color" content="#06080f" />
    <meta name="apple-mobile-web-app-capable" content="yes" />
    <meta name="apple-mobile-web-app-status-bar-style" content="black-translucent" />
    <link rel="apple-touch-icon" href="/amigo-logo.png" />
    <meta name="description" content="Fast, modular download manager" />

    <link rel="preconnect" href="https://fonts.googleapis.com" />
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
    <link href="https://fonts.googleapis.com/css2?family=Rajdhani:wght@400;500;600;700&family=Share+Tech+Mono&display=swap" rel="stylesheet" />
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

Key changes: Inter + Press Start 2P → Rajdhani + Share Tech Mono. Theme-color `#0f172a` → `#06080f`. Apple-touch-icon points to new logo.

- [ ] **Step 2: Update manifest.json theme colors**

In `web-ui/public/manifest.json`, change:
- `"theme_color": "#0f172a"` → `"theme_color": "#06080f"`
- `"background_color": "#0f172a"` → `"background_color": "#06080f"`

- [ ] **Step 3: Verify build still works**

```bash
cd web-ui && npm run check
```
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add web-ui/index.html web-ui/public/manifest.json
git commit -m "chore: switch fonts to Rajdhani + Share Tech Mono, update theme colors"
```

---

### Task 1.3: Rewrite app.css — Neon Theme System

**Files:**
- Modify: `web-ui/src/app.css` (complete rewrite)

- [ ] **Step 1: Write the new app.css**

Replace the entire contents of `web-ui/src/app.css` with:

```css
@import "tailwindcss";

/* ========================================
   NEON THEME SYSTEM — Cyberpunk Arcade
   ======================================== */

@theme {
  --color-neon-primary: var(--neon-primary, #00D4FF);
  --color-neon-success: var(--neon-success, #00FFD0);
  --color-neon-accent: var(--neon-accent, #FF2D78);
  --color-neon-warning: var(--neon-warning, #FFB800);
  --color-bg-deep: var(--bg-deep, #06080f);
  --color-bg-surface: var(--bg-surface, #0c0e18);
  --color-bg-surface-2: var(--bg-surface-2, #12152a);
  --color-text: var(--text-primary, #e8ecf4);
  --color-text-secondary: var(--text-secondary, rgba(232, 236, 244, 0.5));
}

/* --- Dark Mode (Default) --- */
:root {
  --neon-primary: #00D4FF;
  --neon-success: #00FFD0;
  --neon-accent: #FF2D78;
  --neon-warning: #FFB800;

  --bg-deep: #06080f;
  --bg-surface: #0c0e18;
  --bg-surface-2: #12152a;
  --text-primary: #e8ecf4;
  --text-secondary: rgba(232, 236, 244, 0.5);
  --border-color: rgba(255, 255, 255, 0.06);

  /* Glass tokens */
  --glass-blur: 16px;
  --glass-bg: rgba(0, 212, 255, 0.08);
  --glass-border: rgba(0, 212, 255, 0.15);
  --glass-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
}

/* --- "Lights On" Mode --- */
:root.lights-on {
  --bg-deep: #14162a;
  --bg-surface: #1a1d38;
  --bg-surface-2: #222548;
  --text-secondary: rgba(232, 236, 244, 0.45);
  --border-color: rgba(255, 255, 255, 0.08);

  --glass-bg: rgba(0, 212, 255, 0.06);
  --glass-border: rgba(0, 212, 255, 0.12);
}

/* --- Accent Presets --- */
:root.accent-hot {
  --neon-primary: #FF2D78;
  --neon-success: #00FFD0;
  --neon-accent: #00D4FF;
  --neon-warning: #FFB800;
  --glass-bg: rgba(255, 45, 120, 0.08);
  --glass-border: rgba(255, 45, 120, 0.15);
}

:root.accent-cyan {
  --neon-primary: #00FFD0;
  --neon-success: #00D4FF;
  --neon-accent: #FF2D78;
  --neon-warning: #FFB800;
  --glass-bg: rgba(0, 255, 208, 0.08);
  --glass-border: rgba(0, 255, 208, 0.15);
}

/* ========================================
   BASE STYLES
   ======================================== */

body {
  background: var(--bg-deep);
  color: var(--text-primary);
  font-family: 'Rajdhani', sans-serif;
}

/* ========================================
   GLASS PANEL
   Reusable class for glassmorphism surfaces
   ======================================== */

.glass-panel {
  background: var(--glass-bg);
  backdrop-filter: blur(var(--glass-blur));
  -webkit-backdrop-filter: blur(var(--glass-blur));
  border: 1px solid var(--glass-border);
  box-shadow: var(--glass-shadow);
}

/* Mobile: disable backdrop-filter for performance (audit M2) */
@media (max-width: 768px) {
  .glass-panel {
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
    background: var(--bg-surface-2);
  }
}

/* ========================================
   NEON GLOW UTILITIES
   ======================================== */

.neon-glow-primary { box-shadow: 0 0 12px rgba(0, 212, 255, 0.3); }
.neon-glow-success { box-shadow: 0 0 12px rgba(0, 255, 208, 0.3); }
.neon-glow-accent { box-shadow: 0 0 12px rgba(255, 45, 120, 0.3); }
.neon-glow-warning { box-shadow: 0 0 12px rgba(255, 184, 0, 0.3); }

.neon-text-primary { color: var(--neon-primary); text-shadow: 0 0 8px rgba(0, 212, 255, 0.4); }
.neon-text-success { color: var(--neon-success); text-shadow: 0 0 8px rgba(0, 255, 208, 0.4); }
.neon-text-accent { color: var(--neon-accent); text-shadow: 0 0 8px rgba(255, 45, 120, 0.4); }
.neon-text-warning { color: var(--neon-warning); text-shadow: 0 0 8px rgba(255, 184, 0, 0.4); }

/* ========================================
   PROGRESS BAR
   ======================================== */

.progress-bar {
  height: 4px;
  border-radius: 2px;
  background: rgba(0, 212, 255, 0.1);
  overflow: hidden;
}

.progress-bar-fill {
  height: 100%;
  border-radius: 2px;
  background: var(--neon-primary);
  box-shadow: 0 0 10px color-mix(in srgb, var(--neon-primary) 40%, transparent);
}

.progress-bar-fill.active {
  background: linear-gradient(
    90deg,
    var(--neon-primary) 0%,
    color-mix(in srgb, var(--neon-primary) 70%, white) 50%,
    var(--neon-primary) 100%
  );
  background-size: 200% 100%;
  animation: shimmer 2s linear infinite;
}

/* ========================================
   CRT SCAN LINES
   ======================================== */

.scan-lines {
  position: fixed;
  inset: 0;
  pointer-events: none;
  z-index: 999;
  background: repeating-linear-gradient(
    0deg,
    transparent,
    transparent 2px,
    rgba(0, 0, 0, 0.02) 2px,
    rgba(0, 0, 0, 0.02) 4px
  );
}

/* ========================================
   NEON TOP LINE (sidebar + footer accent)
   ======================================== */

.neon-top-line::before {
  content: '';
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  height: 2px;
  background: linear-gradient(90deg, transparent, var(--neon-primary), var(--neon-accent), transparent);
}

/* ========================================
   SCROLLBAR
   ======================================== */

::-webkit-scrollbar { width: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: color-mix(in srgb, var(--neon-primary) 15%, transparent); border-radius: 3px; }
::-webkit-scrollbar-thumb:hover { background: color-mix(in srgb, var(--neon-primary) 30%, transparent); }

/* ========================================
   ANIMATIONS
   ======================================== */

@keyframes page-enter {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}

.page-enter {
  animation: page-enter 0.25s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes card-enter {
  from { opacity: 0; transform: translateY(12px) scale(0.98); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

.card-enter {
  animation: card-enter 0.35s cubic-bezier(0.16, 1, 0.3, 1) both;
  animation-delay: calc(var(--i, 0) * 50ms);
}

@keyframes shimmer {
  0% { background-position: -200% 0; }
  100% { background-position: 200% 0; }
}

@keyframes neon-pulse {
  0%, 100% { box-shadow: 0 0 4px var(--neon-primary); }
  50% { box-shadow: 0 0 12px var(--neon-primary), 0 0 4px var(--neon-primary); }
}

.status-pulse {
  animation: neon-pulse 2s ease-in-out infinite;
}

/* ========================================
   FOCUS INDICATORS (audit H3)
   ======================================== */

*:focus-visible {
  outline: 2px solid var(--neon-primary);
  outline-offset: 2px;
}

/* ========================================
   SKIP TO CONTENT (audit L6)
   ======================================== */

.skip-link {
  position: absolute;
  top: -100%;
  left: 16px;
  z-index: 1000;
  padding: 8px 16px;
  background: var(--neon-primary);
  color: var(--bg-deep);
  font-weight: 700;
  border-radius: 0 0 8px 8px;
  text-decoration: none;
}

.skip-link:focus {
  top: 0;
}

/* ========================================
   REDUCED MOTION (audit H2)
   ======================================== */

@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

- [ ] **Step 2: Verify Tailwind processes the new CSS**

```bash
cd web-ui && npm run check
```
Expected: No errors. The `@theme` directive and CSS custom properties should compile fine.

- [ ] **Step 3: Commit**

```bash
git add web-ui/src/app.css
git commit -m "feat: rewrite theme system — neon palette, glass tokens, scan-lines, a11y"
```

---

### Task 1.4: Update stores.ts — New Stores + Deduplicate Types

**Files:**
- Modify: `web-ui/src/lib/stores.ts`

- [ ] **Step 1: Rewrite stores.ts**

Add new stores and fix the duplicate Download type (audit M6). The full updated file:

```typescript
// Svelte stores for application state
import { writable, derived } from "svelte/store";

// ========================================
// TYPES (single source of truth — audit M6)
// ========================================

export interface Download {
  id: string;
  url: string;
  protocol: string;
  filename: string | null;
  filesize: number | null;
  status: string;
  priority: number;
  bytes_downloaded: number;
  speed: number;
  error: string | null;
  created_at: string;
}

export interface Stats {
  active_downloads: number;
  speed_bytes_per_sec: number;
  queued: number;
  completed: number;
}

export interface CaptchaChallenge {
  id: string;
  plugin_id: string;
  download_id: string;
  image_url: string;
  captcha_type: string;
}

export interface UsenetServer {
  id: string;
  name: string;
  host: string;
  port: number;
  ssl: boolean;
  connections: number;
  priority: number;
}

export interface Features {
  usenet: boolean;
  rss_feeds: boolean;
  server_stats: boolean;
}

// ========================================
// THEME
// ========================================

export type ThemeMode = "dark" | "lights-on";
export type AccentPreset = "electric" | "hot" | "cyan";

function createThemeStore() {
  const stored = typeof localStorage !== "undefined" ? localStorage.getItem("theme") : null;
  const initial: ThemeMode = (stored as ThemeMode) || "dark";
  const { subscribe, set, update } = writable<ThemeMode>(initial);

  return {
    subscribe,
    set(value: ThemeMode) {
      if (typeof localStorage !== "undefined") localStorage.setItem("theme", value);
      document.documentElement.classList.toggle("lights-on", value === "lights-on");
      set(value);
    },
    toggle() {
      update((v) => {
        const next: ThemeMode = v === "dark" ? "lights-on" : "dark";
        if (typeof localStorage !== "undefined") localStorage.setItem("theme", next);
        document.documentElement.classList.toggle("lights-on", next === "lights-on");
        return next;
      });
    },
  };
}

function createAccentStore() {
  const stored = typeof localStorage !== "undefined" ? localStorage.getItem("accent") : null;
  const initial: AccentPreset = (stored as AccentPreset) || "electric";
  const { subscribe, set } = writable<AccentPreset>(initial);

  return {
    subscribe,
    set(value: AccentPreset) {
      if (typeof localStorage !== "undefined") localStorage.setItem("accent", value);
      const root = document.documentElement;
      root.className = root.className.replace(/accent-\w+/g, "").trim();
      if (value !== "electric") {
        root.classList.add(`accent-${value}`);
      }
      set(value);
    },
  };
}

export const theme = createThemeStore();
export const accent = createAccentStore();

// ========================================
// SIDEBAR
// ========================================

function createSidebarStore() {
  const stored = typeof localStorage !== "undefined" ? localStorage.getItem("sidebar-collapsed") : null;
  const initial = stored === "true";
  const { subscribe, set, update } = writable<boolean>(initial);

  return {
    subscribe,
    set(value: boolean) {
      if (typeof localStorage !== "undefined") localStorage.setItem("sidebar-collapsed", String(value));
      set(value);
    },
    toggle() {
      update((v) => {
        const next = !v;
        if (typeof localStorage !== "undefined") localStorage.setItem("sidebar-collapsed", String(next));
        return next;
      });
    },
  };
}

export const sidebarCollapsed = createSidebarStore();

// ========================================
// NAVIGATION
// ========================================

export type Page = "downloads" | "plugins" | "history" | "settings";
export type ProtocolFilter = "all" | "http" | "usenet";

const validPages: Page[] = ["downloads", "plugins", "history", "settings"];

function pageFromHash(): Page {
  const hash = typeof location !== "undefined" ? location.hash.slice(1) : "";
  return validPages.includes(hash as Page) ? (hash as Page) : "downloads";
}

export const currentPage = writable<Page>(pageFromHash());
export const protocolFilter = writable<ProtocolFilter>("all");

// Sync URL hash
if (typeof window !== "undefined") {
  currentPage.subscribe((page) => {
    if (location.hash !== `#${page}`) {
      history.pushState({ page }, "", `#${page}`);
    }
    // Dynamic page title (audit L7)
    document.title = `${page.charAt(0).toUpperCase() + page.slice(1)} — amigo-downloader`;
  });
  window.addEventListener("popstate", () => {
    currentPage.set(pageFromHash());
  });
}

// ========================================
// DOWNLOADS DATA
// ========================================

export const downloads = writable<Download[]>([]);
export const selectedDownloadId = writable<string | null>(null);

export const selectedDownload = derived(
  [downloads, selectedDownloadId],
  ([$downloads, $id]) => $id ? $downloads.find((d) => d.id === $id) ?? null : null
);

/** Update a single download's progress in-place (from WebSocket events). */
export function updateDownloadProgress(id: string, bytes_downloaded: number, speed: number, total_bytes?: number) {
  downloads.update((list) =>
    list.map((d) =>
      d.id === id
        ? { ...d, bytes_downloaded, speed, filesize: total_bytes ?? d.filesize }
        : d
    )
  );
}

/** Mark a download's status (from WebSocket events). */
export function updateDownloadStatus(id: string, status: string) {
  downloads.update((list) =>
    list.map((d) => (d.id === id ? { ...d, status } : d))
  );
}

export const activeDownloads = derived(downloads, ($d) =>
  $d.filter((d) => d.status === "downloading")
);
export const queuedDownloads = derived(downloads, ($d) =>
  $d.filter((d) => d.status === "queued")
);

// ========================================
// STATS
// ========================================

export const stats = writable<Stats>({
  active_downloads: 0,
  speed_bytes_per_sec: 0,
  queued: 0,
  completed: 0,
});

// ========================================
// CAPTCHA
// ========================================

export const pendingCaptcha = writable<CaptchaChallenge | null>(null);

// ========================================
// USENET
// ========================================

export const usenetServers = writable<UsenetServer[]>([]);
export const usenetDownloads = writable<Download[]>([]);

// ========================================
// FEATURE FLAGS
// ========================================

export const features = writable<Features>({
  usenet: false,
  rss_feeds: false,
  server_stats: false,
});

// ========================================
// CRASH REPORT (audit M5 — replaces window.__amigo_report_crash)
// ========================================

export interface CrashContext {
  download_id?: string;
  error_message?: string;
  url?: string;
}

export const crashReport = writable<CrashContext | null>(null);
```

Key changes:
- Removed `LayoutMode` (classic/modern) — replaced by the new design
- Removed old `AccentColor` 6-color system — replaced by `AccentPreset` 3-preset system
- Removed `"dark"` class toggle — replaced by `"lights-on"` class (dark is default, no class needed)
- Added `sidebarCollapsed`, `selectedDownloadId`, `selectedDownload`, `protocolFilter`, `crashReport`
- Removed `Page` types for usenet-downloads and rss (merged/moved)
- All interfaces defined here as single source of truth (M6)
- Dynamic page title in currentPage subscriber (L7)

- [ ] **Step 2: Update api.ts to re-export from stores**

In `web-ui/src/lib/api.ts`, remove the duplicate `Download` interface (lines 52-64) and add an import at the top:

```typescript
import type { Download } from "./stores";
```

Remove this block from api.ts:
```typescript
export interface Download {
  id: string;
  url: string;
  protocol: string;
  filename: string | null;
  filesize: number | null;
  status: string;
  priority: number;
  bytes_downloaded: number;
  speed: number;
  error: string | null;
  created_at: string;
}
```

And add a re-export after the import:
```typescript
export type { Download } from "./stores";
```

- [ ] **Step 3: Verify types**

```bash
cd web-ui && npm run check
```
Expected: Type errors due to removed `LayoutMode`, `layout` store, and changed `Page` type. These will be resolved in Task 2.1 when we rewrite App.svelte. For now, note the expected errors and proceed.

- [ ] **Step 4: Commit**

```bash
git add web-ui/src/lib/stores.ts web-ui/src/lib/api.ts
git commit -m "feat: new store system — neon presets, sidebar state, detail panel, protocol filter"
```

---

### Task 1.5: Create Icon.svelte Component

**Files:**
- Create: `web-ui/src/components/Icon.svelte`

- [ ] **Step 1: Create the Icon component**

```svelte
<script lang="ts">
  // Accessible SVG icon component (audit C1)
  // All icon-only buttons MUST use aria-label on the button, not here.
  let {
    name,
    size = 20,
    class: className = "",
  }: {
    name: string;
    size?: number;
    class?: string;
  } = $props();
</script>

<svg
  class={className}
  width={size}
  height={size}
  fill="none"
  stroke="currentColor"
  viewBox="0 0 24 24"
  stroke-width="2"
  aria-hidden="true"
>
  {#if name === "arrow-down"}
    <path d="M12 4v16m0 0l-6-6m6 6l6-6" />
  {:else if name === "newspaper"}
    <path d="M19 3H5a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2V5a2 2 0 00-2-2zM7 7h10M7 11h4m-4 4h10" />
  {:else if name === "puzzle"}
    <path d="M11 4a2 2 0 114 0v1a1 1 0 001 1h3a2 2 0 012 2v3a1 1 0 01-1 1 2 2 0 100 4 1 1 0 011 1v3a2 2 0 01-2 2h-3a1 1 0 01-1-1 2 2 0 10-4 0 1 1 0 01-1 1H7a2 2 0 01-2-2v-3a1 1 0 011-1 2 2 0 100-4 1 1 0 01-1-1V7a2 2 0 012-2h3a1 1 0 001-1V4z" />
  {:else if name === "clock"}
    <path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
  {:else if name === "gear"}
    <path d="M10.3 4.3a1 1 0 011.4 0l.3.3a1 1 0 001.1.2 8 8 0 011.5.6 1 1 0 00.9-.3l.3-.3a1 1 0 011.4 0l1 1a1 1 0 010 1.4l-.3.3a1 1 0 00-.2 1.1c.3.5.5 1 .6 1.5a1 1 0 00.8.7h.4a1 1 0 011 1v1.4a1 1 0 01-1 1h-.4a1 1 0 00-.8.7 8 8 0 01-.6 1.5 1 1 0 00.2 1.1l.3.3a1 1 0 010 1.4l-1 1a1 1 0 01-1.4 0l-.3-.3a1 1 0 00-1.1-.2 8 8 0 01-1.5.6 1 1 0 00-.7.8v.4a1 1 0 01-1 1h-1.4a1 1 0 01-1-1v-.4a1 1 0 00-.7-.8 8 8 0 01-1.5-.6 1 1 0 00-1.1.2l-.3.3a1 1 0 01-1.4 0l-1-1a1 1 0 010-1.4l.3-.3a1 1 0 00.2-1.1 8 8 0 01-.6-1.5 1 1 0 00-.8-.7H4a1 1 0 01-1-1v-1.4a1 1 0 011-1h.4a1 1 0 00.8-.7c.1-.5.3-1 .6-1.5a1 1 0 00-.2-1.1l-.3-.3a1 1 0 010-1.4l1-1a1 1 0 011.4 0l.3.3a1 1 0 001.1.2 8 8 0 011.5-.6 1 1 0 00.7-.8V4a1 1 0 011-1h1.4" />
    <circle cx="12" cy="12" r="3" />
  {:else if name === "menu"}
    <path d="M4 6h16M4 12h16M4 18h16" />
  {:else if name === "sun"}
    <path d="M12 3v1m0 16v1m-8-9H3m18 0h-1m-2.6-6.4l-.7.7m-9.4 9.4l-.7.7m12.8 0l-.7-.7M6 6l-.7-.7M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
  {:else if name === "moon"}
    <path d="M20.4 12.1a8 8 0 01-11.5-7.3 8 8 0 1011.5 7.3z" />
  {:else if name === "x"}
    <path d="M6 18L18 6M6 6l12 12" />
  {:else if name === "pause"}
    <path d="M10 9v6m4-6v6" />
  {:else if name === "play"}
    <path d="M8 5.14v14l11-7-11-7z" />
  {:else if name === "trash"}
    <path d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
  {:else if name === "refresh"}
    <path d="M4 4v5h5M20 20v-5h-5" />
    <path d="M20.5 9A9 9 0 005.6 5.6L4 9m16 6l-1.6 3.4A9 9 0 013.5 15" />
  {:else if name === "grip"}
    <circle cx="9" cy="5" r="1" fill="currentColor" />
    <circle cx="15" cy="5" r="1" fill="currentColor" />
    <circle cx="9" cy="12" r="1" fill="currentColor" />
    <circle cx="15" cy="12" r="1" fill="currentColor" />
    <circle cx="9" cy="19" r="1" fill="currentColor" />
    <circle cx="15" cy="19" r="1" fill="currentColor" />
  {:else if name === "chevron-left"}
    <path d="M15 19l-7-7 7-7" />
  {:else if name === "chevron-right"}
    <path d="M9 5l7 7-7 7" />
  {:else if name === "rss"}
    <path d="M4 11a9 9 0 019 9M4 4a16 16 0 0116 16" />
    <circle cx="5" cy="19" r="1.5" fill="currentColor" />
  {:else if name === "plus"}
    <path d="M12 5v14m-7-7h14" />
  {:else if name === "flag"}
    <path d="M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1zM4 22v-7" />
  {/if}
</svg>
```

Key: `aria-hidden="true"` on the SVG so screen readers skip the icon itself. The parent `<button>` must provide `aria-label`.

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/components/Icon.svelte
git commit -m "feat: extract Icon.svelte component with aria-hidden for a11y"
```

---

## Phase 2: App Shell

### Task 2.1: Rebuild App.svelte — Layout, Sidebar, Protocol Filter

**Files:**
- Modify: `web-ui/src/App.svelte`

This is the largest single task. The new App.svelte implements:
- Collapsible left sidebar with logo
- 3-column layout (sidebar + content + detail panel)
- Footer status bar
- Protocol segmented control in header
- Skip-to-content link (L6)
- Single Escape handler (M11)
- crashReport store instead of window global (M5)

- [ ] **Step 1: Write the new App.svelte**

Replace the entire `web-ui/src/App.svelte`. Due to the size of this file (~300 lines), the implementing agent should:

1. Read the current App.svelte for reference
2. Read the spec section "2. Layout Architecture" for the target layout
3. Read the mockup at `.superpowers/brainstorm/50537-1774879578/content/06-full-design-v2.html` for the visual reference
4. Implement the new layout with these requirements:
   - `<a class="skip-link" href="#main-content">Skip to content</a>` as first element
   - `<div class="scan-lines"></div>` overlay
   - Outer flex column: `app-body` (flex row: sidebar + main + detail) + footer
   - Sidebar: `<aside aria-label="Navigation">` with `class:collapsed={$sidebarCollapsed}`
   - Main: `<main id="main-content" aria-label="Main content">` with header + filters + content
   - Header contains: page title (h2) + protocol segmented control (only on downloads page) + theme toggle
   - Protocol segmented control: `<div role="radiogroup" aria-label="Protocol filter">` with buttons using `role="radio"` and `aria-checked`
   - Import and use `Footer` component (created in Task 2.2)
   - Import and use `DetailPanel` component (created in Task 2.3)
   - Import and use `Icon` component instead of inline SVG snippet
   - Navigation items array: downloads, plugins, history, settings (no usenet-downloads, no rss)
   - Single `svelte:window onkeydown` handler for all keyboard shortcuts including Escape (M11)
   - Use `crashReport` store instead of `window.__amigo_report_crash` (M5)
   - All nav buttons and icon buttons must have `aria-label` (C1)
   - `svelte:boundary` around each page component (M7)

- [ ] **Step 2: Verify it compiles**

```bash
cd web-ui && npm run check
```

Note: This will show errors for Footer/DetailPanel which don't exist yet. Create stub files:

```bash
echo '<div></div>' > web-ui/src/components/Footer.svelte
echo '<div></div>' > web-ui/src/components/DetailPanel.svelte
```

Then re-run check.

- [ ] **Step 3: Commit**

```bash
git add web-ui/src/App.svelte web-ui/src/components/Footer.svelte web-ui/src/components/DetailPanel.svelte
git commit -m "feat: rebuild App.svelte — collapsible sidebar, 3-column layout, protocol filter"
```

---

### Task 2.2: Create Footer.svelte — Status Bar

**Files:**
- Create: `web-ui/src/components/Footer.svelte` (replace stub)

- [ ] **Step 1: Implement Footer.svelte**

The footer component shows:
- Sparkline (reuse Sparkline component)
- Speed stat
- Active count with pulse dot
- Queued count
- Completed count
- Feedback link (right-aligned)

Props: `speedHistory: number[]` (passed from App.svelte)

Import `stats` store and `Sparkline` component. Use `font-family: 'Share Tech Mono', monospace` for all stat values. The footer uses glass styling with neon top-line accent. Height: 36px. Reference the mockup footer section.

All stat labels use `var(--text-secondary)`, values use `var(--neon-primary)`, completed uses `var(--neon-success)`.

Feedback link: `font-size: 12px; opacity: 0.6` (audit L5 — increased from 10px/0.5).

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/components/Footer.svelte
git commit -m "feat: add Footer status bar with sparkline and neon stats"
```

---

### Task 2.3: Create DetailPanel.svelte — Master-Detail

**Files:**
- Create: `web-ui/src/components/DetailPanel.svelte` (replace stub)

- [ ] **Step 1: Implement DetailPanel.svelte**

The detail panel shows info about the selected download:
- Only renders when `$selectedDownload` is not null
- Desktop (>= 768px): slides in from right, 320px wide, pushes content
- Mobile (< 768px): full-height overlay from right with backdrop
- Header: filename + close button (with `aria-label="Close detail panel"`)
- Sections: File Info, Chunk Viz (larger variant), Speed graph, Connection details, Actions, Error log
- Close button sets `selectedDownloadId` to null
- Transition: `translateX(100%)` → `0`, 250ms

For now, implement the structure with all sections but use placeholder data from the `$selectedDownload` store. The detailed chunk visualization and speed graph can show basic versions that will be enhanced in Phase 3.

Use `role="complementary"` and `aria-label="Download details"` on the panel root.

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/components/DetailPanel.svelte
git commit -m "feat: add DetailPanel with master-detail layout and mobile overlay"
```

---

## Phase 3: Download Components

### Task 3.1: Migrate DownloadCard.svelte

**Files:**
- Modify: `web-ui/src/components/DownloadCard.svelte`

- [ ] **Step 1: Rewrite DownloadCard with neon styling**

Key changes:
- Add drag handle (left edge, 28px, 6-dot grip using `Icon name="grip"`, `cursor: grab`)
- Glass panel styling with status-tinted border
- Click handler: `selectedDownloadId.set(download.id)` — selected card gets brighter border
- All action buttons: minimum 44x44px touch target (audit H1), `aria-label` on each (audit C1)
- Use `Icon` component for pause/play/trash instead of emoji characters
- Status badge with neon colors per status
- `card-enter` animation class with `--i` index

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/components/DownloadCard.svelte
git commit -m "feat: migrate DownloadCard — glass styling, drag handles, a11y fixes"
```

---

### Task 3.2: Migrate DownloadRow.svelte

**Files:**
- Modify: `web-ui/src/components/DownloadRow.svelte`

- [ ] **Step 1: Update DownloadRow**

- Neon-themed table row styling
- All action buttons: `aria-label` (C1), 44px touch targets (H1)
- Use `Icon` component instead of ASCII characters
- Click row to select download (opens detail panel)

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/components/DownloadRow.svelte
git commit -m "feat: migrate DownloadRow — neon styling, a11y fixes"
```

---

### Task 3.3: Migrate ChunkViz.svelte

**Files:**
- Modify: `web-ui/src/components/ChunkViz.svelte`

- [ ] **Step 1: Update ChunkViz**

- Neon primary color for chunk fills
- Add `size` prop: `"compact"` (default, used in cards, 5px height) or `"detailed"` (used in detail panel, 12px height with per-chunk labels)
- Chunk background: `rgba(var(--neon-primary), 0.06)`

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/components/ChunkViz.svelte
git commit -m "feat: migrate ChunkViz — neon colors, compact/detailed variants"
```

---

### Task 3.4: Migrate Sparkline.svelte + Fix H8

**Files:**
- Modify: `web-ui/src/components/Sparkline.svelte`

- [ ] **Step 1: Fix gradient ID collision (H8) and apply neon colors**

- Generate unique ID: `const gradientId = $state(\`spark-fill-${crypto.randomUUID().slice(0, 8)}\`);`
- Use `gradientId` in `<linearGradient id={gradientId}>` and `fill="url(#{gradientId})"`
- Default color prop: `"var(--neon-primary)"`
- Dot pulse animation uses neon-pulse keyframes from app.css

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/components/Sparkline.svelte
git commit -m "fix: Sparkline unique gradient IDs + neon colors"
```

---

### Task 3.5: Create SkeletonCard.svelte

**Files:**
- Create: `web-ui/src/components/SkeletonCard.svelte`

- [ ] **Step 1: Create loading skeleton**

```svelte
<script lang="ts">
  let { count = 3 }: { count?: number } = $props();
</script>

{#each Array(count) as _, i}
  <div
    class="rounded-xl p-4 glass-panel animate-pulse"
    style="--i: {i}"
  >
    <div class="flex items-start justify-between gap-3 mb-3">
      <div class="flex-1">
        <div class="h-4 rounded" style="background: var(--border-color); width: 60%"></div>
        <div class="h-3 rounded mt-2" style="background: var(--border-color); width: 80%"></div>
      </div>
      <div class="h-5 w-20 rounded-full" style="background: var(--border-color)"></div>
    </div>
    <div class="h-1 rounded-full" style="background: var(--border-color)"></div>
  </div>
{/each}
```

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/components/SkeletonCard.svelte
git commit -m "feat: add SkeletonCard loading placeholder"
```

---

### Task 3.6: Migrate Downloads.svelte — Protocol Filter + Merge Usenet

**Files:**
- Modify: `web-ui/src/pages/Downloads.svelte`
- Delete: `web-ui/src/pages/UsenetDownloads.svelte`

- [ ] **Step 1: Rewrite Downloads.svelte**

Key changes:
- Import `protocolFilter`, `usenetDownloads` from stores
- Merge HTTP downloads and Usenet downloads into a single list filtered by `$protocolFilter`
- Filter buttons wrapped in `role="radiogroup"` with `aria-label="Filter by status"` (M8)
- Each filter button: `role="radio"`, `aria-checked={filter === f}`
- Click on card: `selectedDownloadId.set(download.id)`
- Use `SkeletonCard` while loading
- Remove layout store dependency (no more classic/modern toggle — cards only)

- [ ] **Step 2: Delete UsenetDownloads.svelte**

```bash
git rm web-ui/src/pages/UsenetDownloads.svelte
```

- [ ] **Step 3: Commit**

```bash
git add web-ui/src/pages/Downloads.svelte
git commit -m "feat: unified Downloads page with protocol filter, delete UsenetDownloads"
```

---

## Phase 4: Dialogs & Overlays

### Task 4.1: Migrate AddDialog.svelte — Glass + Focus Trap

**Files:**
- Modify: `web-ui/src/components/AddDialog.svelte`

- [ ] **Step 1: Update AddDialog**

- Add `role="dialog"`, `aria-modal="true"`, `aria-labelledby="add-dialog-title"` (C2)
- Add focus trap: on mount, query all focusable elements, trap Tab/Shift+Tab within dialog
- Auto-focus textarea on open
- Glass panel styling for dialog body
- Neon-styled buttons and inputs
- Use `Icon name="x"` for close button with `aria-label="Close dialog"`
- Remove separate `svelte:window onkeydown` — Escape handled by App.svelte (M11)

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/components/AddDialog.svelte
git commit -m "feat: migrate AddDialog — glass styling, focus trap, a11y fixes"
```

---

### Task 4.2: Migrate CaptchaDialog.svelte — Fix H7, M3, M10, C2

**Files:**
- Modify: `web-ui/src/components/CaptchaDialog.svelte`

- [ ] **Step 1: Fix all audit issues and apply neon styling**

- Replace `background: var(--card-bg)` with `background: var(--bg-surface)` (M10)
- Move `setInterval` into `onMount` with cleanup return (H7):
  ```typescript
  onMount(() => {
    const timer = setInterval(() => { /* ... */ }, 1000);
    return () => clearInterval(timer);
  });
  ```
- Defer `AudioContext` to user gesture — play sound on first `submitAnswer()` call, not on mount (M3)
- Add `role="dialog"`, `aria-modal="true"`, `aria-labelledby` (C2)
- Add focus trap
- Glass panel styling, neon progress bar and buttons

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/components/CaptchaDialog.svelte
git commit -m "fix: CaptchaDialog — interval cleanup, AudioContext defer, a11y, neon styling"
```

---

### Task 4.3: Migrate FeedbackDialog.svelte

**Files:**
- Modify: `web-ui/src/components/FeedbackDialog.svelte`

- [ ] **Step 1: Apply glass + a11y**

- `role="dialog"`, `aria-modal="true"`, `aria-labelledby` (C2)
- Focus trap
- Glass panel styling
- Use `crashReport` store instead of `prefill` prop from window global
- Neon-styled action cards

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/components/FeedbackDialog.svelte
git commit -m "feat: migrate FeedbackDialog — glass, focus trap, crashReport store"
```

---

### Task 4.4: Migrate DropZone.svelte + Toasts.svelte

**Files:**
- Modify: `web-ui/src/components/DropZone.svelte`
- Modify: `web-ui/src/components/Toasts.svelte`

- [ ] **Step 1: Update DropZone**

- Neon drop overlay with logo image instead of Mascot
- Glass-styled drop area with neon border
- Neon accent text

- [ ] **Step 2: Update Toasts**

- Neon color bars per toast type (success=cyan, error=hot pink, info=electric blue)
- Glass panel background
- Toast position: `right: calc(1rem + 8px)` for scrollbar offset (L3)

- [ ] **Step 3: Commit**

```bash
git add web-ui/src/components/DropZone.svelte web-ui/src/components/Toasts.svelte
git commit -m "feat: migrate DropZone and Toasts — neon styling, glass panels"
```

---

## Phase 5: Pages & Settings

### Task 5.1: Migrate History.svelte

**Files:**
- Modify: `web-ui/src/pages/History.svelte`

- [ ] **Step 1: Apply neon styling + fixes**

- Replace `text-green-500` with `style="color: var(--neon-success)"` (M9)
- Add `SkeletonCard` loading state (M4)
- Glass panel cards
- Use logo image for empty state instead of nothing

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/pages/History.svelte
git commit -m "feat: migrate History — neon styling, skeleton loading, fix hardcoded color"
```

---

### Task 5.2: Migrate Plugins.svelte

**Files:**
- Modify: `web-ui/src/pages/Plugins.svelte`

- [ ] **Step 1: Apply neon styling + fixes**

- Add `SkeletonCard` loading state (M4)
- Glass panel cards
- Remove Press Start 2P references — use logo image for marketplace placeholder
- Neon status badges (Active/Disabled)

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/pages/Plugins.svelte
git commit -m "feat: migrate Plugins — neon styling, skeleton loading"
```

---

### Task 5.3: Migrate Settings + Sub-Components

**Files:**
- Modify: `web-ui/src/pages/Settings.svelte`
- Modify: `web-ui/src/components/settings/SettingsAppearance.svelte`
- Modify: `web-ui/src/components/settings/SettingsFeatures.svelte`
- Modify: `web-ui/src/components/settings/SettingsUsenet.svelte`
- Modify: `web-ui/src/components/settings/SettingsUsenetServers.svelte`
- Modify: `web-ui/src/components/settings/SettingsDownloads.svelte`
- Modify: `web-ui/src/components/settings/SettingsWebhooks.svelte`
- Create: `web-ui/src/components/settings/SettingsRssFeeds.svelte`
- Delete: `web-ui/src/pages/RssFeeds.svelte`

- [ ] **Step 1: Create SettingsRssFeeds.svelte**

Move the content from `web-ui/src/pages/RssFeeds.svelte` into a new settings sub-component. Same logic, neon styling, glass panels.

- [ ] **Step 2: Update Settings.svelte**

- Import `SettingsRssFeeds` and render it under the Usenet section (when `config.features.usenet && config.features.rss_feeds`)
- Remove Press Start 2P "a" logo — use `<img src="/amigo-logo.png">` in About section
- Glass panel styling throughout

- [ ] **Step 3: Update SettingsAppearance.svelte**

- Replace 6-color accent picker with 3 neon presets (Electric, Hot, Cyan)
- Replace Light/Dark toggle with Dark/"Lights On" toggle
- Remove layout mode toggle (classic/modern removed)
- Use `accent` store (new `AccentPreset` type) and `theme` store (new `ThemeMode` type)

- [ ] **Step 4: Update SettingsFeatures.svelte and SettingsUsenet.svelte**

- Toggle switches: add `role="switch"`, `aria-checked={value}`, `aria-label={label}` (H4)
- Neon accent color on active state
- Glass panel containers

- [ ] **Step 5: Update SettingsDownloads.svelte**

- All inputs: replace `outline-none` with proper focus styling (H3)
- Neon input borders on focus
- Glass panel container

- [ ] **Step 6: Update SettingsWebhooks.svelte and SettingsUsenetServers.svelte**

- Glass panel styling
- Neon buttons and inputs
- Proper focus indicators (H3)

- [ ] **Step 7: Delete RssFeeds.svelte page**

```bash
git rm web-ui/src/pages/RssFeeds.svelte
```

- [ ] **Step 8: Commit**

```bash
git add web-ui/src/pages/Settings.svelte web-ui/src/components/settings/ web-ui/src/pages/RssFeeds.svelte
git commit -m "feat: migrate Settings — neon presets, toggle a11y, RSS moved to settings"
```

---

### Task 5.4: Replace Mascot.svelte

**Files:**
- Modify: `web-ui/src/components/Mascot.svelte`

- [ ] **Step 1: Replace SVG mascot with logo image**

```svelte
<script lang="ts">
  let { size = 64, animate = false }: { size?: number; animate?: boolean } = $props();
</script>

<img
  src="/amigo-logo.png"
  alt="amigo-downloader"
  width={size}
  height={size}
  class="rounded-lg"
  class:mascot-glow={animate}
  style="filter: drop-shadow(0 0 {animate ? '12px' : '6px'} color-mix(in srgb, var(--neon-primary) 40%, transparent));"
/>

<style>
  @keyframes glow-pulse {
    0%, 100% { filter: drop-shadow(0 0 6px color-mix(in srgb, var(--neon-primary) 40%, transparent)); }
    50% { filter: drop-shadow(0 0 16px color-mix(in srgb, var(--neon-primary) 60%, transparent)); }
  }

  .mascot-glow {
    animation: glow-pulse 2s ease-in-out infinite;
  }
</style>
```

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/components/Mascot.svelte
git commit -m "feat: replace pixel-art mascot with cyberpunk logo"
```

---

## Phase 6: Polish & Verification

### Task 6.1: Add Error Boundaries (M7)

**Files:**
- Modify: `web-ui/src/App.svelte`

- [ ] **Step 1: Wrap page components in error boundaries**

In App.svelte, wrap each page component in `{#snippet}` with `svelte:boundary`:

```svelte
<svelte:boundary onerror={(e) => console.error('Page error:', e)}>
  {#if $currentPage === "downloads"}
    <Downloads />
  {:else if $currentPage === "plugins"}
    <Plugins />
  {:else if $currentPage === "history"}
    <History />
  {:else if $currentPage === "settings"}
    <Settings />
  {/if}
</svelte:boundary>
```

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/App.svelte
git commit -m "fix: add error boundaries around page components"
```

---

### Task 6.2: Add Table A11y for Classic View (H6)

**Files:**
- Modify: `web-ui/src/pages/Downloads.svelte`

- [ ] **Step 1: Update table markup**

If the classic table view is kept, add:
- `<caption class="sr-only">Downloads list</caption>` inside `<table>`
- `scope="col"` on every `<th>`

Note: If classic view was removed during the redesign, this task can be skipped.

- [ ] **Step 2: Commit**

```bash
git add web-ui/src/pages/Downloads.svelte
git commit -m "fix: table accessibility — caption and scope attributes"
```

---

### Task 6.3: Final Verification

- [ ] **Step 1: Run type check**

```bash
cd web-ui && npm run check
```
Expected: 0 errors.

- [ ] **Step 2: Run build**

```bash
cd web-ui && npm run build
```
Expected: Build succeeds, no warnings.

- [ ] **Step 3: Visual verification checklist**

Start dev server (`npm run dev`) and verify:
- [ ] Sidebar collapses/expands, state persists across reload
- [ ] Protocol segmented control filters downloads
- [ ] Click download card opens detail panel
- [ ] Detail panel closes when clicking X or pressing Escape
- [ ] Mobile (<768px): sidebar is overlay, detail panel is overlay
- [ ] Footer shows sparkline, speed, active, queued, completed
- [ ] All 3 accent presets work (Electric, Hot, Cyan)
- [ ] Dark and "Lights On" modes work
- [ ] CRT scan lines visible
- [ ] Drag handles visible on cards
- [ ] Logo appears in sidebar, empty states, About section

- [ ] **Step 4: Accessibility verification**

- [ ] Tab through entire page — focus indicators visible on all interactive elements
- [ ] Screen reader announces button labels (not just icons)
- [ ] Dialogs trap focus
- [ ] Skip-to-content link works on Tab
- [ ] `prefers-reduced-motion: reduce` disables all animations
- [ ] Toggle switches announce checked/unchecked state

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: complete cyberpunk arcade redesign with all audit fixes"
```

---

## Audit Fix Cross-Reference

Every audit issue mapped to its implementation task:

| Audit ID | Severity | Task |
|----------|----------|------|
| C1 | Critical | 3.1, 3.2 |
| C2 | Critical | 4.1, 4.2, 4.3 |
| C3 | Critical | 1.3 |
| H1 | High | 3.1, 3.2 |
| H2 | High | 1.3 |
| H3 | High | 1.3, 5.3 |
| H4 | High | 5.3 |
| H5 | High | 2.1 |
| H6 | High | 6.2 |
| H7 | High | 4.2 |
| H8 | High | 3.4 |
| M1 | Medium | 1.2 |
| M2 | Medium | 1.3 |
| M3 | Medium | 4.2 |
| M4 | Medium | 3.5, 5.1, 5.2 |
| M5 | Medium | 1.4, 2.1 |
| M6 | Medium | 1.4 |
| M7 | Medium | 6.1 |
| M8 | Medium | 3.6 |
| M9 | Medium | 5.1 |
| M10 | Medium | 4.2 |
| M11 | Medium | 2.1 |
| M12 | Medium | 2.1 |
| L1 | Low | 1.2 |
| L2 | Low | 2.1 |
| L3 | Low | 4.4 |
| L5 | Low | 2.2 |
| L6 | Low | 1.3, 2.1 |
| L7 | Low | 1.4 |
