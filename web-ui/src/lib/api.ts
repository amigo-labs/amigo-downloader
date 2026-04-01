// REST + WebSocket client for amigo-downloader API
import type { Download } from "./stores";

const API_BASE = "/api/v1";

// ========================================
// UNIFIED REQUEST HELPER
// ========================================

export class ApiError extends Error {
  constructor(
    public status: number,
    public body: { error?: string },
  ) {
    super(body?.error || `HTTP ${status}`);
    this.name = "ApiError";
  }
}

async function api<T>(method: string, path: string, body?: unknown): Promise<T> {
  const opts: RequestInit = { method };
  if (body !== undefined) {
    opts.headers = { "Content-Type": "application/json" };
    opts.body = JSON.stringify(body);
  }
  const res = await fetch(`${API_BASE}${path}`, opts);
  if (!res.ok) {
    const errorBody = await res.json().catch(() => ({}));
    throw new ApiError(res.status, errorBody);
  }
  if (res.status === 204) return undefined as T;
  return res.json();
}

// ========================================
// REST API
// ========================================

export const getStatus = () => api<{ status: string; version: string }>("GET", "/status");
export const getStats = () => api<{ active_downloads: number; speed_bytes_per_sec: number; queued: number; completed: number }>("GET", "/stats");
export const getDownloads = () => api<Download[]>("GET", "/downloads");
export const addDownload = (url: string, filename?: string) => api<{ id: string }>("POST", "/downloads", { url, filename });
export const addBatch = (urls: string[]) => api<{ ids: string[] }>("POST", "/downloads/batch", { urls });
export const pauseDownload = (id: string) => api<void>("PATCH", `/downloads/${id}`, { action: "pause" });
export const resumeDownload = (id: string) => api<void>("PATCH", `/downloads/${id}`, { action: "resume" });
export const deleteDownload = (id: string) => api<void>("DELETE", `/downloads/${id}`);
export const getQueue = () => api<Download[]>("GET", "/queue");
export const getHistory = () => api<Download[]>("GET", "/history");
export const getPlugins = () => api<Plugin[]>("GET", "/plugins");
export const checkUpdates = () => api<unknown>("GET", "/updates/check");
export const getSystemInfo = () => api<unknown>("GET", "/system-info");

// Re-export Download from stores (single source of truth — audit M6)
export type { Download } from "./stores";

interface Plugin {
  id: string;
  name: string;
  version: string;
  url_pattern: string;
  enabled: boolean;
}

export async function importDlc(file: File) {
  const formData = new FormData();
  formData.append("file", file);
  const res = await fetch(`${API_BASE}/downloads/container`, {
    method: "POST",
    body: formData,
  });
  if (!res.ok) throw new ApiError(res.status, await res.json().catch(() => ({})));
  return res.json();
}

export const uploadNzb = (nzbData: string) => api<{ id: string }>("POST", "/downloads/nzb", { nzb_data: nzbData });

export const submitFeedback = (data: {
  type: "bug" | "feature" | "crash";
  title: string;
  description: string;
  include_system_info?: boolean;
  error_context?: { download_id?: string; error_message?: string; url?: string };
}) => api<unknown>("POST", "/feedback", data);

// ========================================
// CAPTCHA
// ========================================

export const getPendingCaptchas = () => api<unknown[]>("GET", "/captcha/pending");
export const solveCaptcha = (id: string, answer: string) => api<void>("POST", `/captcha/${id}/solve`, { answer });
export const cancelCaptcha = (id: string) => api<void>("POST", `/captcha/${id}/cancel`);

// ========================================
// WEBHOOKS
// ========================================

export const getWebhooks = () => api<any[]>("GET", "/webhooks");
export const createWebhook = (webhook: { name: string; url: string; secret?: string; events: string[] }) =>
  api<unknown>("POST", "/webhooks", webhook);
