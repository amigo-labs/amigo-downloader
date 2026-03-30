// Svelte stores for application state
import { writable, derived } from "svelte/store";

// ========================================
// THEME
// ========================================

export type ThemeMode = "light" | "dark";
export type LayoutMode = "modern" | "classic";
export type AccentColor = "blue" | "green" | "purple" | "coral" | "orange" | "cyan";

function createThemeStore() {
  const stored = typeof localStorage !== "undefined" ? localStorage.getItem("theme") : null;
  const initial: ThemeMode = (stored as ThemeMode) || "dark";
  const { subscribe, set, update } = writable<ThemeMode>(initial);

  return {
    subscribe,
    set(value: ThemeMode) {
      if (typeof localStorage !== "undefined") localStorage.setItem("theme", value);
      document.documentElement.classList.toggle("dark", value === "dark");
      set(value);
    },
    toggle() {
      update((v) => {
        const next = v === "dark" ? "light" : "dark";
        if (typeof localStorage !== "undefined") localStorage.setItem("theme", next);
        document.documentElement.classList.toggle("dark", next === "dark");
        return next;
      });
    },
  };
}

function createLayoutStore() {
  const stored = typeof localStorage !== "undefined" ? localStorage.getItem("layout") : null;
  const initial: LayoutMode = (stored as LayoutMode) || "modern";
  const { subscribe, set } = writable<LayoutMode>(initial);

  return {
    subscribe,
    set(value: LayoutMode) {
      if (typeof localStorage !== "undefined") localStorage.setItem("layout", value);
      set(value);
    },
  };
}

function createAccentStore() {
  const stored = typeof localStorage !== "undefined" ? localStorage.getItem("accent") : null;
  const initial: AccentColor = (stored as AccentColor) || "blue";
  const { subscribe, set } = writable<AccentColor>(initial);

  return {
    subscribe,
    set(value: AccentColor) {
      if (typeof localStorage !== "undefined") localStorage.setItem("accent", value);
      // Remove all accent classes, add new one
      const root = document.documentElement;
      root.className = root.className.replace(/accent-\w+/g, "").trim();
      root.classList.add(`accent-${value}`);
      set(value);
    },
  };
}

export const theme = createThemeStore();
export const layout = createLayoutStore();
export const accent = createAccentStore();

// ========================================
// NAVIGATION
// ========================================

export type Page = "downloads" | "usenet-downloads" | "rss" | "plugins" | "history" | "settings";

const validPages: Page[] = ["downloads", "usenet-downloads", "rss", "plugins", "history", "settings"];

function pageFromHash(): Page {
  const hash = typeof location !== "undefined" ? location.hash.slice(1) : "";
  return validPages.includes(hash as Page) ? (hash as Page) : "downloads";
}

export const currentPage = writable<Page>(pageFromHash());

// Sync URL hash ↔ page state
if (typeof window !== "undefined") {
  currentPage.subscribe((page) => {
    if (location.hash !== `#${page}`) {
      history.pushState({ page }, "", `#${page}`);
    }
  });
  window.addEventListener("popstate", () => {
    currentPage.set(pageFromHash());
  });
}

// ========================================
// DOWNLOADS DATA
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

export const downloads = writable<Download[]>([]);

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

export interface Stats {
  active_downloads: number;
  speed_bytes_per_sec: number;
  queued: number;
  completed: number;
}

export const stats = writable<Stats>({
  active_downloads: 0,
  speed_bytes_per_sec: 0,
  queued: 0,
  completed: 0,
});

// ========================================
// CAPTCHA
// ========================================

export interface CaptchaChallenge {
  id: string;
  plugin_id: string;
  download_id: string;
  image_url: string;
  captcha_type: string;
}

export const pendingCaptcha = writable<CaptchaChallenge | null>(null);

// ========================================
// USENET
// ========================================

export interface UsenetServer {
  id: string;
  name: string;
  host: string;
  port: number;
  ssl: boolean;
  connections: number;
  priority: number;
}

export const usenetServers = writable<UsenetServer[]>([]);
export const usenetDownloads = writable<Download[]>([]);

// ========================================
// FEATURE FLAGS
// ========================================

export interface Features {
  usenet: boolean;
  rss_feeds: boolean;
  server_stats: boolean;
}

export const features = writable<Features>({
  usenet: false,
  rss_feeds: false,
  server_stats: false,
});
