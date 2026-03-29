// REST + WebSocket client for amigo-downloader API

const API_BASE = "/api/v1";

// ========================================
// REST API
// ========================================

export async function getStatus() {
  const res = await fetch(`${API_BASE}/status`);
  return res.json();
}

export async function getStats() {
  const res = await fetch(`${API_BASE}/stats`);
  return res.json();
}

export async function getDownloads() {
  const res = await fetch(`${API_BASE}/downloads`);
  return res.json();
}

export async function addDownload(url: string, filename?: string) {
  const res = await fetch(`${API_BASE}/downloads`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ url, filename }),
  });
  return res.json();
}

export async function addBatch(urls: string[]) {
  const res = await fetch(`${API_BASE}/downloads/batch`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ urls }),
  });
  return res.json();
}

export async function pauseDownload(id: string) {
  return fetch(`${API_BASE}/downloads/${id}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ action: "pause" }),
  });
}

export async function resumeDownload(id: string) {
  return fetch(`${API_BASE}/downloads/${id}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ action: "resume" }),
  });
}

export async function deleteDownload(id: string) {
  return fetch(`${API_BASE}/downloads/${id}`, { method: "DELETE" });
}

export async function getQueue() {
  const res = await fetch(`${API_BASE}/queue`);
  return res.json();
}

export async function getHistory() {
  const res = await fetch(`${API_BASE}/history`);
  return res.json();
}

export async function getPlugins() {
  const res = await fetch(`${API_BASE}/plugins`);
  return res.json();
}

export async function checkUpdates() {
  const res = await fetch(`${API_BASE}/updates/check`);
  return res.json();
}

export async function importDlc(file: File) {
  const formData = new FormData();
  formData.append("file", file);
  const res = await fetch(`${API_BASE}/downloads/container`, {
    method: "POST",
    body: formData,
  });
  return res.json();
}

export async function uploadNzb(nzbData: string) {
  const res = await fetch(`${API_BASE}/downloads/nzb`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ nzb_data: nzbData }),
  });
  return res.json();
}

export async function getSystemInfo() {
  const res = await fetch(`${API_BASE}/system-info`);
  return res.json();
}

export async function submitFeedback(data: {
  type: "bug" | "feature" | "crash";
  title: string;
  description: string;
  include_system_info?: boolean;
  error_context?: { download_id?: string; error_message?: string; url?: string };
}) {
  const res = await fetch(`${API_BASE}/feedback`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  });
  return res.json();
}

// ========================================
// CAPTCHA
// ========================================

export async function getPendingCaptchas() {
  const res = await fetch(`${API_BASE}/captcha/pending`);
  return res.json();
}

export async function solveCaptcha(id: string, answer: string) {
  return fetch(`${API_BASE}/captcha/${id}/solve`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ answer }),
  });
}

export async function cancelCaptcha(id: string) {
  return fetch(`${API_BASE}/captcha/${id}/cancel`, { method: "POST" });
}

// ========================================
// WEBHOOKS
// ========================================

export async function getWebhooks() {
  const res = await fetch(`${API_BASE}/webhooks`);
  return res.json();
}

export async function createWebhook(webhook: {
  name: string;
  url: string;
  secret?: string;
  events: string[];
}) {
  const res = await fetch(`${API_BASE}/webhooks`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(webhook),
  });
  return res.json();
}

export async function deleteWebhook(id: string) {
  return fetch(`${API_BASE}/webhooks/${id}`, { method: "DELETE" });
}

export async function testWebhook(id: string) {
  const res = await fetch(`${API_BASE}/webhooks/${id}/test`, { method: "POST" });
  return res.json();
}

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
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const ws = new WebSocket(`${protocol}//${window.location.host}${API_BASE}/ws`);

  ws.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data) as WsMessage;
      onMessage(msg);
    } catch {
      // ignore parse errors
    }
  };

  ws.onclose = () => {
    // Auto-reconnect after 3s
    setTimeout(() => connectWebSocket(onMessage), 3000);
  };

  return ws;
}

// ========================================
// USENET
// ========================================

export async function getUsenetServers() {
  const res = await fetch(`${API_BASE}/usenet/servers`);
  return res.json();
}

export async function addUsenetServer(server: {
  name: string; host: string; port: number; ssl: boolean;
  username: string; password: string; connections: number; priority: number;
}) {
  const res = await fetch(`${API_BASE}/usenet/servers`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(server),
  });
  if (!res.ok) throw new Error((await res.json()).error);
  return res.json();
}

export async function deleteUsenetServer(id: string) {
  return fetch(`${API_BASE}/usenet/servers/${id}`, { method: "DELETE" });
}

export async function getUsenetDownloads() {
  const res = await fetch(`${API_BASE}/downloads/usenet`);
  return res.json();
}

export async function getNzbWatchDir(): Promise<{ path: string }> {
  const res = await fetch(`${API_BASE}/usenet/watch-dir`);
  return res.json();
}

export async function setNzbWatchDir(path: string) {
  const res = await fetch(`${API_BASE}/usenet/watch-dir`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ path }),
  });
  return res.json();
}

// ========================================
// USENET PROCESSING CONFIG
// ========================================

export interface UsenetProcessing {
  par2_repair: boolean;
  auto_unrar: boolean;
  delete_archives_after_extract: boolean;
  delete_par2_after_repair: boolean;
  selective_par2: boolean;
  sequential_postprocess: boolean;
}

export async function getUsenetProcessing(): Promise<UsenetProcessing> {
  const res = await fetch(`${API_BASE}/usenet/processing`);
  return res.json();
}

export async function updateUsenetProcessing(config: UsenetProcessing) {
  const res = await fetch(`${API_BASE}/usenet/processing`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(config),
  });
  return res.json();
}

// ========================================
// FEATURE FLAGS
// ========================================

export async function getFeatures(): Promise<{ rss_feeds: boolean; server_stats: boolean }> {
  const res = await fetch(`${API_BASE}/features`);
  return res.json();
}

export async function updateFeatures(features: { rss_feeds: boolean; server_stats: boolean }) {
  const res = await fetch(`${API_BASE}/features`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(features),
  });
  return res.json();
}

// ========================================
// RSS FEEDS
// ========================================

export async function getRssFeeds() {
  const res = await fetch(`${API_BASE}/rss`);
  return res.json();
}

export async function addRssFeed(feed: {
  name: string; url: string; category: string; interval_minutes: number;
}) {
  const res = await fetch(`${API_BASE}/rss`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(feed),
  });
  if (!res.ok) throw new Error((await res.json()).error);
  return res.json();
}

export async function deleteRssFeed(id: string) {
  return fetch(`${API_BASE}/rss/${id}`, { method: "DELETE" });
}

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
