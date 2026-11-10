// What the server sends over /api/v1/ws, and what the UI does with it.
//
// The server serializes `DownloadEvent` (crates/core/src/coordinator.rs) with
// `#[serde(tag = "type")]`: the fields sit next to `type`, there is no `data`
// envelope. Reading them from a `data` object silently dropped every progress
// tick, every error text and every captcha challenge, so this module owns the
// wire shape and is unit-tested against literal server payloads.

import { displayName } from "./download";
import type { CaptchaChallenge, Download } from "./stores";

export type WsEvent =
  | { type: "added"; id: string; url: string }
  | {
      type: "progress";
      id: string;
      progress: {
        bytes_downloaded: number;
        total_bytes: number | null;
        speed_bytes_per_sec: number;
      };
    }
  | { type: "status_changed"; id: string; status: string }
  | { type: "completed"; id: string }
  | { type: "failed"; id: string; error: string }
  | { type: "removed"; id: string }
  | { type: "plugin_notification"; plugin_id: string; title: string; message: string }
  | ({ type: "captcha_challenge" } & CaptchaChallenge)
  | { type: "captcha_solved"; id: string }
  | { type: "captcha_timeout"; id: string }
  | { type: "queue_empty" };

/** What the UI should do for one event. `refresh` re-fetches the lists. */
export type WsAction =
  | { kind: "progress"; id: string; bytes: number; speed: number; total: number | undefined }
  | { kind: "status"; id: string; status: string }
  | { kind: "completed"; id: string; name: string }
  | { kind: "failed"; id: string; name: string; error: string }
  | { kind: "captcha"; challenge: CaptchaChallenge }
  | { kind: "captcha_done" }
  | { kind: "notification"; title: string; message: string }
  | { kind: "refresh" };

function str(v: unknown): string | undefined {
  return typeof v === "string" && v !== "" ? v : undefined;
}

function num(v: unknown): number | undefined {
  return typeof v === "number" && Number.isFinite(v) ? v : undefined;
}

/**
 * Turn one raw WebSocket message into an action. Unknown or malformed
 * messages yield `null` and are ignored.
 */
export function interpretWsEvent(
  raw: unknown,
  downloads: Pick<Download, "id" | "filename" | "url">[],
): WsAction | null {
  if (typeof raw !== "object" || raw === null) return null;
  const msg = raw as Record<string, unknown>;
  const type = str(msg.type);
  if (!type) return null;
  const id = str(msg.id);
  const nameOf = (downloadId: string): string => {
    const d = downloads.find((x) => x.id === downloadId);
    return d ? displayName(d) : downloadId;
  };

  switch (type) {
    case "progress": {
      const p = msg.progress as Record<string, unknown> | null | undefined;
      if (!id || typeof p !== "object" || p === null) return null;
      return {
        kind: "progress",
        id,
        bytes: num(p.bytes_downloaded) ?? 0,
        speed: num(p.speed_bytes_per_sec) ?? 0,
        total: num(p.total_bytes),
      };
    }
    case "status_changed": {
      const status = str(msg.status);
      return id && status ? { kind: "status", id, status } : null;
    }
    case "completed":
      return id ? { kind: "completed", id, name: nameOf(id) } : null;
    case "failed":
      return id
        ? { kind: "failed", id, name: nameOf(id), error: str(msg.error) ?? "" }
        : null;
    case "captcha_challenge": {
      const challenge: CaptchaChallenge = {
        id: id ?? "",
        plugin_id: str(msg.plugin_id) ?? "",
        download_id: str(msg.download_id) ?? "",
        image_url: str(msg.image_url) ?? "",
        captcha_type: str(msg.captcha_type) ?? "image",
      };
      return challenge.id ? { kind: "captcha", challenge } : null;
    }
    case "captcha_solved":
    case "captcha_timeout":
      return { kind: "captcha_done" };
    case "plugin_notification":
      return {
        kind: "notification",
        title: str(msg.title) ?? "Plugin",
        message: str(msg.message) ?? "",
      };
    case "added":
    case "removed":
    case "queue_empty":
      return { kind: "refresh" };
    default:
      return null;
  }
}
