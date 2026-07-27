// Pure helpers for rendering a download.
//
// Every one of these existed in two or three verbatim copies -- DownloadCard,
// DownloadCompactRow and DetailPanel each had their own fileIcon, progress,
// statusColor, statusLabel and eta. Keeping them here makes the two list
// variants incapable of drifting apart, and makes the logic unit-testable
// without mounting a component.

import type { Download } from "./stores";

export const KNOWN_STATUS = [
  "queued",
  "downloading",
  "paused",
  "completed",
  "failed",
] as const;

export type DownloadStatus = (typeof KNOWN_STATUS)[number];

export function isKnownStatus(status: string): status is DownloadStatus {
  return (KNOWN_STATUS as readonly string[]).includes(status);
}

const ICON_BY_EXT: Record<string, string> = {
  zip: "archive", rar: "archive", "7z": "archive", tar: "archive", gz: "archive", bz2: "archive",
  mp4: "video", mkv: "video", avi: "video", mov: "video", webm: "video", flv: "video",
  mp3: "music", flac: "music", ogg: "music", wav: "music", aac: "music", m4a: "music",
  pdf: "file-text", doc: "file-text", docx: "file-text", txt: "file-text", rtf: "file-text", odt: "file-text",
  jpg: "image", jpeg: "image", png: "image", gif: "image", svg: "image", webp: "image", bmp: "image",
};

export function fileIcon(filename: string | null | undefined): string {
  if (!filename) return "file";
  const ext = filename.split(".").pop()?.toLowerCase() ?? "";
  return ICON_BY_EXT[ext] ?? "file";
}

export function progressPct(d: Pick<Download, "filesize" | "bytes_downloaded">): number {
  if (!d.filesize) return 0;
  return Math.max(0, Math.min(100, Math.round((d.bytes_downloaded / d.filesize) * 100)));
}

/** Text/icon colour for a status. Uses -ink tokens, which clear WCAG AA. */
export function statusInk(status: string): string {
  switch (status) {
    case "downloading": return "var(--accent-ink)";
    case "completed": return "var(--success-ink)";
    case "failed": return "var(--danger-ink)";
    case "paused": return "var(--warning-ink)";
    default: return "var(--text-secondary)";
  }
}

/** Decorative fill for a status (accent bars, dots). Never carries text. */
export function statusFill(status: string): string {
  switch (status) {
    case "downloading": return "var(--accent)";
    case "completed": return "var(--success)";
    case "failed": return "var(--danger)";
    case "paused": return "var(--warning)";
    default: return "var(--text-muted)";
  }
}

/** Tone name for the shared primitives. */
export function statusTone(status: string): "accent" | "success" | "warning" | "danger" {
  switch (status) {
    case "completed": return "success";
    case "failed": return "danger";
    case "paused": return "warning";
    default: return "accent";
  }
}

/** i18n key for a status; callers run it through tr(). */
export function statusLabelKey(status: string): string {
  return isKnownStatus(status) ? `status.${status}` : status;
}

export function isActive(d: Pick<Download, "status">): boolean {
  return d.status === "downloading";
}

export function canPause(d: Pick<Download, "status">): boolean {
  return d.status === "downloading";
}

export function canResume(d: Pick<Download, "status">): boolean {
  return d.status === "paused" || d.status === "queued";
}

export function canRetry(d: Pick<Download, "status">): boolean {
  return d.status === "failed";
}

/** Seconds remaining, or null when it cannot be estimated. */
export function etaSeconds(
  d: Pick<Download, "status" | "speed" | "filesize" | "bytes_downloaded">,
): number | null {
  if (!isActive(d) || !d.speed || d.speed <= 0 || !d.filesize) return null;
  const remaining = d.filesize - d.bytes_downloaded;
  if (remaining <= 0) return null;
  return Math.round(remaining / d.speed);
}

export function formatEta(secs: number | null): string {
  if (secs === null) return "";
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return `${h}h ${m}m`;
}

/**
 * Count downloads per status in one pass.
 *
 * The filter bar used to call a `countByStatus(status)` helper once per chip,
 * so every render walked the full list six times -- on every WebSocket
 * progress tick.
 */
export function statusCounts(list: Pick<Download, "status">[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const d of list) counts[d.status] = (counts[d.status] ?? 0) + 1;
  return counts;
}

/** Human label for a download, preferring the filename over the raw URL. */
export function displayName(d: Pick<Download, "filename" | "url">): string {
  return d.filename || d.url;
}
