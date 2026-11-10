import { describe, expect, it } from "vitest";
import { interpretWsEvent } from "../src/lib/wsEvents";

// Payloads exactly as the server serializes `DownloadEvent`
// (crates/core/tests/event_wire_format.rs pins the same strings).
const downloads = [
  { id: "d1", filename: "ubuntu.iso", url: "https://example.com/ubuntu.iso" },
  { id: "d2", filename: null, url: "https://example.com/get?id=2" },
];

describe("interpretWsEvent", () => {
  it("reads progress from the flat server payload", () => {
    const raw = JSON.parse(
      '{"type":"progress","id":"d1","progress":{"bytes_downloaded":341400000,"total_bytes":900000000,"speed_bytes_per_sec":4896147}}',
    );
    expect(interpretWsEvent(raw, downloads)).toEqual({
      kind: "progress",
      id: "d1",
      bytes: 341400000,
      speed: 4896147,
      total: 900000000,
    });
  });

  it("keeps an unknown total undefined", () => {
    const raw = JSON.parse(
      '{"type":"progress","id":"d1","progress":{"bytes_downloaded":10,"total_bytes":null,"speed_bytes_per_sec":5}}',
    );
    expect(interpretWsEvent(raw, downloads)).toMatchObject({ total: undefined, bytes: 10 });
  });

  it("carries the error text of a failed download, and its name", () => {
    const raw = JSON.parse('{"type":"failed","id":"d1","error":"HTTP 404 Not Found"}');
    expect(interpretWsEvent(raw, downloads)).toEqual({
      kind: "failed",
      id: "d1",
      name: "ubuntu.iso",
      error: "HTTP 404 Not Found",
    });
  });

  it("names a completed download by filename, falling back to the URL, then the id", () => {
    expect(interpretWsEvent({ type: "completed", id: "d1" }, downloads)).toMatchObject({
      name: "ubuntu.iso",
    });
    expect(interpretWsEvent({ type: "completed", id: "d2" }, downloads)).toMatchObject({
      name: "https://example.com/get?id=2",
    });
    expect(interpretWsEvent({ type: "completed", id: "gone" }, downloads)).toMatchObject({
      name: "gone",
    });
  });

  it("reads a captcha challenge from the flat payload", () => {
    const raw = JSON.parse(
      '{"type":"captcha_challenge","id":"c1","plugin_id":"hoster","download_id":"d1","image_url":"https://example.com/c.png","captcha_type":"image"}',
    );
    expect(interpretWsEvent(raw, downloads)).toEqual({
      kind: "captcha",
      challenge: {
        id: "c1",
        plugin_id: "hoster",
        download_id: "d1",
        image_url: "https://example.com/c.png",
        captcha_type: "image",
      },
    });
  });

  it("reads a plugin notification, which has no id", () => {
    const raw = JSON.parse(
      '{"type":"plugin_notification","plugin_id":"hoster","title":"Login","message":"Session renewed"}',
    );
    expect(interpretWsEvent(raw, downloads)).toEqual({
      kind: "notification",
      title: "Login",
      message: "Session renewed",
    });
  });

  it("passes status changes through and refreshes on list changes", () => {
    expect(
      interpretWsEvent({ type: "status_changed", id: "d1", status: "paused" }, downloads),
    ).toEqual({ kind: "status", id: "d1", status: "paused" });
    expect(interpretWsEvent({ type: "queue_empty" }, downloads)).toEqual({ kind: "refresh" });
    expect(interpretWsEvent({ type: "removed", id: "d1" }, downloads)).toEqual({ kind: "refresh" });
  });

  it("ignores malformed messages", () => {
    expect(interpretWsEvent(null, downloads)).toBeNull();
    expect(interpretWsEvent({ id: "d1" }, downloads)).toBeNull();
    expect(interpretWsEvent({ type: "progress", id: "d1" }, downloads)).toBeNull();
    expect(interpretWsEvent({ type: "nonsense" }, downloads)).toBeNull();
  });
});
