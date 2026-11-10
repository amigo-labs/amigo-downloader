import { describe, it, expect } from "vitest";
import {
  fileIcon, progressPct, statusInk, statusTone, statusLabelKey, isKnownStatus,
  canPause, canResume, canRetry, etaSeconds, formatEta, statusCounts, displayName,
  downloadStatusKey, failureReason, isRetrying, urlHost,
} from "../src/lib/download";

const base = {
  id: "1", url: "https://example.com/a.zip", filename: "a.zip",
  status: "downloading", filesize: 1000, bytes_downloaded: 250,
  speed: 100, protocol: "http", created_at: "", error: null,
} as any;

describe("fileIcon", () => {
  it("maps known extensions", () => {
    expect(fileIcon("a.zip")).toBe("archive");
    expect(fileIcon("a.MKV")).toBe("video");
    expect(fileIcon("a.flac")).toBe("music");
    expect(fileIcon("a.pdf")).toBe("file-text");
    expect(fileIcon("a.png")).toBe("image");
  });

  it("falls back for unknown or missing names", () => {
    expect(fileIcon("a.xyz")).toBe("file");
    expect(fileIcon(null)).toBe("file");
    expect(fileIcon("")).toBe("file");
    expect(fileIcon("noextension")).toBe("file");
  });
});

describe("progressPct", () => {
  it("computes a rounded percentage", () => {
    expect(progressPct({ filesize: 1000, bytes_downloaded: 250 })).toBe(25);
  });

  it("returns 0 when the size is unknown, rather than NaN or Infinity", () => {
    expect(progressPct({ filesize: null, bytes_downloaded: 250 } as any)).toBe(0);
    expect(progressPct({ filesize: 0, bytes_downloaded: 250 })).toBe(0);
  });

  it("clamps a server overshoot to 100", () => {
    expect(progressPct({ filesize: 100, bytes_downloaded: 150 })).toBe(100);
  });
});

describe("status helpers", () => {
  it("uses -ink tokens, never the decorative ones", () => {
    for (const s of ["downloading", "completed", "failed", "paused", "queued"]) {
      expect(statusInk(s)).not.toMatch(/--accent\)|--success\)|--danger\)|--warning\)/);
    }
  });

  it("passes unknown statuses through instead of inventing a key", () => {
    expect(statusLabelKey("downloading")).toBe("status.downloading");
    expect(statusLabelKey("weird")).toBe("weird");
    expect(isKnownStatus("weird")).toBe(false);
  });

  it("gates actions by status", () => {
    expect(canPause({ status: "downloading" })).toBe(true);
    expect(canPause({ status: "paused" })).toBe(false);
    expect(canResume({ status: "paused" })).toBe(true);
    expect(canResume({ status: "queued" })).toBe(true);
    expect(canRetry({ status: "failed" })).toBe(true);
    expect(canRetry({ status: "completed" })).toBe(false);
  });

  it("maps tones for the shared primitives", () => {
    expect(statusTone("completed")).toBe("success");
    expect(statusTone("failed")).toBe("danger");
    expect(statusTone("paused")).toBe("warning");
    expect(statusTone("queued")).toBe("accent");
  });
});

describe("etaSeconds / formatEta", () => {
  it("estimates from the remaining bytes and speed", () => {
    expect(etaSeconds({ ...base, filesize: 1000, bytes_downloaded: 250, speed: 250 })).toBe(3);
  });

  it("returns null when it cannot be estimated", () => {
    expect(etaSeconds({ ...base, speed: 0 })).toBeNull();
    expect(etaSeconds({ ...base, status: "paused" })).toBeNull();
    expect(etaSeconds({ ...base, filesize: null })).toBeNull();
    expect(etaSeconds({ ...base, bytes_downloaded: 1000 })).toBeNull();
  });

  it("formats across unit boundaries", () => {
    expect(formatEta(null)).toBe("");
    expect(formatEta(45)).toBe("45s");
    expect(formatEta(90)).toBe("1m 30s");
    expect(formatEta(3661)).toBe("1h 1m");
  });
});

describe("statusCounts", () => {
  it("counts every status in a single pass", () => {
    const counts = statusCounts([
      { status: "downloading" }, { status: "downloading" }, { status: "failed" },
    ]);
    expect(counts).toEqual({ downloading: 2, failed: 1 });
  });

  it("returns an empty map for an empty list", () => {
    expect(statusCounts([])).toEqual({});
  });
});

describe("displayName", () => {
  it("prefers the filename", () => {
    expect(displayName({ filename: "a.iso", url: "https://example.com/b.iso" })).toBe("a.iso");
  });

  it("treats an empty filename like a missing one", () => {
    expect(displayName({ filename: "", url: "https://x/y" })).toBe("y");
  });

  it("falls back to the decoded last path segment, without the query", () => {
    expect(
      displayName({ filename: null, url: "https://example.com/dl/My%20File.zip?token=abc&x=1" }),
    ).toBe("My File.zip");
  });

  it("falls back to the URL when there is no path segment or it cannot be parsed", () => {
    expect(displayName({ filename: null, url: "https://example.com/" })).toBe("https://example.com/");
    expect(displayName({ filename: null, url: "not a url" })).toBe("not a url");
  });
});

describe("retry and failure state", () => {
  it("shows a running download between attempts as retrying, not as an error", () => {
    const d = { status: "downloading", error: "retrying" };
    expect(isRetrying(d)).toBe(true);
    expect(downloadStatusKey(d)).toBe("status.retrying");
    expect(failureReason(d)).toBeNull();
  });

  it("gives the reason of a failed download", () => {
    const d = { status: "failed", error: "HTTP 404 Not Found" };
    expect(isRetrying(d)).toBe(false);
    expect(downloadStatusKey(d)).toBe("status.failed");
    expect(failureReason(d)).toBe("HTTP 404 Not Found");
  });

  it("has no reason without an error", () => {
    expect(failureReason({ status: "failed", error: null })).toBeNull();
    expect(failureReason({ status: "completed", error: "stale" })).toBeNull();
  });
});

describe("urlHost", () => {
  it("gives the host without path and query", () => {
    expect(urlHost("https://cdn.example.com/dl/a%20b.zip?token=1")).toBe("cdn.example.com");
  });

  it("keeps a non-default port", () => {
    expect(urlHost("http://nas.local:8080/file.iso")).toBe("nas.local:8080");
  });

  it("falls back to the input when it is not a URL", () => {
    expect(urlHost("not a url")).toBe("not a url");
  });
});
