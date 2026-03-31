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

export type ThemeMode = "dark" | "light";

function applyThemeClass(value: ThemeMode) {
  const root = document.documentElement;
  root.classList.toggle("light", value === "light");
}

function createThemeStore() {
  const stored = typeof localStorage !== "undefined" ? localStorage.getItem("theme") : null;
  const initial: ThemeMode = (stored as ThemeMode) || "dark";
  const { subscribe, set, update } = writable<ThemeMode>(initial);

  return {
    subscribe,
    set(value: ThemeMode) {
      if (typeof localStorage !== "undefined") localStorage.setItem("theme", value);
      applyThemeClass(value);
      set(value);
    },
    toggle() {
      update((v) => {
        const next: ThemeMode = v === "dark" ? "light" : "dark";
        if (typeof localStorage !== "undefined") localStorage.setItem("theme", next);
        applyThemeClass(next);
        return next;
      });
    },
  };
}

export const theme = createThemeStore();

// ========================================
// COLOR PALETTE
// ========================================

export type ColorPalette = "blue" | "teal" | "indigo" | "amber" | "violet" | "rose";

const PALETTE_COLORS: Record<ColorPalette, string> = {
  blue: "#3b82f6",
  teal: "#14b8a6",
  indigo: "#6366f1",
  amber: "#f59e0b",
  violet: "#8b5cf6",
  rose: "#f43f5e",
};

function createPaletteStore() {
  const stored = typeof localStorage !== "undefined" ? localStorage.getItem("color-palette") : null;
  const initial: ColorPalette = (stored as ColorPalette) || "blue";
  const { subscribe, set } = writable<ColorPalette>(initial);

  return {
    subscribe,
    colors: PALETTE_COLORS,
    set(value: ColorPalette) {
      if (typeof localStorage !== "undefined") localStorage.setItem("color-palette", value);
      const root = document.documentElement;
      root.className = root.className.replace(/palette-\w+/g, "").trim();
      root.classList.add(`palette-${value}`);
      // Re-apply theme class that may have been stripped
      const currentTheme = typeof localStorage !== "undefined" ? localStorage.getItem("theme") : null;
      if (currentTheme === "light") root.classList.add("light");
      set(value);
      // Glow tokens depend on --neon-primary which changes with palette
      const storedIntensity = typeof localStorage !== "undefined"
        ? localStorage.getItem("neon-intensity")
        : null;
      applyIntensity(storedIntensity !== null ? parseFloat(storedIntensity) : 0.5);
    },
  };
}

export const palette = createPaletteStore();

// ========================================
// NEON INTENSITY
// ========================================

export type NeonLevel = 0 | 0.25 | 0.5 | 0.75 | 1;

const NEON_LABELS: Record<number, string> = {
  0: "Off",
  25: "Low",
  50: "Mid",
  75: "High",
  100: "Full",
};

export function getNeonLabel(intensity: number): string {
  return NEON_LABELS[Math.round(intensity * 100)] ?? "";
}

export function applyIntensity(raw: number): void {
  const intensity = Math.max(0, Math.min(1, raw));
  const root = document.documentElement;
  root.style.setProperty("--neon-intensity", String(intensity));

  if (intensity === 0) {
    root.style.setProperty("--neon-glow-sm", "none");
    root.style.setProperty("--neon-glow-md", "none");
    root.style.setProperty("--neon-glow-lg", "none");
    root.style.setProperty("--neon-text-glow", "none");
    root.style.setProperty(
      "--neon-border",
      "color-mix(in srgb, var(--neon-primary) 10%, transparent)"
    );
    root.style.setProperty(
      "--neon-border-hover",
      "color-mix(in srgb, var(--neon-primary) 15%, transparent)"
    );
    root.style.setProperty("--neon-drop-blur", "0px");
  } else {
    const sm = `0 0 ${6 * intensity}px color-mix(in srgb, var(--neon-primary) ${Math.round(25 * intensity)}%, transparent)`;
    root.style.setProperty("--neon-glow-sm", sm);

    const md = `0 0 ${12 * intensity}px color-mix(in srgb, var(--neon-primary) ${Math.round(30 * intensity)}%, transparent), 0 0 ${4 * intensity}px color-mix(in srgb, var(--neon-primary) ${Math.round(15 * intensity)}%, transparent)`;
    root.style.setProperty("--neon-glow-md", md);

    const lg = `0 0 ${20 * intensity}px color-mix(in srgb, var(--neon-primary) ${Math.round(35 * intensity)}%, transparent), 0 0 ${8 * intensity}px color-mix(in srgb, var(--neon-primary) ${Math.round(20 * intensity)}%, transparent), 0 0 ${2 * intensity}px color-mix(in srgb, var(--neon-primary) ${Math.round(40 * intensity)}%, transparent)`;
    root.style.setProperty("--neon-glow-lg", lg);

    const textGlow = `0 0 ${8 * intensity}px color-mix(in srgb, var(--neon-primary) ${Math.round(40 * intensity)}%, transparent)`;
    root.style.setProperty("--neon-text-glow", textGlow);

    const borderOpacity = Math.max(10, Math.round(30 * intensity));
    root.style.setProperty(
      "--neon-border",
      `color-mix(in srgb, var(--neon-primary) ${borderOpacity}%, transparent)`
    );

    const borderHoverOpacity = Math.max(15, Math.round(50 * intensity));
    root.style.setProperty(
      "--neon-border-hover",
      `color-mix(in srgb, var(--neon-primary) ${borderHoverOpacity}%, transparent)`
    );

    root.style.setProperty("--neon-drop-blur", `${3 * intensity}px`);
  }

  root.classList.toggle("neon-full", intensity >= 1.0);
}

function createNeonIntensityStore() {
  const stored = typeof localStorage !== "undefined" ? localStorage.getItem("neon-intensity") : null;
  const initial = stored !== null ? parseFloat(stored) : 0.5;
  const { subscribe, set } = writable<number>(initial);

  return {
    subscribe,
    set(value: number) {
      if (typeof localStorage !== "undefined") localStorage.setItem("neon-intensity", String(value));
      applyIntensity(value);
      set(value);
    },
  };
}

export const neonIntensity = createNeonIntensityStore();

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
