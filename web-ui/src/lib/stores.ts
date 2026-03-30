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