export const deleteWebhook = (id: string) => api<void>("DELETE", `/webhooks/${id}`);
export const testWebhook = (id: string) => api<{ status: number }>("POST", `/webhooks/${id}/test`);

// ========================================
// WEBSOCKET
// ========================================

export type WsMessage = {
  type: string;
  id: string;
  data: Record<string, unknown>;
};

export function connectWebSocket(
  onMessage: (msg: WsMessage) => void
): WebSocket {
  let reconnectDelay = 1000;

  function connect(): WebSocket {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const ws = new WebSocket(`${protocol}//${window.location.host}${API_BASE}/ws`);

    ws.onopen = () => {
      reconnectDelay = 1000; // Reset backoff on successful connect
    };

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data) as WsMessage;
        onMessage(msg);
      } catch {
        // ignore parse errors
      }
    };

    ws.onclose = () => {
      // Exponential backoff: 1s, 2s, 4s, 8s, max 30s
      setTimeout(() => connect(), reconnectDelay);
      reconnectDelay = Math.min(reconnectDelay * 2, 30000);
    };

    return ws;
  }

  return connect();
}

// ========================================
// USENET
// ========================================

export const getUsenetServers = () => api<any[]>("GET", "/usenet/servers");
export const addUsenetServer = (server: {
  name: string; host: string; port: number; ssl: boolean;
  username: string; password: string; connections: number; priority: number;
}) => api<unknown>("POST", "/usenet/servers", server);
export const deleteUsenetServer = (id: string) => api<void>("DELETE", `/usenet/servers/${id}`);
export const getUsenetDownloads = () => api<Download[]>("GET", "/downloads/usenet");
export const getNzbWatchDir = () => api<{ path: string }>("GET", "/usenet/watch-dir");
export const setNzbWatchDir = (path: string) => api<{ path: string }>("POST", "/usenet/watch-dir", { path });

// ========================================
// UNIFIED CONFIG
// ========================================

export interface AppConfig {
  download_dir: string;
  temp_dir: string;
  max_concurrent_downloads: number;
  bandwidth: {
    global_limit: number;
    http_limit: number;
    usenet_limit: number;
    schedule_enabled: boolean;
    schedules: { name: string; start: string; end: string; limit: number }[];
  };
  http: {
    max_chunks_per_download: number;
    max_connections_per_host: number;
    user_agent: string;
    timeout_connect_secs: number;
    timeout_read_secs: number;
  };
  usenet: {
    par2_repair: boolean;
    auto_unrar: boolean;
    delete_archives_after_extract: boolean;
    delete_par2_after_repair: boolean;
    selective_par2: boolean;
    sequential_postprocess: boolean;
  };
  retry: {
    max_retries: number;
    base_delay_secs: number;
    max_delay_secs: number;
  };
  features: {
    usenet: boolean;
    rss_feeds: boolean;
    server_stats: boolean;
  };
  [key: string]: unknown;
}

export const getConfig = () => api<AppConfig>("GET", "/config");
export const putConfig = (config: AppConfig) => api<AppConfig>("PUT", "/config", config);

// ========================================
// RSS FEEDS
// ========================================

export const getRssFeeds = () => api<any[]>("GET", "/rss");
export const addRssFeed = (feed: { name: string; url: string; category: string; interval_minutes: number }) =>
  api<unknown>("POST", "/rss", feed);
export const deleteRssFeed = (id: string) => api<void>("DELETE", `/rss/${id}`);

// ========================================
// HELPERS
// ========================================

export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0)} ${units[i]}`;
}

export function formatSpeed(bytesPerSec: number): string {
  return `${formatBytes(bytesPerSec)}/s`;
}

export function formatRelativeTime(isoString: string): string {
  const date = new Date(isoString);
  const now = Date.now();
  const diff = now - date.getTime();
  if (diff < 0) return "just now";
  const secs = Math.floor(diff / 1000);
  if (secs < 60) return "just now";
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d ago`;
  return date.toLocaleDateString();
}
